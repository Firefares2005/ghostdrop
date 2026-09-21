//! Wire protocol shared by sender and receiver.
//!
//! After the QUIC connection is open, the receiver opens one bidirectional
//! stream and writes "go". The sender answers with:
//!
//!   kind (u8) | name length (u16, big-endian) | name | size (u64, big-endian)
//!   then the payload:
//!     kind 0 (file): exactly `size` raw bytes
//!     kind 1 (dir):  a tar stream (size is 0 = unknown)

use std::io;

use anyhow::{bail, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Protocol identifier. Bump when the wire format changes.
pub const ALPN: &[u8] = b"ghostdrop/1";

pub const KIND_FILE: u8 = 0;
pub const KIND_DIR: u8 = 1;

pub struct Header {
    pub kind: u8,
    pub name: String,
    pub size: u64,
}

impl Header {
    pub async fn write<W: AsyncWrite + Unpin>(&self, w: &mut W) -> Result<()> {
        let nb = self.name.as_bytes();
        if nb.len() > u16::MAX as usize {
            bail!("name too long");
        }
        w.write_all(&[self.kind]).await?;
        w.write_all(&(nb.len() as u16).to_be_bytes()).await?;
        w.write_all(nb).await?;
        w.write_all(&self.size.to_be_bytes()).await?;
        Ok(())
    }

    pub async fn read<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
        let kind = r.read_u8().await?;
        if kind != KIND_FILE && kind != KIND_DIR {
            bail!("unknown transfer kind {kind}");
        }
        let len = r.read_u16().await? as usize;
        let mut name = vec![0u8; len];
        r.read_exact(&mut name).await?;
        let size = r.read_u64().await?;
        Ok(Self {
            kind,
            name: String::from_utf8(name)?,
            size,
        })
    }
}

/// Copy everything from `r` to `w`. Calls `on_bytes(n)` after every chunk.
pub async fn copy_counted<R, W, F>(r: &mut R, w: &mut W, mut on_bytes: F) -> Result<u64>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
    F: FnMut(u64),
{
    let mut buf = vec![0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = r.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        w.write_all(&buf[..n]).await?;
        total += n as u64;
        on_bytes(n as u64);
    }
    Ok(total)
}

/// Blocking `Read` wrapper that reports how many bytes went through.
pub struct CountingReader<R, F> {
    inner: R,
    on_bytes: F,
}

impl<R, F> CountingReader<R, F> {
    pub fn new(inner: R, on_bytes: F) -> Self {
        Self { inner, on_bytes }
    }
}

impl<R: io::Read, F: FnMut(u64)> io::Read for CountingReader<R, F> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        if n > 0 {
            (self.on_bytes)(n as u64);
        }
        Ok(n)
    }
}