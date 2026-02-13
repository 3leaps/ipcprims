use std::time::Duration;

use ipcprims_peer::PeerError;
use ipcprims_peer::{connect_with_config, HandshakeConfig};
use serde::Serialize;

use crate::cmd::InfoArgs;
use crate::exit::{peer_error, CliError, CliResult, SUCCESS, USAGE};
use crate::output::{channel_name, OutputFormat};

#[derive(Serialize)]
struct ChannelInfo {
    id: u16,
    name: &'static str,
}

#[derive(Serialize)]
struct PeerCreds {
    uid: u32,
    gid: u32,
    pid: u32,
}

#[derive(Serialize)]
struct InfoOutput {
    schema_id: &'static str,
    peer_id: String,
    protocol_version: String,
    channels: Vec<ChannelInfo>,
    ping_latency_ms: Option<f64>,
    peer_credentials: Option<PeerCreds>,
    connected: bool,
}

pub fn run(args: InfoArgs, format: OutputFormat) -> CliResult<i32> {
    let timeout = parse_timeout(&args.timeout)?;
    let handshake_config = HandshakeConfig {
        timeout,
        ..HandshakeConfig::default()
    };

    // Request built-in channels; server returns negotiated intersection.
    let requested_channels = [1, 2, 3, 4];
    let mut peer =
        connect_with_timeout(&args.path, &requested_channels, &handshake_config, timeout)?;

    let channels: Vec<ChannelInfo> = peer
        .channels()
        .iter()
        .copied()
        .map(|id| ChannelInfo {
            id,
            name: channel_name(id),
        })
        .collect();

    let ping_latency_ms = peer
        .ping()
        .ok()
        .map(|d| (d.as_secs_f64() * 1000.0 * 100.0).round() / 100.0);

    let peer_credentials =
        peer.peer_credentials()
            .map(|(uid, gid, pid)| PeerCreds { uid, gid, pid });

    let out = InfoOutput {
        schema_id: "https://schemas.3leaps.dev/ipcprims/cli/v1/connection-info.schema.json",
        peer_id: peer.id().to_string(),
        protocol_version: peer.handshake_result().protocol_version.clone(),
        channels,
        ping_latency_ms,
        peer_credentials,
        connected: true,
    };

    print_info(&out, format);
    Ok(SUCCESS)
}

fn connect_with_timeout(
    path: &std::path::Path,
    channels: &[u16],
    handshake_config: &HandshakeConfig,
    timeout: Duration,
) -> CliResult<ipcprims_peer::Peer> {
    let start = std::time::Instant::now();
    loop {
        match connect_with_config(path, channels, handshake_config, None, None) {
            Ok(peer) => return Ok(peer),
            Err(err) => {
                if !is_retryable_connect_error(&err) {
                    return Err(peer_error("connect failed", err));
                }
                if start.elapsed() >= timeout {
                    return Err(CliError::new(
                        crate::exit::TIMEOUT,
                        format!("connect timed out after {timeout:?}"),
                    ));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

fn is_retryable_connect_error(err: &PeerError) -> bool {
    match err {
        PeerError::Transport(ipcprims_transport::TransportError::Connect { source, .. }) => {
            source.kind() == std::io::ErrorKind::NotFound
                || source.kind() == std::io::ErrorKind::ConnectionRefused
        }
        _ => false,
    }
}

fn print_info(out: &InfoOutput, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string(out).unwrap_or_else(|_| "{}".to_string())
            );
        }
        OutputFormat::Table | OutputFormat::Pretty => {
            println!("Connection Info:");
            println!("  Peer ID:          {}", out.peer_id);
            println!("  Protocol:         ipcprims {}", out.protocol_version);
            let chans = out
                .channels
                .iter()
                .map(|c| format!("{} ({})", c.name, c.id))
                .collect::<Vec<_>>()
                .join(", ");
            println!("  Channels:         {}", chans);
            match out.ping_latency_ms {
                Some(ms) => println!("  Ping:             {ms:.2}ms"),
                None => println!("  Ping:             unavailable"),
            }
            match &out.peer_credentials {
                Some(c) => println!(
                    "  Peer credentials: uid={} gid={} pid={}",
                    c.uid, c.gid, c.pid
                ),
                None => println!("  Peer credentials: unavailable"),
            }
        }
        OutputFormat::Raw => {
            println!("{}", out.peer_id);
        }
    }
}

fn parse_timeout(input: &str) -> CliResult<Duration> {
    let input = input.trim();
    if input.is_empty() {
        return Err(CliError::new(USAGE, "timeout must not be empty"));
    }

    let (number, unit) = if let Some(num) = input.strip_suffix("ms") {
        (num, "ms")
    } else if let Some(num) = input.strip_suffix('s') {
        (num, "s")
    } else {
        (input, "s")
    };

    let value: u64 = number
        .parse()
        .map_err(|_| CliError::new(USAGE, format!("invalid timeout value: {input}")))?;

    if value == 0 {
        return Err(CliError::new(USAGE, "timeout must be greater than zero"));
    }

    match unit {
        "ms" => Ok(Duration::from_millis(value)),
        "s" => Ok(Duration::from_secs(value)),
        _ => Err(CliError::new(
            USAGE,
            format!("unsupported timeout unit: {unit}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_timeout_seconds() {
        assert_eq!(parse_timeout("5s").unwrap(), Duration::from_secs(5));
        assert_eq!(parse_timeout("2").unwrap(), Duration::from_secs(2));
    }

    #[test]
    fn parse_timeout_millis() {
        assert_eq!(parse_timeout("150ms").unwrap(), Duration::from_millis(150));
    }

    #[test]
    fn parse_timeout_invalid() {
        assert!(parse_timeout("0s").is_err());
        assert!(parse_timeout("bad").is_err());
    }
}
