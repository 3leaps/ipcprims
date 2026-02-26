//! Async echo server — accepts one peer and echoes frames back (Tokio, Unix-only).
//!
//! Run with:
//!   cargo run --example async-echo-server --features async
//!
//! Notes:
//! - This example is Unix-only in v0.2.0 (async UDS).
//! - Use the sync CLI (`ipcprims send`) or another async client to drive it.

#[cfg(unix)]
mod unix {
    use std::fs;

    use ipcprims::peer::AsyncPeerListener;

    pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
        // macOS UDS paths have strict length limits; keep the path short by anchoring in /tmp.
        let sock_dir = std::path::PathBuf::from("/tmp")
            .join(format!("ipcprims-async-echo-{}", std::process::id()));
        fs::create_dir_all(&sock_dir)?;
        let sock_path = sock_dir.join("echo.sock");

        // Ensure no stale socket.
        let _ = fs::remove_file(&sock_path);

        let listener = AsyncPeerListener::bind(&sock_path)?;
        eprintln!("Listening on {}", sock_path.display());

        let peer = listener.accept().await?;
        eprintln!("Peer connected: {}", peer.id());

        let (tx, mut rx) = peer.into_split();
        let mut any = rx.take_any_receiver();

        loop {
            match any.recv().await {
                Ok(frame) => {
                    eprintln!(
                        "Received {} bytes on channel {}",
                        frame.payload.len(),
                        frame.channel
                    );
                    tx.send(frame.channel, &frame.payload).await?;
                }
                Err(e) => {
                    eprintln!("Peer disconnected: {e}");
                    break;
                }
            }
        }

        let _ = fs::remove_dir_all(&sock_dir);
        Ok(())
    }
}

#[cfg(unix)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    unix::run().await
}

#[cfg(not(unix))]
fn main() {
    eprintln!("This example is Unix-only (async UDS is not available on Windows in v0.2.0).");
}
