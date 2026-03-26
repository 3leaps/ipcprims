use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg_attr(not(unix), allow(unused_imports))]
use ipcprims_frame::{
    FrameConfig, FrameReader, FrameWriter, COMMAND, DATA, DEFAULT_MAX_PAYLOAD, ERROR, TELEMETRY,
};
#[cfg(windows)]
use ipcprims_transport::NamedPipeListener;
#[cfg(unix)]
use ipcprims_transport::UnixDomainSocket;

use crate::error::Result;
#[cfg_attr(not(unix), allow(unused_imports))]
use crate::handshake::handshake_server_with_config;
use crate::handshake::HandshakeConfig;
use crate::peer::{Peer, PeerConfig, SchemaRegistryHandle};

/// Listens for and accepts peer connections.
pub struct PeerListener {
    #[cfg(unix)]
    socket: UnixDomainSocket,
    #[cfg(windows)]
    socket: NamedPipeListener,
    supported_channels: Vec<u16>,
    handshake_config: HandshakeConfig,
    #[cfg_attr(not(unix), allow(dead_code))]
    schema_registry: Option<SchemaRegistryHandle>,
    peer_config: PeerConfig,
    next_peer_id: AtomicU64,
}

impl PeerListener {
    /// Bind to a Unix domain socket path.
    pub fn bind(path: impl AsRef<Path>) -> Result<Self> {
        #[cfg(unix)]
        {
            let socket = UnixDomainSocket::bind(path)?;
            Ok(Self {
                socket,
                supported_channels: vec![COMMAND, DATA, TELEMETRY, ERROR],
                handshake_config: HandshakeConfig::default(),
                schema_registry: None,
                peer_config: PeerConfig::default(),
                next_peer_id: AtomicU64::new(1),
            })
        }

        #[cfg(windows)]
        {
            let socket = NamedPipeListener::bind(path)?;
            Ok(Self {
                socket,
                supported_channels: vec![COMMAND, DATA, TELEMETRY, ERROR],
                handshake_config: HandshakeConfig::default(),
                schema_registry: None,
                peer_config: PeerConfig::default(),
                next_peer_id: AtomicU64::new(1),
            })
        }
    }

    /// Override the supported channel set.
    ///
    /// This is the authorization boundary for channel negotiation.
    pub fn with_channels(mut self, channels: &[u16]) -> Self {
        self.supported_channels = channels.to_vec();
        self
    }

    /// Override handshake config.
    pub fn with_handshake_config(mut self, config: HandshakeConfig) -> Self {
        self.handshake_config = config;
        self
    }

    /// Attach shared schema registry.
    #[cfg(feature = "schema")]
    pub fn with_schema_registry(
        mut self,
        registry: std::sync::Arc<ipcprims_schema::SchemaRegistry>,
    ) -> Self {
        self.schema_registry = Some(registry);
        self
    }

    /// Override peer behavior config.
    pub fn with_peer_config(mut self, config: PeerConfig) -> Self {
        self.peer_config = config;
        self
    }

    /// Accept next connection and assign an auto-generated peer id.
    pub fn accept(&self) -> Result<Peer> {
        let id = self.next_peer_id.fetch_add(1, Ordering::Relaxed);
        self.accept_with_id(&format!("peer-{id}"))
    }

    /// Accept next connection and use explicit peer id.
    pub fn accept_with_id(&self, peer_id: &str) -> Result<Peer> {
        #[cfg(unix)]
        {
            let stream = self.socket.accept()?;
            let reader_stream = stream.try_clone()?;

            let frame_config = FrameConfig {
                max_payload_size: self.handshake_config.max_handshake_payload,
                read_timeout: Some(self.handshake_config.timeout),
                write_timeout: Some(self.handshake_config.timeout),
            };

            let mut reader = FrameReader::with_config_ipc(reader_stream, frame_config.clone())?;
            let mut writer = FrameWriter::with_config_ipc(stream, frame_config)?;

            let handshake = handshake_server_with_config(
                &mut reader,
                &mut writer,
                &self.supported_channels,
                peer_id,
                &self.handshake_config,
            )?;
            // Handshake uses a tighter pre-auth payload budget; restore runtime defaults after auth.
            reader.set_max_payload_size(DEFAULT_MAX_PAYLOAD);
            writer.set_max_payload_size(DEFAULT_MAX_PAYLOAD);

            Ok(Peer::from_parts(
                peer_id.to_string(),
                reader,
                writer,
                handshake,
                self.schema_registry.clone(),
                self.peer_config.clone(),
            ))
        }

        #[cfg(windows)]
        {
            let stream = self.socket.accept()?;
            let reader_stream = stream.try_clone()?;

            let frame_config = FrameConfig {
                max_payload_size: self.handshake_config.max_handshake_payload,
                read_timeout: Some(self.handshake_config.timeout),
                write_timeout: Some(self.handshake_config.timeout),
            };

            let mut reader = FrameReader::with_config_ipc(reader_stream, frame_config.clone())?;
            let mut writer = FrameWriter::with_config_ipc(stream, frame_config)?;

            let handshake = handshake_server_with_config(
                &mut reader,
                &mut writer,
                &self.supported_channels,
                peer_id,
                &self.handshake_config,
            )?;
            // Handshake uses a tighter pre-auth payload budget; restore runtime defaults after auth.
            reader.set_max_payload_size(DEFAULT_MAX_PAYLOAD);
            writer.set_max_payload_size(DEFAULT_MAX_PAYLOAD);

            Ok(Peer::from_parts(
                peer_id.to_string(),
                reader,
                writer,
                handshake,
                self.schema_registry.clone(),
                self.peer_config.clone(),
            ))
        }
    }

    /// Bound socket path.
    pub fn path(&self) -> &Path {
        #[cfg(unix)]
        {
            self.socket.path()
        }

        #[cfg(windows)]
        {
            self.socket.path()
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::path::PathBuf;
    use std::thread;

    use ipcprims_frame::{COMMAND, DATA};

    use super::*;
    use crate::connector::connect;

    fn make_sock_path(tag: &str) -> PathBuf {
        let dir = std::path::PathBuf::from(format!(
            "/tmp/ipcp-{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir should be creatable");
        dir.join("listener.sock")
    }

    #[test]
    fn accept_returns_peer() {
        let sock_path = make_sock_path("accept");
        let listener = PeerListener::bind(&sock_path).expect("listener should bind");

        let server = thread::spawn(move || {
            let peer = listener.accept().expect("listener should accept");
            assert_eq!(peer.id(), "peer-1");
            assert!(peer.supports_channel(COMMAND));
            assert!(peer.handshake_result().client_auth_token.is_none());
        });

        let _client = connect(&sock_path, &[COMMAND]).expect("client should connect");
        server.join().expect("server thread should finish");

        if let Some(parent) = sock_path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn with_channels_negotiates_intersection() {
        let sock_path = make_sock_path("channels");
        let listener = PeerListener::bind(&sock_path)
            .expect("listener should bind")
            .with_channels(&[DATA]);

        let server = thread::spawn(move || {
            let peer = listener.accept().expect("listener should accept");
            assert_eq!(peer.channels(), &[DATA]);
        });

        let client = connect(&sock_path, &[COMMAND, DATA]).expect("client should connect");
        assert_eq!(client.channels(), &[DATA]);
        server.join().expect("server thread should finish");

        if let Some(parent) = sock_path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn accepts_multiple_sequential_connections() {
        let sock_path = make_sock_path("multi");
        let listener = PeerListener::bind(&sock_path).expect("listener should bind");

        let server = thread::spawn(move || {
            let first = listener.accept().expect("first accept should succeed");
            let second = listener.accept().expect("second accept should succeed");
            assert_eq!(first.id(), "peer-1");
            assert_eq!(second.id(), "peer-2");
        });

        let _c1 = connect(&sock_path, &[COMMAND]).expect("first client should connect");
        let _c2 = connect(&sock_path, &[COMMAND]).expect("second client should connect");
        server.join().expect("server thread should finish");

        if let Some(parent) = sock_path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use std::thread;

    use ipcprims_frame::COMMAND;

    use super::*;
    use crate::connector::connect;

    fn make_pipe_name(tag: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(format!(
            r"\\.\pipe\ipcprims-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be after epoch")
                .as_nanos()
        ))
    }

    #[test]
    fn bind_succeeds_on_named_pipe() {
        let pipe = make_pipe_name("bind");
        let listener = PeerListener::bind(&pipe).expect("listener should bind named pipe");
        let path = listener.path().to_string_lossy().to_string();
        assert!(path.starts_with(r"\\.\pipe\"));
    }

    #[test]
    fn accept_roundtrip_on_named_pipe() {
        let pipe = make_pipe_name("accept");
        let listener = PeerListener::bind(&pipe).expect("listener should bind named pipe");

        let server = thread::spawn(move || {
            let mut peer = listener.accept().expect("listener should accept");
            let frame = peer.recv_on(COMMAND).expect("should receive command frame");
            peer.send(COMMAND, frame.payload.as_ref())
                .expect("should echo command");
        });

        let mut client = connect(&pipe, &[COMMAND]).expect("client should connect");
        let response = client
            .request(b"hello-listener")
            .expect("request should succeed");
        assert_eq!(response.payload.as_ref(), b"hello-listener");

        server.join().expect("server thread should complete");
    }

    /// Verify that a named pipe listener can accept two sequential clients.
    ///
    /// Windows named pipes require a new pipe instance after each client
    /// disconnects. This test exercises that recreation path — it was the
    /// subject of fix 6f89584.
    #[test]
    fn multi_client_reconnect_on_named_pipe() {
        let pipe = make_pipe_name("multi-reconnect");
        let listener = PeerListener::bind(&pipe).expect("listener should bind named pipe");

        let server = thread::spawn(move || {
            // Accept first client, exchange a frame, then drop (disconnect).
            let mut peer1 = listener.accept().expect("first accept should succeed");
            assert_eq!(peer1.id(), "peer-1");
            let frame = peer1
                .recv_on(COMMAND)
                .expect("should receive from client 1");
            peer1
                .send(COMMAND, frame.payload.as_ref())
                .expect("should echo to client 1");
            drop(peer1);

            // Accept second client on the same listener.
            let mut peer2 = listener.accept().expect("second accept should succeed");
            assert_eq!(peer2.id(), "peer-2");
            let frame = peer2
                .recv_on(COMMAND)
                .expect("should receive from client 2");
            peer2
                .send(COMMAND, frame.payload.as_ref())
                .expect("should echo to client 2");
        });

        // Client 1: connect, roundtrip, disconnect.
        let mut c1 = connect(&pipe, &[COMMAND]).expect("client 1 should connect");
        let r1 = c1.request(b"from-client-1").expect("client 1 request");
        assert_eq!(r1.payload.as_ref(), b"from-client-1");
        drop(c1);

        // Small delay for pipe instance recreation.
        thread::sleep(std::time::Duration::from_millis(50));

        // Client 2: connect to the same pipe name.
        let mut c2 = connect(&pipe, &[COMMAND]).expect("client 2 should connect");
        let r2 = c2.request(b"from-client-2").expect("client 2 request");
        assert_eq!(r2.payload.as_ref(), b"from-client-2");

        server.join().expect("server thread should complete");
    }
}
