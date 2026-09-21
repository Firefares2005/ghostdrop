//! # ghostdrop
//!
//! Send a file or a folder to another device. P2P, encrypted, no account.
//!
//! The library never prints and never draws progress bars. It reports what
//! happens through [`Events`], so any UI (terminal, Flutter, Tauri...) can
//! plug in.
//!
//! ```no_run
//! use ghostdrop::{send, Event, Events};
//!
//! # async fn demo() -> anyhow::Result<()> {
//! let events = Events::new(|e| {
//!     if let Event::Ready { code } = e {
//!         println!("tell the receiver: {code}");
//!     }
//! });
//! send("photo.jpg".into(), events).await?;
//! # Ok(())
//! # }
//! ```

mod proto;
mod receiver;
mod sender;

use std::{path::PathBuf, sync::Arc};

pub use receiver::receive;
pub use sender::send;

/// Something that happened during a transfer.
#[derive(Debug, Clone)]
pub enum Event {
    /// Sender only: the code to give to the receiver is ready.
    Ready { code: String },
    /// The other side connected.
    Connected,
    /// The transfer is starting. `size` is `None` for folders (unknown upfront).
    Started {
        name: String,
        is_dir: bool,
        size: Option<u64>,
    },
    /// Total bytes transferred so far.
    Progress { bytes: u64 },
    /// Finished. `path` is where the data was saved (receiver only).
    Finished { path: Option<PathBuf> },
}

/// Callback that receives [`Event`]s. Cheap to clone.
#[derive(Clone)]
pub struct Events(Arc<dyn Fn(Event) + Send + Sync>);

impl Events {
    pub fn new(f: impl Fn(Event) + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }

    /// Ignore all events.
    pub fn none() -> Self {
        Self::new(|_| {})
    }

    pub(crate) fn emit(&self, e: Event) {
        (self.0)(e)
    }
}