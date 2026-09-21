use std::{path::PathBuf, time::Duration};

use anyhow::{bail, Context, Result};
use iroh::Endpoint;
use tokio::fs::File;
use tokio_util::io::SyncIoBridge;

use crate::{
    proto::{copy_counted, Header, ALPN, KIND_DIR, KIND_FILE},
    Event, Events,
};

/// Send a file or a folder to whoever connects with our code.
///
/// Emits `Ready` (with the code) as soon as the receiver can be told.
/// Accepts exactly one connection, then stops listening.
pub async fn send(path: PathBuf, events: Events) -> Result<()> {
    // Absolute path, so "." and ".." still give a real name.
    let path = tokio::fs::canonicalize(&path)
        .await
        .context("path not found")?;
    let meta = tokio::fs::metadata(&path).await?;
    let is_dir = meta.is_dir();
    let name = path
        .file_name()
        .context("path has no name (root folder?)")?
        .to_string_lossy()
        .to_string();

    // Fresh keypair every run: the public key is the code.
    let ep = Endpoint::builder()
        .alpns(vec![ALPN.to_vec()])
        .discovery_n0() // lets the receiver find us from the key alone
        .bind()
        .await?;
    tokio::time::sleep(Duration::from_secs(1)).await; // let discovery publish

    events.emit(Event::Ready {
        code: ep.node_id().to_string(),
    });

    let incoming = ep.accept().await.context("endpoint closed")?;
    let conn = incoming.await?;
    let (mut tx, mut rx) = conn.accept_bi().await?;
    events.emit(Event::Connected);

    let mut hello = [0u8; 2];
    rx.read_exact(&mut hello).await?;
    if &hello != b"go" {
        bail!("unexpected handshake");
    }

    let header = Header {
        kind: if is_dir { KIND_DIR } else { KIND_FILE },
        name: name.clone(),
        size: if is_dir { 0 } else { meta.len() },
    };
    header.write(&mut tx).await?;
    events.emit(Event::Started {
        name: name.clone(),
        is_dir,
        size: if is_dir { None } else { Some(meta.len()) },
    });

    let mut done = 0u64;
    if is_dir {
        // tar runs on a blocking thread and streams into a pipe;
        // we read the pipe and forward it over the network. No temp files.
        let (mut pipe_rd, pipe_wr) = tokio::io::duplex(256 * 1024);
        let bridge = SyncIoBridge::new(pipe_wr);
        let (dir, top) = (path.clone(), name.clone());
        let task = tokio::task::spawn_blocking(move || -> Result<()> {
            let mut b = tar::Builder::new(bridge);
            b.follow_symlinks(false);
            b.append_dir_all(&top, &dir)?;
            b.finish()?;
            Ok(())
        });
        copy_counted(&mut pipe_rd, &mut tx, |n| {
            done += n;
            events.emit(Event::Progress { bytes: done });
        })
        .await?;
        task.await??;
    } else {
        let mut file = File::open(&path).await.context("cannot open file")?;
        copy_counted(&mut file, &mut tx, |n| {
            done += n;
            events.emit(Event::Progress { bytes: done });
        })
        .await?;
    }

    tx.finish()?;
    conn.closed().await; // wait until the receiver confirms it got everything
    ep.close().await;
    events.emit(Event::Finished { path: None });
    Ok(())
}