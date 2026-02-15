use std::path::Path;

use ipcprims_frame::{FrameConfig, FrameReader, FrameWriter, DEFAULT_MAX_PAYLOAD};
#[cfg(unix)]
use ipcprims_transport::UnixDomainSocket;

use crate::error::Result;
use crate::handshake::{handshake_client_with_config, HandshakeConfig};
use crate::peer::{Peer, PeerConfig, SchemaRegistryHandle};

/// Connect to a listening peer as a client.
pub fn connect(path: impl AsRef<Path>, channels: &[u16]) -> Result<Peer> {
    connect_with_config(path, channels, &HandshakeConfig::default(), None, None)
}

/// Connect with explicit configuration.
pub fn connect_with_config(
    path: impl AsRef<Path>,
    channels: &[u16],
    handshake_config: &HandshakeConfig,
    schema_registry: Option<SchemaRegistryHandle>,
    peer_config: Option<PeerConfig>,
) -> Result<Peer> {
    #[cfg(not(unix))]
    {
        let _ = (channels, handshake_config, schema_registry, peer_config);
        let path = path.as_ref().to_path_buf();
        return Err(ipcprims_transport::TransportError::Connect {
            path,
            source: std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "ipcprims-peer requires Unix domain sockets (Windows support planned in v0.2.0)",
            ),
        }
        .into());
    }

    #[cfg(unix)]
    {
        let stream = UnixDomainSocket::connect(path)?;
        let reader_stream = stream.try_clone()?;

        let frame_config = FrameConfig {
            max_payload_size: handshake_config.max_handshake_payload,
            read_timeout: Some(handshake_config.timeout),
            write_timeout: Some(handshake_config.timeout),
        };

        let mut reader = FrameReader::with_config_ipc(reader_stream, frame_config.clone())?;
        let mut writer = FrameWriter::with_config_ipc(stream, frame_config)?;

        let handshake =
            handshake_client_with_config(&mut reader, &mut writer, channels, handshake_config)?;
        // Handshake uses a tighter pre-auth payload budget; restore runtime defaults after auth.
        reader.set_max_payload_size(DEFAULT_MAX_PAYLOAD);
        writer.set_max_payload_size(DEFAULT_MAX_PAYLOAD);
        let id = handshake.peer_id.clone();

        Ok(Peer::from_parts(
            id,
            reader,
            writer,
            handshake,
            schema_registry,
            peer_config.unwrap_or_default(),
        ))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::thread;

    use ipcprims_frame::COMMAND;

    use super::*;
    use crate::listener::PeerListener;

    #[test]
    fn connect_convenience() {
        let dir = std::env::temp_dir().join(format!(
            "ipcc-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir should be creatable");
        let sock_path = dir.join("listener.sock");

        let listener = PeerListener::bind(&sock_path).expect("listener should bind");

        let server = thread::spawn(move || {
            let mut peer = listener.accept().expect("listener should accept");
            let frame = peer.recv_on(COMMAND).expect("should receive command frame");
            peer.send(COMMAND, frame.payload.as_ref())
                .expect("should echo command");
        });

        let mut client = connect(&sock_path, &[COMMAND]).expect("client should connect");
        let response = client.request(b"hello").expect("request should succeed");
        assert_eq!(response.payload.as_ref(), b"hello");

        server.join().expect("server thread should complete");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn connect_runtime_payload_not_limited_by_handshake_cap() {
        let dir = std::env::temp_dir().join(format!(
            "ipcc-large-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir should be creatable");
        let sock_path = dir.join("listener.sock");

        let listener = PeerListener::bind(&sock_path)
            .expect("listener should bind")
            .with_handshake_config(HandshakeConfig {
                max_handshake_payload: 16 * 1024,
                ..HandshakeConfig::default()
            });

        let server = thread::spawn(move || {
            let mut peer = listener.accept().expect("listener should accept");
            let frame = peer.recv_on(COMMAND).expect("should receive command frame");
            peer.send(COMMAND, frame.payload.as_ref())
                .expect("should echo command");
        });

        let mut client = connect(&sock_path, &[COMMAND]).expect("client should connect");
        let payload = vec![0xAB; 64 * 1024];
        let response = client.request(&payload).expect("request should succeed");
        assert_eq!(response.payload.len(), payload.len());
        assert_eq!(response.payload.as_ref(), payload.as_slice());

        server.join().expect("server thread should complete");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
