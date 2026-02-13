use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use ipcprims_frame::ERROR;
use ipcprims_peer::PeerListener;
#[cfg(feature = "schema")]
use ipcprims_schema::{RegistryConfig, SchemaRegistry};

use crate::cmd::EchoArgs;
use crate::exit::{peer_error, CliError, CliResult, SUCCESS};
use crate::output::{channel_name, OutputFormat};

enum RecvErrorDisposition {
    Break,
    ContinueWithError(Vec<u8>),
    Fatal(CliError),
}

pub fn run(args: EchoArgs, _format: OutputFormat) -> CliResult<i32> {
    let mut listener =
        PeerListener::bind(&args.path).map_err(|err| peer_error("bind failed", err))?;

    if let Some(channels) = &args.channels {
        listener = listener.with_channels(channels);
    }

    #[cfg(feature = "schema")]
    if let Some(dir) = &args.validate {
        let registry = SchemaRegistry::from_directory_with_config(
            dir,
            RegistryConfig {
                strict_mode: true,
                fail_on_missing_schema: false,
                ..RegistryConfig::default()
            },
        )
        .map_err(|err| {
            CliError::new(
                crate::exit::DATA_INVALID,
                format!("schema load failed: {err}"),
            )
        })?;
        listener = listener.with_schema_registry(std::sync::Arc::new(registry));
    }

    let running = Arc::new(AtomicBool::new(true));
    install_ctrlc_handler(running.clone())?;

    while running.load(Ordering::SeqCst) {
        let mut peer = match listener.accept() {
            Ok(peer) => peer,
            Err(err) => return Err(peer_error("accept failed", err)),
        };

        while running.load(Ordering::SeqCst) {
            let frame = match peer.recv() {
                Ok(frame) => frame,
                Err(err) => match classify_recv_error(err) {
                    RecvErrorDisposition::Break => break,
                    RecvErrorDisposition::ContinueWithError(payload) => {
                        if let Err(send_err) = peer.send(ERROR, &payload) {
                            tracing::warn!(error = %send_err, "failed sending schema error response");
                        }
                        continue;
                    }
                    RecvErrorDisposition::Fatal(cli_err) => return Err(cli_err),
                },
            };

            if let Some(channels) = &args.channels {
                if !channels.contains(&frame.channel) {
                    continue;
                }
            }

            tracing::info!(
                channel = frame.channel,
                channel_name = channel_name(frame.channel),
                size = frame.payload.len(),
                "echoing frame"
            );

            peer.send(frame.channel, frame.payload.as_ref())
                .map_err(|err| peer_error("echo send failed", err))?;
        }
    }

    Ok(SUCCESS)
}

fn install_ctrlc_handler(running: Arc<AtomicBool>) -> CliResult<()> {
    ctrlc::set_handler(move || {
        running.store(false, Ordering::SeqCst);
    })
    .map_err(|err| {
        CliError::new(
            crate::exit::INTERNAL,
            format!("signal handler setup failed: {err}"),
        )
    })
}

fn classify_recv_error(err: ipcprims_peer::PeerError) -> RecvErrorDisposition {
    if matches!(err, ipcprims_peer::PeerError::Disconnected(_)) {
        return RecvErrorDisposition::Break;
    }
    #[cfg(feature = "schema")]
    if let ipcprims_peer::PeerError::Schema(schema_err) = err {
        return RecvErrorDisposition::ContinueWithError(schema_error_payload(schema_err));
    }
    RecvErrorDisposition::Fatal(peer_error("receive failed", err))
}

#[cfg(feature = "schema")]
fn schema_error_payload(err: ipcprims_schema::SchemaError) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "error": format!("schema validation error: {err}")
    }))
    .unwrap_or_else(|_| b"{\"error\":\"schema validation failed\"}".to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ipcprims_peer::PeerError;

    #[test]
    fn disconnected_error_breaks_loop() {
        let disposition = classify_recv_error(PeerError::Disconnected("closed".to_string()));
        assert!(matches!(disposition, RecvErrorDisposition::Break));
    }

    #[cfg(feature = "schema")]
    #[test]
    fn schema_error_produces_error_payload_and_continues() {
        let disposition = classify_recv_error(PeerError::Schema(
            ipcprims_schema::SchemaError::ValidationFailed {
                channel: 1,
                message: "bad payload".to_string(),
            },
        ));

        let payload = match disposition {
            RecvErrorDisposition::ContinueWithError(payload) => payload,
            _ => panic!("expected continue disposition"),
        };

        let value: serde_json::Value =
            serde_json::from_slice(&payload).expect("payload should be valid json");
        assert_eq!(
            value["error"],
            "schema validation error: validation failed on channel 1: bad payload"
        );
    }

    #[test]
    fn non_schema_error_is_fatal() {
        let disposition =
            classify_recv_error(PeerError::Timeout(std::time::Duration::from_secs(1)));
        assert!(matches!(disposition, RecvErrorDisposition::Fatal(_)));
    }
}
