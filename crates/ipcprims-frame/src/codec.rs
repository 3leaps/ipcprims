use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::{FrameError, Result};

/// Frame header: magic (2) + length (4) + channel (2) = 8 bytes.
pub const HEADER_SIZE: usize = 8;

/// Magic bytes: "IP" (0x49 0x50).
pub const MAGIC: [u8; 2] = [0x49, 0x50];

/// Default maximum payload size: 16 MiB.
pub const DEFAULT_MAX_PAYLOAD: usize = 16 * 1024 * 1024;

/// A framed message with channel routing.
#[derive(Debug, Clone)]
pub struct Frame {
    /// The channel this message belongs to.
    pub channel: u16,
    /// The message payload.
    pub payload: Bytes,
}

impl Frame {
    /// Create a new frame.
    pub fn new(channel: u16, payload: impl Into<Bytes>) -> Self {
        Self {
            channel,
            payload: payload.into(),
        }
    }

    /// The total wire size of this frame (header + payload).
    pub fn wire_size(&self) -> usize {
        HEADER_SIZE + self.payload.len()
    }
}

/// Encode a frame into the wire format.
///
/// Wire format:
/// ```text
/// ┌──────────────┬───────────┬──────────┬─────────────────┐
/// │ Magic (2B)   │ Length    │ Channel  │ Payload          │
/// │ 0x49 0x50    │ (4B LE)  │ (2B LE)  │ (Length bytes)   │
/// │ "IP"         │          │          │                  │
/// └──────────────┴───────────┴──────────┴─────────────────┘
/// ```
pub fn encode_frame(channel: u16, payload: &[u8], dst: &mut BytesMut) -> Result<()> {
    if payload.len() > u32::MAX as usize {
        return Err(FrameError::PayloadTooLarge {
            size: payload.len(),
            max: u32::MAX as usize,
        });
    }
    dst.reserve(HEADER_SIZE + payload.len());
    dst.put_slice(&MAGIC);
    dst.put_u32_le(payload.len() as u32);
    dst.put_u16_le(channel);
    dst.put_slice(payload);
    Ok(())
}

/// Decode a frame from a buffer.
///
/// Returns `Ok(None)` if the buffer doesn't contain a complete frame yet.
/// On success, consumes the frame bytes from the buffer.
pub fn decode_frame(src: &mut BytesMut, max_payload: usize) -> Result<Option<Frame>> {
    if src.len() < HEADER_SIZE {
        return Ok(None); // Need more data
    }

    // Check magic
    if src[0..2] != MAGIC {
        return Err(FrameError::InvalidMagic);
    }

    let payload_len = u32::from_le_bytes(src[2..6].try_into().unwrap()) as usize;
    let channel = u16::from_le_bytes(src[6..8].try_into().unwrap());

    if payload_len > max_payload {
        return Err(FrameError::PayloadTooLarge {
            size: payload_len,
            max: max_payload,
        });
    }

    let total = HEADER_SIZE + payload_len;
    if src.len() < total {
        return Ok(None); // Need more data
    }

    src.advance(HEADER_SIZE);
    let payload = src.split_to(payload_len).freeze();

    Ok(Some(Frame { channel, payload }))
}

/// Configuration for the frame codec.
#[derive(Debug, Clone)]
pub struct FrameConfig {
    /// Maximum payload size in bytes. Default: 16 MiB.
    pub max_payload_size: usize,
    /// Read timeout for blocking operations.
    pub read_timeout: Option<std::time::Duration>,
    /// Write timeout for blocking operations.
    pub write_timeout: Option<std::time::Duration>,
}

impl Default for FrameConfig {
    fn default() -> Self {
        Self {
            max_payload_size: DEFAULT_MAX_PAYLOAD,
            read_timeout: None,
            write_timeout: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let mut buf = BytesMut::new();
        let payload = b"hello, ipcprims!";
        let channel = 1u16;

        encode_frame(channel, payload, &mut buf).unwrap();

        assert_eq!(buf.len(), HEADER_SIZE + payload.len());

        let frame = decode_frame(&mut buf, DEFAULT_MAX_PAYLOAD)
            .unwrap()
            .unwrap();

        assert_eq!(frame.channel, channel);
        assert_eq!(frame.payload.as_ref(), payload);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_decode_incomplete_header() {
        let mut buf = BytesMut::from(&[0x49, 0x50, 0x00][..]);
        let result = decode_frame(&mut buf, DEFAULT_MAX_PAYLOAD).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_decode_incomplete_payload() {
        let mut buf = BytesMut::new();
        encode_frame(1, b"hello", &mut buf).unwrap();
        buf.truncate(HEADER_SIZE + 2); // Truncate payload

        let result = decode_frame(&mut buf, DEFAULT_MAX_PAYLOAD).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_decode_invalid_magic() {
        let mut buf = BytesMut::from(&[0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]);
        let result = decode_frame(&mut buf, DEFAULT_MAX_PAYLOAD);
        assert!(matches!(result, Err(FrameError::InvalidMagic)));
    }

    #[test]
    fn test_decode_payload_too_large() {
        let mut buf = BytesMut::new();
        buf.put_slice(&MAGIC);
        buf.put_u32_le(1024 * 1024 * 32); // 32 MiB
        buf.put_u16_le(1);

        let result = decode_frame(&mut buf, DEFAULT_MAX_PAYLOAD);
        assert!(matches!(result, Err(FrameError::PayloadTooLarge { .. })));
    }

    #[test]
    fn test_multiple_frames() {
        let mut buf = BytesMut::new();
        encode_frame(1, b"first", &mut buf).unwrap();
        encode_frame(2, b"second", &mut buf).unwrap();

        let f1 = decode_frame(&mut buf, DEFAULT_MAX_PAYLOAD)
            .unwrap()
            .unwrap();
        assert_eq!(f1.channel, 1);
        assert_eq!(f1.payload.as_ref(), b"first");

        let f2 = decode_frame(&mut buf, DEFAULT_MAX_PAYLOAD)
            .unwrap()
            .unwrap();
        assert_eq!(f2.channel, 2);
        assert_eq!(f2.payload.as_ref(), b"second");

        assert!(buf.is_empty());
    }

    #[test]
    fn test_empty_payload() {
        let mut buf = BytesMut::new();
        encode_frame(0, b"", &mut buf).unwrap();

        let frame = decode_frame(&mut buf, DEFAULT_MAX_PAYLOAD)
            .unwrap()
            .unwrap();
        assert_eq!(frame.channel, 0);
        assert!(frame.payload.is_empty());
    }

    #[test]
    fn test_frame_wire_size() {
        let frame = Frame::new(1, Bytes::from_static(b"test"));
        assert_eq!(frame.wire_size(), HEADER_SIZE + 4);
    }
}
