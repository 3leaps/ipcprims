//! Minimal echo server — accepts one peer and echoes messages back.
//!
//! Run with:
//!   cargo run --example echo-server --features peer
//!
//! In another terminal:
//!   cargo run --features cli -- send /tmp/ipcprims-echo-example.sock \
//!     --channel 1 --json '{"hello":"world"}' --wait --wait-timeout 3

use std::fs;

use ipcprims::peer::PeerListener;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sock_dir = std::env::temp_dir().join(format!("ipcprims-echo-{}", std::process::id()));
    fs::create_dir_all(&sock_dir)?;
    let sock_path = sock_dir.join("echo.sock");

    // Ensure no stale socket
    let _ = fs::remove_file(&sock_path);

    let listener = PeerListener::bind(&sock_path)?;
    eprintln!("Listening on {}", sock_path.display());

    // Accept one peer and echo messages until disconnect.
    let mut peer = listener.accept()?;
    eprintln!("Peer connected: {}", peer.id());

    loop {
        match peer.recv() {
            Ok(frame) => {
                eprintln!(
                    "Received {} bytes on channel {}",
                    frame.payload.len(),
                    frame.channel
                );
                peer.send(frame.channel, &frame.payload)?;
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
