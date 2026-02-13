use std::fs;
use std::time::Duration;

use ipcprims_frame::{Frame, ERROR};
use ipcprims_peer::{connect_with_config, HandshakeConfig, PeerConfig};

use crate::cmd::SendArgs;
use crate::exit::{peer_error, CliError, CliResult, SUCCESS, USAGE};
use crate::output::{print_frame, OutputFormat};

pub fn run(args: SendArgs, format: OutputFormat) -> CliResult<i32> {
    let wait_timeout = parse_duration(&args.wait_timeout)?;
    let peer_config = PeerConfig {
        shutdown_timeout: wait_timeout,
        ..PeerConfig::default()
    };
    let mut requested_channels = vec![args.channel];
    if args.wait && args.channel != ERROR {
        requested_channels.push(ERROR);
    }
    let mut peer = connect_with_config(
        &args.path,
        &requested_channels,
        &HandshakeConfig::default(),
        None,
        Some(peer_config),
    )
    .map_err(|err| peer_error("connect failed", err))?;

    let payload = resolve_payload(&args)?;
    peer.send(args.channel, &payload)
        .map_err(|err| peer_error("send failed", err))?;

    if args.wait {
        let frame = wait_for_response(&mut peer, args.channel)
            .map_err(|err| peer_error("receive failed", err))?;
        print_frame(&frame, peer.id(), format);
    }

    Ok(SUCCESS)
}

fn resolve_payload(args: &SendArgs) -> CliResult<Vec<u8>> {
    if let Some(json) = &args.json {
        serde_json::from_str::<serde_json::Value>(json)
            .map_err(|err| CliError::new(USAGE, format!("--json is not valid JSON: {err}")))?;
        return Ok(json.as_bytes().to_vec());
    }
    if let Some(data) = &args.data {
        return Ok(data.as_bytes().to_vec());
    }
    if let Some(path) = &args.file {
        return fs::read(path).map_err(|err| {
            crate::exit::io_error(&format!("failed reading {}", path.display()), err)
        });
    }
    Ok(Vec::new())
}

fn parse_duration(input: &str) -> CliResult<Duration> {
    let input = input.trim();
    if input.is_empty() {
        return Err(CliError::new(USAGE, "duration must not be empty"));
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
        .map_err(|_| CliError::new(USAGE, format!("invalid duration value: {input}")))?;

    if value == 0 {
        return Err(CliError::new(USAGE, "duration must be greater than zero"));
    }

    match unit {
        "ms" => Ok(Duration::from_millis(value)),
        "s" => Ok(Duration::from_secs(value)),
        _ => Err(CliError::new(
            USAGE,
            format!("unsupported duration unit: {unit}"),
        )),
    }
}

trait ResponseReceiver {
    fn recv_on_channel(&mut self, channel: u16) -> Result<Frame, ipcprims_peer::PeerError>;
}

impl ResponseReceiver for ipcprims_peer::Peer {
    fn recv_on_channel(&mut self, channel: u16) -> Result<Frame, ipcprims_peer::PeerError> {
        self.recv_on(channel)
    }
}

fn wait_for_response<R: ResponseReceiver>(
    receiver: &mut R,
    channel: u16,
) -> Result<Frame, ipcprims_peer::PeerError> {
    match receiver.recv_on_channel(channel) {
        Ok(frame) => Ok(frame),
        Err(ipcprims_peer::PeerError::Timeout(_)) if channel != ERROR => {
            receiver.recv_on_channel(ERROR)
        }
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockReceiver {
        called_channel: Option<u16>,
        calls: usize,
    }

    impl ResponseReceiver for MockReceiver {
        fn recv_on_channel(&mut self, channel: u16) -> Result<Frame, ipcprims_peer::PeerError> {
            self.calls += 1;
            self.called_channel = Some(channel);
            if self.calls == 1 {
                return Err(ipcprims_peer::PeerError::Timeout(Duration::from_secs(1)));
            }
            Ok(Frame::new(channel, b"ok".to_vec()))
        }
    }

    #[test]
    fn wait_for_response_falls_back_to_error_channel() {
        let mut receiver = MockReceiver {
            called_channel: None,
            calls: 0,
        };
        let frame = wait_for_response(&mut receiver, 7).expect("wait should succeed");
        assert_eq!(receiver.called_channel, Some(ERROR));
        assert_eq!(frame.channel, ERROR);
    }

    #[test]
    fn parse_duration_seconds_and_millis() {
        assert_eq!(parse_duration("2s").unwrap(), Duration::from_secs(2));
        assert_eq!(parse_duration("150ms").unwrap(), Duration::from_millis(150));
        assert_eq!(parse_duration("3").unwrap(), Duration::from_secs(3));
    }

    #[test]
    fn parse_duration_rejects_invalid_values() {
        assert!(parse_duration("0s").is_err());
        assert!(parse_duration("bad").is_err());
    }
}
