use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use ipcprims_frame::{COMMAND, DATA, ERROR, TELEMETRY};
use ipcprims_transport::AsyncUnixDomainSocket;
use tokio_util::sync::CancellationToken;

use crate::async_peer::{build_async_peer_with_cancel, AsyncPeer};
use crate::error::Result;
use crate::handshake::{async_handshake_server_with_config, HandshakeConfig};
use crate::peer::{PeerConfig, SchemaRegistryHandle};

/// Listens for and accepts peer connections (async).
pub struct AsyncPeerListener {
    socket: AsyncUnixDomainSocket,
    supported_channels: Vec<u16>,
    handshake_config: HandshakeConfig,
    schema_registry: Option<SchemaRegistryHandle>,
    peer_config: PeerConfig,
    cancel: Option<CancellationToken>,
    next_peer_id: AtomicU64,
}

impl AsyncPeerListener {
    /// Bind to a Unix domain socket path.
    pub fn bind(path: impl AsRef<Path>) -> Result<Self> {
        let socket = AsyncUnixDomainSocket::bind(path)?;
        Ok(Self {
            socket,
            supported_channels: vec![COMMAND, DATA, TELEMETRY, ERROR],
            handshake_config: HandshakeConfig::default(),
            schema_registry: None,
            peer_config: PeerConfig::default(),
            cancel: None,
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

    /// Attach an external structured cancellation token.
    ///
    /// When cancelled, all accepted peers from this listener will stop their reader tasks and
    /// receivers will observe a disconnect.
    pub fn with_cancellation_token(mut self, token: CancellationToken) -> Self {
        self.cancel = Some(token);
        self
    }

    /// Accept next connection and assign an auto-generated peer id.
    pub async fn accept(&self) -> Result<AsyncPeer> {
        let id = self.next_peer_id.fetch_add(1, Ordering::Relaxed);
        self.accept_with_id(&format!("peer-{id}")).await
    }

    /// Accept next connection and use explicit peer id.
    pub async fn accept_with_id(&self, peer_id: &str) -> Result<AsyncPeer> {
        let stream = self.socket.accept().await?;
        let (mut reader, mut writer) = stream.into_split();

        let handshake = async_handshake_server_with_config(
            &mut reader,
            &mut writer,
            &self.supported_channels,
            peer_id,
            &self.handshake_config,
        )
        .await?;

        Ok(build_async_peer_with_cancel(
            peer_id.to_string(),
            reader,
            writer,
            handshake,
            self.schema_registry.clone(),
            self.peer_config.clone(),
            self.cancel.clone(),
        ))
    }

    /// Bound socket path.
    pub fn path(&self) -> &Path {
        self.socket.path()
    }
}
