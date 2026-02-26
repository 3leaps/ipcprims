//! Async gateway loop — accept peers and process arrival-ordered frames (Tokio, Unix-only).
//!
//! Run with:
//!   cargo run --example async-gateway --features async
//!
//! This illustrates the intended consumption pattern for multi-peer services:
//! - Accept peers via `AsyncPeerListener`
//! - Read frames in arrival order via `AnyReceiver`
//! - Use an external `CancellationToken` for structured shutdown

#[cfg(unix)]
mod unix {
    use std::collections::HashMap;
    use std::fs;
    use std::sync::Arc;

    use ipcprims::peer::AsyncPeerListener;
    use tokio::sync::Mutex;
    use tokio_util::sync::CancellationToken;

    pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
        let cancel = CancellationToken::new();
        let cancel_for_ctrlc = cancel.clone();

        // Best-effort Ctrl-C handler.
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            cancel_for_ctrlc.cancel();
        });

        // macOS UDS paths have strict length limits; keep the path short by anchoring in /tmp.
        let sock_dir = std::path::PathBuf::from("/tmp")
            .join(format!("ipcprims-async-gateway-{}", std::process::id()));
        fs::create_dir_all(&sock_dir)?;
        let sock_path = sock_dir.join("gateway.sock");
        let _ = fs::remove_file(&sock_path);

        let listener = AsyncPeerListener::bind(&sock_path)?.with_cancellation_token(cancel.clone());
        eprintln!("Gateway listening on {}", sock_path.display());

        // Track peers by id for optional broadcast-like patterns.
        let peers: Arc<Mutex<HashMap<String, ipcprims::peer::AsyncPeerTx>>> =
            Arc::new(Mutex::new(HashMap::new()));

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    eprintln!("shutdown: cancellation requested");
                    break;
                }
                accepted = listener.accept() => {
                    let peer = accepted?;
                    let id = peer.id().to_string();
                    eprintln!("[gateway] accepted {id}");

                    let (tx, mut rx) = peer.into_split();
                    peers.lock().await.insert(id.clone(), tx.clone());

                    let peers = peers.clone();
                    tokio::spawn(async move {
                        let mut any = rx.take_any_receiver();
                        loop {
                            match any.recv().await {
                                Ok(frame) => {
                                    eprintln!("[peer={id}] channel={} bytes={}", frame.channel, frame.payload.len());
                                    // Example: echo back on same channel.
                                    if let Some(tx) = peers.lock().await.get(&id).cloned() {
                                        let _ = tx.send(frame.channel, &frame.payload).await;
                                    }
                                }
                                Err(e) => {
                                    eprintln!("[peer={id}] disconnected: {e}");
                                    break;
                                }
                            }
                        }
                        peers.lock().await.remove(&id);
                    });
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
