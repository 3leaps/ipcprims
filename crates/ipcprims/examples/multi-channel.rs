//! Multi-channel example — demonstrates sending on COMMAND and DATA channels.
//!
//! Run with:
//!   cargo run --example multi-channel --features peer

use std::fs;
use std::thread;

use ipcprims::frame::{COMMAND, DATA};
use ipcprims::peer::{connect, PeerListener};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sock_dir = std::env::temp_dir().join(format!("ipcprims-multi-{}", std::process::id()));
    fs::create_dir_all(&sock_dir)?;
    let sock_path = sock_dir.join("multi.sock");
    let _ = fs::remove_file(&sock_path);

    let listener = PeerListener::bind(&sock_path)?.with_channels(&[COMMAND, DATA]);

    let path_for_client = sock_path.clone();
    let server = thread::spawn(
        move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let mut peer = listener.accept()?;
            eprintln!("[server] peer connected: {}", peer.id());

            // Receive two messages on different channels
            for _ in 0..2 {
                let frame = peer.recv()?;
                let channel_name = match frame.channel {
                    1 => "COMMAND",
                    2 => "DATA",
                    _ => "UNKNOWN",
                };
                eprintln!(
                    "[server] channel={channel_name} payload={}",
                    String::from_utf8_lossy(&frame.payload)
                );
                peer.send(frame.channel, &frame.payload)?;
            }
            Ok(())
        },
    );

    let mut client = connect(&path_for_client, &[COMMAND, DATA])?;

    // Send a command
    client.send(COMMAND, b"{\"action\":\"ping\"}")?;
    let resp = client.recv_on(COMMAND)?;
    eprintln!(
        "[client] COMMAND response: {}",
        String::from_utf8_lossy(&resp.payload)
    );

    // Send data
    client.send(DATA, b"bulk payload bytes here")?;
    let resp = client.recv_on(DATA)?;
    eprintln!(
        "[client] DATA response: {}",
        String::from_utf8_lossy(&resp.payload)
    );

    server
        .join()
        .expect("server thread should not panic")
        .expect("server should complete without error");
    let _ = fs::remove_dir_all(&sock_dir);
    Ok(())
}
