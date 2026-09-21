use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use iroh::{Endpoint, NodeId};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};
use tokio_util::io::SyncIoBridge;

use crate::{
    proto::{copy_counted, CountingReader, Header, ALPN, KIND_FILE},
    Event, Events,
};

/// Never trust a name coming from the network: keep only the last component.
fn sanitize(name: &str) -> Result<String> {
    let f = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .context("bad name from sender")?;
    if f.is_empty() || f == "." || f == ".." {
        bail!("bad name from sender");
    }
    Ok(f.to_string())
}

/// Receive a file or folder from the sender with this `code`, into `out`.
/// Returns the path where it was saved.
pub async fn receive(code: String, out: PathBuf, events: Events) -> Result<PathBuf> {
    let node_id: NodeId = code.trim().parse().context("invalid code")?;

    let ep = Endpoint::builder().discovery_n0().bind().await?;
    let conn = ep
        .connect(node_id, ALPN)
        .await
        .context("could not reach sender (is it still waiting?)")?;
    events.emit(Event::Connected);

    let (mut tx, mut rx) = conn.open_bi().await?;
    tx.write_all(b"go").await?;
    tx.finish()?;

    let header = Header::read(&mut rx).await?;
    let safe = sanitize(&header.name)?;
    tokio::fs::create_dir_all(&out).await?;
    let dest = out.join(&safe);
    if dest.exists() {
        bail!("{} already exists, refusing to overwrite", dest.display());
    }

    let is_file = header.kind == KIND_FILE;
    events.emit(Event::Started {
        name: safe.clone(),
        is_dir: !is_file,
        size: if is_file { Some(header.size) } else { None },
    });

    let mut done = 0u64;
    if is_file {
        let part = out.join(format!("{safe}.part"));
        let mut file = File::create(&part).await?;
        let mut limited = (&mut rx).take(header.size);
        let copied = copy_counted(&mut limited, &mut file, |n| {
            done += n;
            events.emit(Event::Progress { bytes: done });
        })
        .await?;
        file.flush().await?;
        if copied != header.size {
            bail!("incomplete transfer: {copied}/{} bytes", header.size);
        }
        tokio::fs::rename(&part, &dest).await?;
    } else {
        // Unpack into a scratch folder first, so a malicious sender can never
        // write next to (or over) your other files.
        let tmp = out.join(format!(".ghostdrop-{}", std::process::id()));
        tokio::fs::create_dir_all(&tmp).await?;

        let ev = events.clone();
        let reader = CountingReader::new(SyncIoBridge::new(rx), move |n| {
            done += n;
            ev.emit(Event::Progress { bytes: done });
        });
        let tmp2 = tmp.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            // tar refuses entries that escape the target folder ("../")
            tar::Archive::new(reader).unpack(&tmp2)?;
            Ok(())
        })
        .await??;

        let extracted = tmp.join(&safe);
        if !extracted.is_dir() {
            tokio::fs::remove_dir_all(&tmp).await.ok();
            bail!("archive did not contain the expected folder");
        }
        tokio::fs::rename(&extracted, &dest).await?;
        tokio::fs::remove_dir_all(&tmp).await.ok();
    }

    conn.close(0u32.into(), b"done");
    ep.close().await;
    events.emit(Event::Finished {
        path: Some(dest.clone()),
    });
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::sanitize;

    #[test]
    fn strips_path_components() {
        assert_eq!(sanitize("../../etc/passwd").unwrap(), "passwd");
        assert_eq!(sanitize("folder").unwrap(), "folder");
    }

    #[test]
    fn rejects_dangerous_names() {
        assert!(sanitize("..").is_err());
        assert!(sanitize("").is_err());
    }
}