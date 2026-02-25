use std::path::Path;

use ipcprims_transport::AsyncUnixDomainSocket;

use crate::async_peer::{build_async_peer, AsyncPeer};
use crate::error::Result;
use crate::handshake::{async_handshake_client_with_config, HandshakeConfig};
use crate::peer::{PeerConfig, SchemaRegistryHandle};

/// Connect to a listening peer as a client (async).
pub async fn async_connect(path: impl AsRef<Path>, channels: &[u16]) -> Result<AsyncPeer> {
    async_connect_with_config(path, channels, &HandshakeConfig::default(), None, None).await
}

/// Connect with explicit configuration (async).
pub async fn async_connect_with_config(
    path: impl AsRef<Path>,
    channels: &[u16],
    handshake_config: &HandshakeConfig,
    schema_registry: Option<SchemaRegistryHandle>,
    peer_config: Option<PeerConfig>,
) -> Result<AsyncPeer> {
    let stream = AsyncUnixDomainSocket::connect(path).await?;
    let (mut reader, mut writer) = stream.into_split();

    let handshake =
        async_handshake_client_with_config(&mut reader, &mut writer, channels, handshake_config)
            .await?;

    Ok(build_async_peer(
        handshake.peer_id.clone(),
        reader,
        writer,
        handshake,
        schema_registry,
        peer_config.unwrap_or_default(),
    ))
}
