//! Terminal app. All the real work lives in the library (src/lib.rs).

use std::{path::PathBuf, time::Duration};

use anyhow::Result;
use clap::{Parser, Subcommand};
use ghostdrop::{receive, send, Event, Events};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use qrcode::{render::unicode, QrCode};

#[derive(Parser)]
#[command(
    name = "ghostdrop",
    version,
    about = "Send files or folders device-to-device, encrypted, no account"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Send a file or a folder. Prints a code (and QR) for the receiver.
    Send {
        path: PathBuf,
        /// Do not print the QR code
        #[arg(long)]
        no_qr: bool,
    },
    /// Receive using the code printed by the sender.
    Get {
        code: String,
        /// Folder to save into
        #[arg(short, long, default_value = ".")]
        out: PathBuf,
    },
}

/// Turn library events into terminal output.
fn cli_events(no_qr: bool) -> Events {
    let pb = ProgressBar::hidden();
    Events::new(move |e| match e {
        Event::Ready { code } => {
            println!("On the other device run:\n\n  ghostdrop get {code}\n");
            if !no_qr {
                if let Ok(qr) = QrCode::new(code.as_bytes()) {
                    let art = qr
                        .render::<unicode::Dense1x2>()
                        .dark_color(unicode::Dense1x2::Light)
                        .light_color(unicode::Dense1x2::Dark)
                        .build();
                    println!("{art}\n");
                }
            }
            println!("Waiting for receiver...");
        }
        Event::Connected => println!("Connected."),
        Event::Started { name, is_dir, size } => {
            println!("{} {name}", if is_dir { "Folder:" } else { "File:" });
            pb.set_draw_target(ProgressDrawTarget::stderr());
            match size {
                Some(total) => {
                    pb.set_length(total);
                    pb.set_style(
                        ProgressStyle::with_template(
                            "{bar:40.cyan/blue} {bytes}/{total_bytes} ({bytes_per_sec}, eta {eta})",
                        )
                        .unwrap(),
                    );
                }
                None => {
                    pb.set_style(
                        ProgressStyle::with_template("{spinner} {bytes} ({bytes_per_sec})")
                            .unwrap(),
                    );
                    pb.enable_steady_tick(Duration::from_millis(100));
                }
            }
        }
        Event::Progress { bytes } => pb.set_position(bytes),
        Event::Finished { .. } => pb.finish(),
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Send { path, no_qr } => {
            send(path, cli_events(no_qr)).await?;
            println!("Done.");
        }
        Cmd::Get { code, out } => {
            let saved = receive(code, out, cli_events(true)).await?;
            println!("Saved to {}", saved.display());
        }
    }
    Ok(())
}