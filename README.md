# ghostdrop

Send a file **or a whole folder** to another device with one command.
P2P, end-to-end encrypted, no account, no server of your own.

```
# device A
$ ghostdrop send my_project/
On the other device run:  ghostdrop get <code>      (+ QR code)

# device B
$ ghostdrop get <code>
$ ghostdrop get <code> --out ~/Downloads
```

## Build
```
cargo build --release
# binary: target/release/ghostdrop
```
iroh's API changes between versions, so keep `iroh = "0.35"` pinned.

On Android (Termux): `pkg install rust`, then the same `cargo build --release`.

## How it works
- The sender creates a fresh keypair; the code is its public key.
- The receiver finds the sender via iroh discovery and connects directly
  (hole punching) or through a relay, over QUIC + TLS 1.3.
- The relay only forwards encrypted bytes and cannot read anything.
- Files are streamed as raw bytes. Folders are streamed as tar, on the fly,
  with no temporary archive on disk.
- The sender accepts one connection, then stops listening.

## Safety
- Sender file names are reduced to their last path component.
- Existing files or folders are never overwritten.
- Folders are unpacked into a scratch directory first; tar refuses `../` paths.
- Symlinks are not followed when sending folders.

## Layout
```
src/main.rs   CLI (clap)
src/proto.rs  wire format, progress bar, copy helper
src/send.rs   sender
src/recv.rs   receiver
```

## Roadmap
- [x] Files, folders, progress, QR
- [ ] `--lan` mode (local network discovery)
- [ ] Short words code (blue-tiger-moon) via DHT, no server
- [ ] Split into lib + CLI, then Android app (Flutter + flutter_rust_bridge)