//! Tokio async framing codec.
//!
//! This wraps the pure `encode_frame` / `decode_frame` logic in a
//! `tokio_util::codec::{Encoder, Decoder}` implementation so callers can use
//! `tokio_util::codec::Framed` / `FramedRead` / `FramedWrite`.

use bytes::BytesMut;
use tokio_util::codec::{Decoder, Encoder};

use crate::codec::{decode_frame, encode_frame, Frame, FrameConfig};
use crate::error::FrameError;

/// Tokio codec for the ipcprims wire format.
#[derive(Debug, Clone)]
pub struct IpcCodec {
    config: FrameConfig,
}

impl IpcCodec {
    /// Create a codec with default configuration.
    pub fn new() -> Self {
        Self::with_config(FrameConfig::default())
    }

    /// Create a codec with explicit configuration.
    pub fn with_config(config: FrameConfig) -> Self {
        Self { config }
    }

    /// Update maximum payload size for subsequent frame decoding/encoding.
    pub fn set_max_payload_size(&mut self, max_payload_size: usize) {
        self.config.max_payload_size = max_payload_size;
    }

    /// Current codec configuration.
    pub fn config(&self) -> &FrameConfig {
        &self.config
    }
}

impl Default for IpcCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder for IpcCodec {
    type Item = Frame;
    type Error = FrameError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        decode_frame(src, self.config.max_payload_size)
    }
}

impl Encoder<Frame> for IpcCodec {
    type Error = FrameError;

    fn encode(&mut self, item: Frame, dst: &mut BytesMut) -> Result<(), Self::Error> {
        encode_frame(item.channel, item.payload.as_ref(), dst)
    }
}

#[cfg(all(test, feature = "async"))]
mod tests {
    use bytes::Bytes;

    use super::*;
    use crate::codec::DEFAULT_MAX_PAYLOAD;

    #[test]
    fn encode_decode_roundtrip() {
        let mut codec = IpcCodec::new();
        let mut buf = BytesMut::new();

        codec
            .encode(Frame::new(7, Bytes::from_static(b"hello")), &mut buf)
            .unwrap();

        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(decoded.channel, 7);
        assert_eq!(decoded.payload.as_ref(), b"hello");
        assert!(buf.is_empty());
    }

    #[test]
    fn decode_enforces_max_payload() {
        let mut codec = IpcCodec::with_config(FrameConfig {
            max_payload_size: 4,
            ..FrameConfig::default()
        });

        let mut buf = BytesMut::new();
        encode_frame(1, b"toolong", &mut buf).unwrap();

        let err = codec.decode(&mut buf).unwrap_err();
        assert!(matches!(err, FrameError::PayloadTooLarge { .. }));

        let mut codec_ok = IpcCodec::with_config(FrameConfig {
            max_payload_size: DEFAULT_MAX_PAYLOAD,
            ..FrameConfig::default()
        });
        let decoded = codec_ok.decode(&mut buf).unwrap().unwrap();
        assert_eq!(decoded.payload.as_ref(), b"toolong");
    }
}

#[cfg(all(test, unix, feature = "async"))]
mod integration_tests_unix {
    use bytes::{BufMut, Bytes, BytesMut};
    use futures_util::{SinkExt, StreamExt};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_util::codec::{Decoder, Framed};

    use crate::channel::COMMAND;
    use crate::codec::{decode_frame, encode_frame, Frame, DEFAULT_MAX_PAYLOAD, MAGIC};

    #[tokio::test]
    async fn async_uds_echo_frame_roundtrip() {
        // Avoid platform UDS path length limits (macOS `sun_path` is 104 bytes).
        let sock_path = std::path::PathBuf::from(format!(
            "/tmp/ipc-{}-{}.sock",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let listener = ipcprims_transport::AsyncUnixDomainSocket::bind(&sock_path).unwrap();

        let server = tokio::spawn(async move {
            let mut stream = listener.accept().await.unwrap();
            let mut buf = BytesMut::new();
            let mut chunk = [0u8; 4096];

            loop {
                let n = stream.read(&mut chunk).await.unwrap();
                assert_ne!(n, 0);
                buf.extend_from_slice(&chunk[..n]);

                if let Some(frame) = decode_frame(&mut buf, DEFAULT_MAX_PAYLOAD).unwrap() {
                    let mut out = BytesMut::new();
                    encode_frame(frame.channel, frame.payload.as_ref(), &mut out).unwrap();
                    stream.write_all(&out).await.unwrap();
                    break;
                }
            }
        });

        let mut client = ipcprims_transport::AsyncUnixDomainSocket::connect(&sock_path)
            .await
            .unwrap();
        let payload = b"hello-async";
        let mut out = BytesMut::new();
        encode_frame(COMMAND, payload, &mut out).unwrap();
        client.write_all(&out).await.unwrap();

        let mut buf = BytesMut::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = client.read(&mut chunk).await.unwrap();
            assert_ne!(n, 0);
            buf.extend_from_slice(&chunk[..n]);
            if let Some(frame) = decode_frame(&mut buf, DEFAULT_MAX_PAYLOAD).unwrap() {
                assert_eq!(frame.channel, COMMAND);
                assert_eq!(frame.payload.as_ref(), payload);
                break;
            }
        }

        server.await.unwrap();
        let _ = std::fs::remove_file(&sock_path);
    }

    #[tokio::test]
    async fn async_uds_framed_ipc_codec_roundtrip() {
        // Avoid platform UDS path length limits (macOS `sun_path` is 104 bytes).
        let sock_path = std::path::PathBuf::from(format!(
            "/tmp/ipc-framed-{}-{}.sock",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let listener = ipcprims_transport::AsyncUnixDomainSocket::bind(&sock_path).unwrap();

        let server = tokio::spawn(async move {
            let stream = listener.accept().await.unwrap();
            let mut framed = Framed::new(stream, super::IpcCodec::new());

            let frame = framed.next().await.unwrap().unwrap();
            assert_eq!(frame.channel, COMMAND);
            assert_eq!(frame.payload.len(), 128 * 1024);

            framed
                .send(Frame::new(frame.channel, frame.payload))
                .await
                .unwrap();
        });

        let stream = ipcprims_transport::AsyncUnixDomainSocket::connect(&sock_path)
            .await
            .unwrap();
        let mut framed = Framed::new(stream, super::IpcCodec::new());

        // Large payload to exercise partial reads/writes through `tokio_util::codec::Framed`.
        let payload = vec![0xABu8; 128 * 1024];
        framed
            .send(Frame::new(COMMAND, Bytes::from(payload.clone())))
            .await
            .unwrap();

        let echoed = framed.next().await.unwrap().unwrap();
        assert_eq!(echoed.channel, COMMAND);
        assert_eq!(echoed.payload.as_ref(), payload.as_slice());

        server.await.unwrap();
        let _ = std::fs::remove_file(&sock_path);
    }

    #[test]
    fn codec_rejects_oversize_declared_length_without_payload_bytes() {
        let mut codec = super::IpcCodec::new();
        let mut buf = BytesMut::new();

        buf.put_slice(&MAGIC);
        buf.put_u32_le((DEFAULT_MAX_PAYLOAD as u32).saturating_add(1));
        buf.put_u16_le(1);

        let err = codec.decode(&mut buf).unwrap_err();
        assert!(matches!(err, super::FrameError::PayloadTooLarge { .. }));
    }
}
