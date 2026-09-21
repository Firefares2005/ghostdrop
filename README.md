# ghostdrop

**Send a file or a whole folder to another device with one command.**
Peer-to-peer, end-to-end encrypted, no account, no server to run.

[![Release](https://img.shields.io/github/v/release/Firefares2005/ghostdrop)](https://github.com/Firefares2005/ghostdrop/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

```text
# Device A (sender)
$ ghostdrop send my_project/

On the other device run:

  ghostdrop get 749deace709f01e84d83a8afd16edd8eb127f8fc2bdbf3bbc17ca3debdfe9ce2

# Device B (receiver)
$ ghostdrop get 749deace709f...
$ ghostdrop get 749deace709f... --out ~/Downloads
```

The sender prints a code (and a QR code). The receiver types it. That's it.

## Features

- **Files and folders**: folders are streamed as tar on the fly, with no temporary archive on disk.
- **Direct connection**: NAT hole punching over QUIC, with an encrypted relay as fallback.
- **End-to-end encrypted**: QUIC + TLS 1.3. The relay only sees encrypted bytes.
- **No account, no setup**: a fresh keypair is generated on every run.
- **Live progress**: progress bar, speed and ETA.
- **QR code** in the terminal for quick pairing.
- **Cross-platform**: Windows, macOS, Linux and Android (Termux).
- **Library + CLI**: the core is a Rust library, so any UI can plug in.

## Install

### Prebuilt binaries

**Linux / macOS**
```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/Firefares2005/ghostdrop/releases/latest/download/ghostdrop-installer.sh | sh
```

**Windows (PowerShell)**
```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/Firefares2005/ghostdrop/releases/latest/download/ghostdrop-installer.ps1 | iex"
```

Binaries are installed to `~/.cargo/bin`. Archives for every platform are also on the
[Releases page](https://github.com/Firefares2005/ghostdrop/releases).

### From source

```bash
cargo install --git https://github.com/Firefares2005/ghostdrop
```

### Android (Termux)

The prebuilt Linux binary does not run in Termux (it needs glibc), so build from source:

```bash
pkg update && pkg install -y rust git
cargo install --git https://github.com/Firefares2005/ghostdrop --profile dist
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc && source ~/.bashrc
termux-setup-storage   # once, to reach your Downloads folder
```

Save received files to `~/storage/downloads` so they show up in your file manager.

## Usage

```bash
# Send a file or a folder
ghostdrop send photo.jpg
ghostdrop send "D:\Projects\my folder"
ghostdrop send ~/Documents --no-qr      # hide the QR code

# Receive
ghostdrop get <code>                    # save in the current folder
ghostdrop get <code> --out ~/Downloads  # save somewhere else (created if missing)
```

| Command | Options | Description |
|---|---|---|
| `send <path>` | `--no-qr` | Send one file or folder and print a code |
| `get <code>` | `-o, --out <dir>` | Receive using the sender's code (default: `.`) |

### Good to know

- **One item per transfer.** To send several files, put them in a folder.
- The sender must **stay running** until the transfer finishes.
- The code works for **one connection**; the sender stops listening after that.
- Existing files or folders are **never overwritten**. Pick another `--out` or remove the old one.
- Root folders (`C:\`, `/`) can't be sent. Send a folder inside them.
- If a transfer is interrupted, start it again from the beginning (no resume yet).

## How it works

1. The sender creates a fresh keypair on every run. **The code is its public key.**
2. The receiver finds the sender through [iroh](https://github.com/n0-computer/iroh) discovery, using only that key.
3. They connect directly via hole punching, or through a relay, over **QUIC + TLS 1.3**.
4. The receiver opens a stream and the sender replies with a small header (kind, name, size), then the payload.
5. Files are sent as raw bytes. Folders are sent as a tar stream.
6. The receiver confirms completion and both sides close.

```text
receiver -> "go"
sender   -> kind (u8) | name length (u16) | name | size (u64) | payload
```

The wire protocol is identified by the ALPN `ghostdrop/1`.

## Security

- **Treat the code like a password.** Anyone who has it can connect before your receiver does. Share it over a channel you trust.
- Traffic is encrypted end to end. Relays only forward encrypted bytes and cannot read your data.
- Names sent by the other side are reduced to their last path component, so `../../etc/passwd` becomes `passwd`.
- Folders are unpacked into a scratch directory first and moved into place only after validation. tar refuses entries that escape the target (`../`).
- Symlinks are **not followed** when sending folders.
- Nothing is overwritten. Incomplete files are written as `.part` and renamed only when complete.

## Use as a library

The library never prints and never draws progress bars. It reports everything through `Events`, so a terminal, Flutter or Tauri app can plug in.

```rust
use ghostdrop::{send, receive, Event, Events};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let events = Events::new(|e| match e {
        Event::Ready { code } => println!("tell the receiver: {code}"),
        Event::Progress { bytes } => println!("{bytes} bytes"),
        _ => {}
    });
    send("photo.jpg".into(), events).await?;
    Ok(())
}
```

Library only (no CLI dependencies):

```bash
cargo build --no-default-features
```

`receive(code, out_dir, events)` returns the path where the data was saved.

## Project layout

```text
src/main.rs      CLI (clap, indicatif, qrcode)
src/lib.rs       public API: send, receive, Event, Events
src/proto.rs     wire format and copy helpers
src/sender.rs    sender
src/receiver.rs  receiver
```

iroh's API changes between versions, so `iroh = "0.35"` is pinned.

## Roadmap

- [x] Files, folders, progress bar, QR code
- [x] Library + CLI split
- [x] Prebuilt binaries for Windows, macOS and Linux
- [ ] Send multiple paths in one command
- [ ] Resume interrupted transfers
- [ ] `--lan` mode (local network discovery)
- [ ] Short word codes (`blue-tiger-moon`)
- [ ] Android app (Flutter + flutter_rust_bridge)

## Releasing

Releases are automated with [cargo-dist](https://github.com/axodotdev/cargo-dist).
Bump `version` in `Cargo.toml`, commit, then:

```bash
git tag vX.Y.Z
git push origin main vX.Y.Z
```

GitHub Actions builds every platform and publishes the release.

## License

[MIT](LICENSE)
