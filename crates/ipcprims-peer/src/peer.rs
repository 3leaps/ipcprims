use std::collections::{HashMap, VecDeque};
use std::io::ErrorKind;
use std::time::{Duration, Instant};

use ipcprims_frame::{Frame, FrameError, FrameReader, FrameWriter, COMMAND, CONTROL};
use ipcprims_transport::IpcStream;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::control::{
    ControlMessage, CONTROL_PING, CONTROL_PONG, CONTROL_SHUTDOWN_ACK, CONTROL_SHUTDOWN_FORCE,
    CONTROL_SHUTDOWN_REQUEST,
};
use crate::error::{PeerError, Result};
use crate::handshake::HandshakeResult;

#[cfg(feature = "schema")]
use ipcprims_schema::SchemaRegistry;
#[cfg(feature = "schema")]
use std::sync::Arc;
#[cfg(feature = "schema")]
pub(crate) type SchemaRegistryHandle = Arc<SchemaRegistry>;
#[cfg(not(feature = "schema"))]
pub(crate) type SchemaRegistryHandle = std::sync::Arc<()>;

/// Peer behavior configuration.
#[derive(Debug, Clone)]
pub struct PeerConfig {
    /// Max buffered frames per channel for `recv_on`.
    /// Together with `max_total_buffered_bytes`, this bounds off-channel buffering memory.
    pub max_buffer_per_channel: usize,
    /// Max total buffered payload+header bytes across all buffered channels.
    /// Worst-case queued frame memory is capped at this value.
    pub max_total_buffered_bytes: usize,
    /// Timeout for CONTROL operations (shutdown, ping).
    pub shutdown_timeout: Duration,
    /// Maximum CONTROL frames processed in a single receive/wait loop before disconnecting.
    pub max_control_frames_per_loop: usize,
    /// Whether remote SHUTDOWN_FORCE messages are honored.
    pub allow_shutdown_force: bool,
    /// Whether unknown CONTROL message types are passed through instead of rejected.
    pub allow_unknown_control_messages: bool,
}

impl Default for PeerConfig {
    fn default() -> Self {
        Self {
            max_buffer_per_channel: 256,
            max_total_buffered_bytes: 16 * 1024 * 1024,
            shutdown_timeout: Duration::from_secs(5),
            max_control_frames_per_loop: 256,
            allow_shutdown_force: false,
            allow_unknown_control_messages: false,
        }
    }
}

/// A connected, handshaken peer.
pub struct Peer {
    id: String,
    reader: FrameReader<IpcStream>,
    writer: FrameWriter<IpcStream>,
    handshake_result: HandshakeResult,
    client_auth_token: Option<String>,
    schema_registry: Option<SchemaRegistryHandle>,
    channel_buffers: HashMap<u16, VecDeque<Frame>>,
    buffered_total_bytes: usize,
    config: PeerConfig,
    shutdown_requested: bool,
}

impl Peer {
    /// Create a peer from connected components.
    pub(crate) fn from_parts(
        id: String,
        reader: FrameReader<IpcStream>,
        writer: FrameWriter<IpcStream>,
        mut handshake_result: HandshakeResult,
        schema_registry: Option<SchemaRegistryHandle>,
        config: PeerConfig,
    ) -> Self {
        let _ = reader
            .get_ref()
            .set_read_timeout(Some(config.shutdown_timeout));
        let _ = writer
            .get_ref()
            .set_write_timeout(Some(config.shutdown_timeout));

        let client_auth_token = handshake_result.client_auth_token.take();

        Self {
            id,
            reader,
            writer,
            handshake_result,
            client_auth_token,
            schema_registry,
            channel_buffers: HashMap::new(),
            buffered_total_bytes: 0,
            config,
            shutdown_requested: false,
        }
    }

    /// Send bytes on a negotiated channel.
    pub fn send(&mut self, channel: u16, payload: &[u8]) -> Result<()> {
        if channel != CONTROL && !self.supports_channel(channel) {
            return Err(PeerError::UnsupportedChannel(channel));
        }

        self.validate_send(channel, payload)?;
        self.writer.send(channel, payload)?;
        Ok(())
    }

    /// Send JSON value on a channel.
    pub fn send_json<T: Serialize>(&mut self, channel: u16, value: &T) -> Result<()> {
        let payload = serde_json::to_vec(value)?;
        self.send(channel, &payload)
    }

    /// Receive next non-internal frame from any channel.
    pub fn recv(&mut self) -> Result<Frame> {
        if self.shutdown_requested {
            return Err(PeerError::Disconnected("shutdown requested".to_string()));
        }

        let mut control_frames_seen = 0usize;
        loop {
            let frame = self.read_frame_once()?;
            if frame.channel != CONTROL {
                self.ensure_inbound_channel(frame.channel)?;
                self.validate_recv(&frame)?;
                return Ok(frame);
            }
            control_frames_seen = control_frames_seen.saturating_add(1);
            if control_frames_seen > self.config.max_control_frames_per_loop {
                return Err(PeerError::Disconnected(
                    "control frame flood detected".to_string(),
                ));
            }

            match self.handle_control_frame(frame)? {
                ControlDisposition::Continue => continue,
                ControlDisposition::Return(frame) => {
                    self.validate_recv(&frame)?;
                    return Ok(frame);
                }
                ControlDisposition::Disconnected(reason) => {
                    return Err(PeerError::Disconnected(reason));
                }
            }
        }
    }

    /// Receive next frame on a specific channel, buffering other channels.
    pub fn recv_on(&mut self, channel: u16) -> Result<Frame> {
        if channel != CONTROL && !self.supports_channel(channel) {
            return Err(PeerError::UnsupportedChannel(channel));
        }

        if let Some(frame) = self.pop_buffered(channel) {
            return Ok(frame);
        }

        loop {
            let frame = self.recv()?;
            if frame.channel == channel {
                return Ok(frame);
            }
            self.buffer_frame(frame)?;
        }
    }

    /// Send a COMMAND request and wait for COMMAND response.
    ///
    /// This helper is intended for a single in-flight request/response flow.
    /// Replay protection, idempotency, and correlation matching are consumer
    /// policy and should be implemented in message payloads.
    pub fn request(&mut self, payload: &[u8]) -> Result<Frame> {
        self.send(COMMAND, payload)?;
        self.recv_on(COMMAND)
    }

    /// Send JSON request and deserialize JSON response.
    ///
    /// The same ordering/replay considerations as [`Self::request`] apply.
    pub fn request_json<T: Serialize, R: DeserializeOwned>(&mut self, value: &T) -> Result<R> {
        self.send_json(COMMAND, value)?;
        let frame = self.recv_on(COMMAND)?;
        Ok(serde_json::from_slice(frame.payload.as_ref())?)
    }

    /// Send ping and wait for pong.
    pub fn ping(&mut self) -> Result<Duration> {
        self.reader
            .get_ref()
            .set_read_timeout(Some(self.config.shutdown_timeout))?;

        let start = Instant::now();
        self.send_control(ControlMessage::ping())?;

        let deadline = start + self.config.shutdown_timeout;
        self.wait_for_control_message(CONTROL_PONG, deadline)?;
        Ok(start.elapsed())
    }

    /// Graceful shutdown.
    pub fn shutdown(mut self) -> Result<()> {
        self.reader
            .get_ref()
            .set_read_timeout(Some(self.config.shutdown_timeout))?;

        self.send_control(ControlMessage::shutdown_request(None))?;
        let deadline = Instant::now() + self.config.shutdown_timeout;
        match self.wait_for_control_message(CONTROL_SHUTDOWN_ACK, deadline) {
            Ok(()) => Ok(()),
            Err(PeerError::Timeout(_)) => Err(PeerError::ShutdownFailed(
                "timed out waiting for shutdown acknowledgement".to_string(),
            )),
            Err(err) => Err(PeerError::ShutdownFailed(err.to_string())),
        }
    }

    /// Peer identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Negotiated channels.
    pub fn channels(&self) -> &[u16] {
        &self.handshake_result.negotiated_channels
    }

    /// True if the channel was negotiated.
    pub fn supports_channel(&self, channel: u16) -> bool {
        self.handshake_result.negotiated_channels.contains(&channel)
    }

    /// Handshake metadata.
    pub fn handshake_result(&self) -> &HandshakeResult {
        &self.handshake_result
    }

    /// Client auth token observed during handshake, if present.
    pub fn client_auth_token(&self) -> Option<&str> {
        self.client_auth_token.as_deref()
    }

    /// Take and clear the client auth token from this peer.
    pub fn take_client_auth_token(&mut self) -> Option<String> {
        self.client_auth_token.take()
    }

    /// Connected peer credentials, when available on this platform.
    pub fn peer_credentials(&self) -> Option<(u32, u32, u32)> {
        self.reader.get_ref().peer_credentials()
    }

    fn send_control(&mut self, message: ControlMessage) -> Result<()> {
        let payload = serde_json::to_vec(&message)?;
        self.writer.send(CONTROL, &payload)?;
        Ok(())
    }

    fn read_frame_once(&mut self) -> Result<Frame> {
        match self.reader.read_frame() {
            Ok(frame) => Ok(frame),
            Err(FrameError::ConnectionClosed) => {
                Err(PeerError::Disconnected("connection closed".to_string()))
            }
            Err(FrameError::Io(err))
                if err.kind() == ErrorKind::WouldBlock || err.kind() == ErrorKind::TimedOut =>
            {
                Err(PeerError::Timeout(self.config.shutdown_timeout))
            }
            Err(err) => Err(PeerError::Frame(err)),
        }
    }

    fn handle_control_frame(&mut self, frame: Frame) -> Result<ControlDisposition> {
        let message = match serde_json::from_slice::<ControlMessage>(frame.payload.as_ref()) {
            Ok(message) => message,
            Err(_) => {
                return Ok(ControlDisposition::Disconnected(
                    "invalid CONTROL JSON payload".to_string(),
                ));
            }
        };

        match message.msg_type.as_str() {
            CONTROL_PING => {
                self.send_control(ControlMessage::pong())?;
                Ok(ControlDisposition::Continue)
            }
            CONTROL_PONG => Ok(ControlDisposition::Continue),
            CONTROL_SHUTDOWN_REQUEST => {
                self.send_control(ControlMessage::shutdown_ack())?;
                self.shutdown_requested = true;
                Ok(ControlDisposition::Continue)
            }
            CONTROL_SHUTDOWN_FORCE => {
                if self.config.allow_shutdown_force {
                    self.shutdown_requested = true;
                    Ok(ControlDisposition::Disconnected(
                        "force shutdown".to_string(),
                    ))
                } else {
                    Ok(ControlDisposition::Disconnected(
                        "received disallowed SHUTDOWN_FORCE".to_string(),
                    ))
                }
            }
            _ if self.config.allow_unknown_control_messages => {
                Ok(ControlDisposition::Return(frame))
            }
            _ => Ok(ControlDisposition::Disconnected(
                "unknown CONTROL message type".to_string(),
            )),
        }
    }

    fn wait_for_control_message(&mut self, expected: &str, deadline: Instant) -> Result<()> {
        let mut control_frames_seen = 0usize;
        loop {
            if Instant::now() >= deadline {
                return Err(PeerError::Timeout(self.config.shutdown_timeout));
            }

            let frame = match self.reader.read_frame() {
                Ok(frame) => frame,
                Err(FrameError::Io(err))
                    if err.kind() == ErrorKind::WouldBlock || err.kind() == ErrorKind::TimedOut =>
                {
                    continue;
                }
                Err(FrameError::ConnectionClosed) => {
                    return Err(PeerError::Disconnected("connection closed".to_string()));
                }
                Err(err) => return Err(PeerError::Frame(err)),
            };

            if frame.channel != CONTROL {
                self.ensure_inbound_channel(frame.channel)?;
                self.validate_recv(&frame)?;
                self.buffer_frame(frame)?;
                continue;
            }
            control_frames_seen = control_frames_seen.saturating_add(1);
            if control_frames_seen > self.config.max_control_frames_per_loop {
                return Err(PeerError::Disconnected(
                    "control frame flood detected".to_string(),
                ));
            }

            let parsed = serde_json::from_slice::<ControlMessage>(frame.payload.as_ref());
            let message = match parsed {
                Ok(message) => message,
                Err(_) => {
                    return Err(PeerError::Disconnected(
                        "invalid CONTROL JSON payload".to_string(),
                    ));
                }
            };

            match message.msg_type.as_str() {
                msg if msg == expected => return Ok(()),
                CONTROL_PING => {
                    self.send_control(ControlMessage::pong())?;
                }
                CONTROL_SHUTDOWN_REQUEST => {
                    self.send_control(ControlMessage::shutdown_ack())?;
                    self.shutdown_requested = true;
                    return Err(PeerError::Disconnected("shutdown requested".to_string()));
                }
                CONTROL_SHUTDOWN_FORCE => {
                    if self.config.allow_shutdown_force {
                        self.shutdown_requested = true;
                        return Err(PeerError::Disconnected("force shutdown".to_string()));
                    }
                    return Err(PeerError::Disconnected(
                        "received disallowed SHUTDOWN_FORCE".to_string(),
                    ));
                }
                _ if self.config.allow_unknown_control_messages => {
                    self.buffer_frame(frame)?;
                }
                _ => {
                    return Err(PeerError::Disconnected(
                        "unknown CONTROL message type".to_string(),
                    ));
                }
            }
        }
    }

    fn buffer_frame(&mut self, frame: Frame) -> Result<()> {
        let frame_bytes = frame.wire_size();
        if self.buffered_total_bytes.saturating_add(frame_bytes)
            > self.config.max_total_buffered_bytes
        {
            return Err(PeerError::BufferFull(frame.channel));
        }

        let queue = self.channel_buffers.entry(frame.channel).or_default();
        if queue.len() >= self.config.max_buffer_per_channel {
            return Err(PeerError::BufferFull(frame.channel));
        }
        self.buffered_total_bytes = self.buffered_total_bytes.saturating_add(frame_bytes);
        queue.push_back(frame);
        Ok(())
    }

    fn pop_buffered(&mut self, channel: u16) -> Option<Frame> {
        let queue = self.channel_buffers.get_mut(&channel)?;
        let frame = queue.pop_front();
        if let Some(frame) = &frame {
            self.buffered_total_bytes = self.buffered_total_bytes.saturating_sub(frame.wire_size());
        }
        if queue.is_empty() {
            self.channel_buffers.remove(&channel);
        }
        frame
    }

    fn ensure_inbound_channel(&self, channel: u16) -> Result<()> {
        if self.supports_channel(channel) {
            return Ok(());
        }
        Err(PeerError::Disconnected(format!(
            "received frame on unnegotiated channel {channel}"
        )))
    }

    #[cfg(feature = "schema")]
    fn validate_send(&self, channel: u16, payload: &[u8]) -> Result<()> {
        if let Some(registry) = &self.schema_registry {
            registry.validate(channel, payload)?;
        }
        Ok(())
    }

    #[cfg(not(feature = "schema"))]
    fn validate_send(&self, _channel: u16, _payload: &[u8]) -> Result<()> {
        let _ = &self.schema_registry;
        Ok(())
    }

    #[cfg(feature = "schema")]
    fn validate_recv(&self, frame: &Frame) -> Result<()> {
        if let Some(registry) = &self.schema_registry {
            registry.validate_frame(frame)?;
        }
        Ok(())
    }

    #[cfg(not(feature = "schema"))]
    fn validate_recv(&self, _frame: &Frame) -> Result<()> {
        let _ = &self.schema_registry;
        Ok(())
    }
}

enum ControlDisposition {
    Continue,
    Return(Frame),
    Disconnected(String),
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc;
    use std::thread;

    use super::*;
    use crate::handshake::HandshakeResult;

    static TEST_SOCK_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn make_connected_ipc_pair() -> (ipcprims_transport::IpcStream, ipcprims_transport::IpcStream) {
        let unique = TEST_SOCK_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::path::PathBuf::from(format!(
            "/tmp/icpp-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be after epoch")
                .as_nanos(),
            unique
        ));
        std::fs::create_dir_all(&dir).expect("temp dir should be creatable");
        let sock_path = dir.join("peer.sock");

        let listener =
            ipcprims_transport::UnixDomainSocket::bind(&sock_path).expect("listener should bind");
        let path_clone = sock_path.clone();
        let connector = thread::spawn(move || {
            ipcprims_transport::UnixDomainSocket::connect(path_clone)
                .expect("client should connect")
        });

        let server = listener.accept().expect("listener should accept");
        let client = connector.join().expect("connector thread should finish");
        let _ = std::fs::remove_dir_all(&dir);

        (server, client)
    }

    fn peer_pair(config: PeerConfig) -> (Peer, Peer) {
        let (left, right) = make_connected_ipc_pair();

        let left_reader = FrameReader::with_config_ipc(
            left.try_clone().unwrap(),
            ipcprims_frame::FrameConfig {
                read_timeout: Some(config.shutdown_timeout),
                write_timeout: Some(config.shutdown_timeout),
                ..Default::default()
            },
        )
        .unwrap();
        let left_writer = FrameWriter::with_config_ipc(
            left,
            ipcprims_frame::FrameConfig {
                read_timeout: Some(config.shutdown_timeout),
                write_timeout: Some(config.shutdown_timeout),
                ..Default::default()
            },
        )
        .unwrap();

        let right_reader = FrameReader::with_config_ipc(
            right.try_clone().unwrap(),
            ipcprims_frame::FrameConfig {
                read_timeout: Some(config.shutdown_timeout),
                write_timeout: Some(config.shutdown_timeout),
                ..Default::default()
            },
        )
        .unwrap();
        let right_writer = FrameWriter::with_config_ipc(
            right,
            ipcprims_frame::FrameConfig {
                read_timeout: Some(config.shutdown_timeout),
                write_timeout: Some(config.shutdown_timeout),
                ..Default::default()
            },
        )
        .unwrap();

        let handshake = HandshakeResult {
            peer_id: "peer".to_string(),
            protocol_version: "1.0".to_string(),
            negotiated_channels: vec![1, 2, 3, 4],
            client_auth_token: None,
        };

        let a = Peer::from_parts(
            "left".to_string(),
            left_reader,
            left_writer,
            handshake.clone(),
            None,
            config.clone(),
        );
        let b = Peer::from_parts(
            "right".to_string(),
            right_reader,
            right_writer,
            handshake,
            None,
            config,
        );

        (a, b)
    }

    #[test]
    fn send_recv() {
        let config = PeerConfig::default();
        let (mut a, mut b) = peer_pair(config);

        a.send(1, b"hello").unwrap();
        let frame = b.recv().unwrap();
        assert_eq!(frame.channel, 1);
        assert_eq!(frame.payload.as_ref(), b"hello");
    }

    #[test]
    fn send_json_and_parse() {
        #[derive(serde::Serialize, serde::Deserialize)]
        struct Msg {
            n: u32,
        }

        let config = PeerConfig::default();
        let (mut a, mut b) = peer_pair(config);

        a.send_json(1, &Msg { n: 7 }).unwrap();
        let frame = b.recv().unwrap();
        let parsed: Msg = serde_json::from_slice(frame.payload.as_ref()).unwrap();
        assert_eq!(parsed.n, 7);
    }

    #[test]
    fn recv_on_buffers_other_channels() {
        let config = PeerConfig::default();
        let (mut a, mut b) = peer_pair(config);

        a.send(1, b"one").unwrap();
        a.send(2, b"two").unwrap();

        let two = b.recv_on(2).unwrap();
        assert_eq!(two.payload.as_ref(), b"two");

        let one = b.recv_on(1).unwrap();
        assert_eq!(one.payload.as_ref(), b"one");
    }

    #[test]
    fn recv_on_buffer_overflow() {
        let config = PeerConfig {
            max_buffer_per_channel: 1,
            max_total_buffered_bytes: 16 * 1024 * 1024,
            shutdown_timeout: Duration::from_millis(200),
            max_control_frames_per_loop: 256,
            allow_shutdown_force: false,
            allow_unknown_control_messages: false,
        };
        let (mut a, mut b) = peer_pair(config);

        a.send(1, b"a").unwrap();
        a.send(1, b"b").unwrap();
        a.send(2, b"target").unwrap();

        let err = b.recv_on(2).unwrap_err();
        assert!(matches!(err, PeerError::BufferFull(1)));
    }

    #[test]
    fn request_response_command_channel() {
        let config = PeerConfig::default();
        let (mut client, mut server) = peer_pair(config);

        let server_thread = thread::spawn(move || {
            let frame = server.recv_on(COMMAND).unwrap();
            server.send(COMMAND, frame.payload.as_ref()).unwrap();
        });

        let response = client.request(b"req").unwrap();
        assert_eq!(response.channel, COMMAND);
        assert_eq!(response.payload.as_ref(), b"req");

        server_thread.join().unwrap();
    }

    #[test]
    fn ping_pong_roundtrip() {
        let config = PeerConfig::default();
        let (mut left, mut right) = peer_pair(config);

        let responder = thread::spawn(move || {
            let frame = right.reader.read_frame().expect("should read ping frame");
            assert_eq!(frame.channel, CONTROL);
            let message: ControlMessage =
                serde_json::from_slice(frame.payload.as_ref()).expect("valid control message");
            assert_eq!(message.msg_type, CONTROL_PING);
            right
                .send_control(ControlMessage::pong())
                .expect("should send pong");
        });

        let rtt = left.ping().unwrap();
        assert!(rtt <= Duration::from_secs(1));

        responder.join().unwrap();
    }

    #[test]
    fn graceful_shutdown() {
        let config = PeerConfig::default();
        let (left, mut right) = peer_pair(config);

        let right_thread = thread::spawn(move || {
            // First recv handles shutdown request and sets shutdown flag internally.
            let err = right.recv().unwrap_err();
            assert!(matches!(err, PeerError::Disconnected(_)));
        });

        left.shutdown().unwrap();
        right_thread.join().unwrap();
    }

    #[test]
    fn shutdown_timeout() {
        let config = PeerConfig {
            max_buffer_per_channel: 256,
            max_total_buffered_bytes: 16 * 1024 * 1024,
            shutdown_timeout: Duration::from_millis(50),
            max_control_frames_per_loop: 256,
            allow_shutdown_force: false,
            allow_unknown_control_messages: false,
        };
        let (left, right) = peer_pair(config);

        let closer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(200));
            drop(right);
        });

        let result = left.shutdown();
        assert!(matches!(result, Err(PeerError::ShutdownFailed(_))));
        closer.join().unwrap();
    }

    #[test]
    fn take_client_auth_token_clears_stored_token() {
        let (mut peer, _) = peer_pair(PeerConfig::default());
        peer.client_auth_token = Some("token-123".to_string());

        assert_eq!(peer.client_auth_token(), Some("token-123"));
        let taken = peer.take_client_auth_token();
        assert_eq!(taken.as_deref(), Some("token-123"));
        assert_eq!(peer.client_auth_token(), None);
    }

    #[test]
    fn handshake_result_does_not_expose_token_material() {
        let (mut peer, _) = peer_pair(PeerConfig::default());
        peer.client_auth_token = Some("token-123".to_string());
        assert_eq!(peer.handshake_result().client_auth_token, None);
    }

    #[test]
    fn request_json_roundtrip() {
        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
        struct In {
            x: u32,
        }
        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
        struct Out {
            y: u32,
        }

        let config = PeerConfig::default();
        let (mut client, mut server) = peer_pair(config);

        let server_thread = thread::spawn(move || {
            let frame = server.recv_on(COMMAND).unwrap();
            let req: In = serde_json::from_slice(frame.payload.as_ref()).unwrap();
            server.send_json(COMMAND, &Out { y: req.x + 1 }).unwrap();
        });

        let response: Out = client.request_json(&In { x: 2 }).unwrap();
        assert_eq!(response, Out { y: 3 });
        server_thread.join().unwrap();
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn peer_credentials_linux() {
        let config = PeerConfig::default();
        let (peer, _other) = peer_pair(config);
        let creds = peer.reader.get_ref().peer_credentials();
        assert!(creds.is_some());
    }

    #[test]
    #[cfg(not(target_os = "linux"))]
    fn peer_credentials_non_linux() {
        let config = PeerConfig::default();
        let (peer, _other) = peer_pair(config);
        let creds = peer.reader.get_ref().peer_credentials();
        assert!(creds.is_none());
    }

    #[test]
    fn buffers_non_control_while_waiting_for_pong() {
        let config = PeerConfig::default();
        let (mut left, mut right) = peer_pair(config);
        let (tx, rx) = mpsc::channel();

        let right_thread = thread::spawn(move || {
            right.send(1, b"queued").unwrap();
            let ping = right.reader.read_frame().expect("should read ping frame");
            assert_eq!(ping.channel, CONTROL);
            let message: ControlMessage =
                serde_json::from_slice(ping.payload.as_ref()).expect("valid control message");
            assert_eq!(message.msg_type, CONTROL_PING);
            right.send_control(ControlMessage::pong()).unwrap();
            tx.send(()).unwrap();
        });

        let _ = left.ping().unwrap();
        let queued = left.recv_on(1).unwrap();
        assert_eq!(queued.payload.as_ref(), b"queued");

        rx.recv().unwrap();
        right_thread.join().unwrap();
    }

    #[test]
    fn recv_rejects_unnegotiated_channel() {
        let config = PeerConfig::default();
        let (mut left, mut right) = peer_pair(config);

        left.send(1, b"ok").unwrap();
        let _ = right.recv().unwrap();

        // Channel 9 is not negotiated for this pair.
        left.writer.send(9, b"bad").unwrap();
        let err = right.recv().unwrap_err();
        assert!(matches!(err, PeerError::Disconnected(_)));
    }

    #[test]
    fn recv_on_enforces_global_buffer_bytes_cap() {
        let config = PeerConfig {
            max_buffer_per_channel: 32,
            max_total_buffered_bytes: 40,
            shutdown_timeout: Duration::from_millis(200),
            max_control_frames_per_loop: 256,
            allow_shutdown_force: false,
            allow_unknown_control_messages: false,
        };
        let (mut left, mut right) = peer_pair(config);

        left.send(1, b"1234567890").unwrap(); // 18 bytes with header
        left.send(3, b"abcdefghij").unwrap(); // +18 => 36
        left.send(4, b"ZZZZZZZZZZ").unwrap(); // +18 => 54 (overflow)
        left.send(2, b"target").unwrap();

        let err = right.recv_on(2).unwrap_err();
        assert!(matches!(err, PeerError::BufferFull(4)));
    }

    #[test]
    fn shutdown_force_disallowed_by_default() {
        let config = PeerConfig::default();
        let (mut left, mut right) = peer_pair(config);

        left.send_control(ControlMessage::shutdown_force()).unwrap();
        let err = right.recv().unwrap_err();
        assert!(matches!(err, PeerError::Disconnected(_)));
    }

    #[test]
    fn invalid_control_json_disconnects() {
        let config = PeerConfig::default();
        let (mut left, mut right) = peer_pair(config);

        left.writer.send(CONTROL, b"{bad-json").unwrap();
        let err = right.recv().unwrap_err();
        assert!(matches!(err, PeerError::Disconnected(_)));
    }

    #[test]
    fn unknown_control_type_disconnects_by_default() {
        let config = PeerConfig::default();
        let (mut left, mut right) = peer_pair(config);

        left.writer
            .send(
                CONTROL,
                serde_json::to_vec(&serde_json::json!({"type":"new_control"}))
                    .unwrap()
                    .as_slice(),
            )
            .unwrap();
        let err = right.recv().unwrap_err();
        assert!(matches!(err, PeerError::Disconnected(_)));
    }

    #[test]
    fn control_flood_disconnects() {
        let config = PeerConfig {
            max_control_frames_per_loop: 4,
            ..PeerConfig::default()
        };
        let (mut left, mut right) = peer_pair(config);

        for _ in 0..8 {
            left.send_control(ControlMessage::ping()).unwrap();
        }
        let err = right.recv().unwrap_err();
        assert!(matches!(err, PeerError::Disconnected(_)));
    }

    #[cfg(feature = "schema")]
    #[test]
    fn schema_validation_send_and_recv() {
        use ipcprims_schema::SchemaRegistry;

        let config = PeerConfig::default();
        let (left, right) = make_connected_ipc_pair();

        let mut registry = SchemaRegistry::new();
        registry
            .register(
                1,
                r#"{"type":"object","properties":{"v":{"type":"integer"}},"required":["v"]}"#,
            )
            .unwrap();
        let registry = Arc::new(registry);

        let left_peer = Peer::from_parts(
            "left".to_string(),
            FrameReader::new(left.try_clone().unwrap()),
            FrameWriter::new(left),
            HandshakeResult {
                peer_id: "p".into(),
                protocol_version: "1.0".into(),
                negotiated_channels: vec![1],
                client_auth_token: None,
            },
            Some(Arc::clone(&registry)),
            config.clone(),
        );

        let mut right_peer = Peer::from_parts(
            "right".to_string(),
            FrameReader::new(right.try_clone().unwrap()),
            FrameWriter::new(right),
            HandshakeResult {
                peer_id: "p".into(),
                protocol_version: "1.0".into(),
                negotiated_channels: vec![1],
                client_auth_token: None,
            },
            Some(registry),
            config,
        );

        let mut left_peer = left_peer;
        left_peer
            .send_json(1, &serde_json::json!({"v": 1}))
            .unwrap();
        assert!(matches!(
            left_peer.send_json(1, &serde_json::json!({"v": "bad"})),
            Err(PeerError::Schema(_))
        ));

        let frame = right_peer.recv().unwrap();
        assert_eq!(frame.channel, 1);
    }
}
