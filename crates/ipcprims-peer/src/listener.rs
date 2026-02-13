use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use ipcprims_frame::{
    FrameConfig, FrameReader, FrameWriter, COMMAND, DATA, DEFAULT_MAX_PAYLOAD, ERROR, TELEMETRY,
};
use ipcprims_transport::UnixDomainSocket;

use crate::error::Result;
use crate::handshake::{handshake_server_with_config, HandshakeConfig};
use crate::peer::{Peer, PeerConfig, SchemaRegistryHandle};

/// Listens for and accepts peer connections.
pub struct PeerListener {
    socket: UnixDomainSocket,
    supported_channels: Vec<u16>,
    handshake_config: HandshakeConfig,
    schema_registry: Option<SchemaRegistryHandle>,
    peer_config: PeerConfig,
    next_peer_id: AtomicU64,
}

impl PeerListener {
    /// Bind to a Unix domain socket path.
    pub fn bind(path: impl AsRef<Path>) -> Result<Self> {
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

    /// Bound socket path.
    pub fn path(&self) -> &Path {
        self.socket.path()
    }
}

#[cfg(test)]
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
