use std::io::{ErrorKind, Read};

use bytes::BytesMut;
use ipcprims_transport::IpcStream;

use crate::codec::{decode_frame, Frame, FrameConfig};
use crate::error::{FrameError, Result};

const INITIAL_BUFFER_CAPACITY: usize = 8 * 1024;
const READ_CHUNK_SIZE: usize = 8 * 1024;

/// Reads complete frames from any `Read` stream.
///
/// Handles partial reads internally — callers always get complete frames.
pub struct FrameReader<T> {
    inner: T,
    buf: BytesMut,
    config: FrameConfig,
}

impl<T: Read> FrameReader<T> {
    /// Create a new frame reader with default configuration.
    pub fn new(inner: T) -> Self {
        Self::with_config(inner, FrameConfig::default())
    }

    /// Create a new frame reader with explicit configuration.
    pub fn with_config(inner: T, config: FrameConfig) -> Self {
        Self {
            inner,
            buf: BytesMut::with_capacity(INITIAL_BUFFER_CAPACITY),
            config,
        }
    }

    /// Read the next complete frame (blocking).
    ///
    /// Returns `Err(FrameError::ConnectionClosed)` when EOF is reached.
    pub fn read_frame(&mut self) -> Result<Frame> {
        loop {
            if let Some(frame) = decode_frame(&mut self.buf, self.config.max_payload_size)? {
                return Ok(frame);
            }

            let mut chunk = [0u8; READ_CHUNK_SIZE];
            let read = match self.inner.read(&mut chunk) {
                Ok(n) => n,
                Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                Err(err) => return Err(FrameError::Io(err)),
            };

            if read == 0 {
                return Err(FrameError::ConnectionClosed);
            }

            self.buf.extend_from_slice(&chunk[..read]);
        }
    }

    /// Borrow the underlying stream.
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Mutably borrow the underlying stream.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Consume the reader and return the inner stream.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Update maximum payload size for subsequent frame decoding.
    pub fn set_max_payload_size(&mut self, max_payload_size: usize) {
        self.config.max_payload_size = max_payload_size;
    }

    /// Current frame reader configuration.
    pub fn config(&self) -> &FrameConfig {
        &self.config
    }
}

impl FrameReader<IpcStream> {
    /// Create a frame reader for `IpcStream` and apply read timeout from config.
    pub fn with_config_ipc(inner: IpcStream, config: FrameConfig) -> Result<Self> {
        inner
            .set_read_timeout(config.read_timeout)
            .map_err(transport_to_frame_error)?;
        Ok(Self::with_config(inner, config))
    }
}

fn transport_to_frame_error(err: ipcprims_transport::TransportError) -> FrameError {
    match err {
        ipcprims_transport::TransportError::Io(io)
        | ipcprims_transport::TransportError::Accept(io) => FrameError::Io(io),
        ipcprims_transport::TransportError::Bind { source, .. }
        | ipcprims_transport::TransportError::Connect { source, .. } => FrameError::Io(source),
        other => FrameError::Io(std::io::Error::other(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::sync::{Arc, Mutex};

    use bytes::{BufMut, BytesMut};

    use super::*;
    use crate::codec::{encode_frame, MAGIC};

    #[test]
    fn read_single_frame() {
        let mut wire = BytesMut::new();
        encode_frame(1, b"hello", &mut wire).unwrap();

        let mut reader = FrameReader::new(Cursor::new(wire.to_vec()));
        let frame = reader.read_frame().unwrap();

        assert_eq!(frame.channel, 1);
        assert_eq!(frame.payload.as_ref(), b"hello");
    }

    #[test]
    fn read_multiple_frames() {
        let mut wire = BytesMut::new();
        encode_frame(1, b"one", &mut wire).unwrap();
        encode_frame(2, b"two", &mut wire).unwrap();
        encode_frame(3, b"three", &mut wire).unwrap();

        let mut reader = FrameReader::new(Cursor::new(wire.to_vec()));

        let f1 = reader.read_frame().unwrap();
        let f2 = reader.read_frame().unwrap();
        let f3 = reader.read_frame().unwrap();

        assert_eq!((f1.channel, f1.payload.as_ref()), (1, b"one".as_ref()));
        assert_eq!((f2.channel, f2.payload.as_ref()), (2, b"two".as_ref()));
        assert_eq!((f3.channel, f3.payload.as_ref()), (3, b"three".as_ref()));
    }

    #[test]
    fn read_frame_with_large_payload() {
        let payload = vec![0xAB; 64 * 1024];
        let mut wire = BytesMut::new();
        encode_frame(9, &payload, &mut wire).unwrap();

        let mut reader = FrameReader::new(Cursor::new(wire.to_vec()));
        let frame = reader.read_frame().unwrap();

        assert_eq!(frame.channel, 9);
        assert_eq!(frame.payload.as_ref(), payload.as_slice());
    }

    #[test]
    fn partial_read_handling() {
        let mut wire = BytesMut::new();
        encode_frame(4, b"slow", &mut wire).unwrap();

        let byte_reader = ByteByByteReader {
            bytes: wire.to_vec(),
            pos: 0,
        };
        let mut reader = FrameReader::new(byte_reader);

        let frame = reader.read_frame().unwrap();
        assert_eq!(frame.channel, 4);
        assert_eq!(frame.payload.as_ref(), b"slow");
    }

    #[test]
    fn connection_closed_cleanly() {
        let mut reader = FrameReader::new(Cursor::new(Vec::<u8>::new()));
        let err = reader.read_frame().unwrap_err();
        assert!(matches!(err, FrameError::ConnectionClosed));
    }

    #[test]
    fn connection_closed_mid_frame() {
        let mut partial = BytesMut::new();
        partial.put_slice(&MAGIC);
        partial.put_u32_le(16);
        partial.put_u16_le(2);
        partial.put_slice(b"only-part");

        let mut reader = FrameReader::new(Cursor::new(partial.to_vec()));
        let err = reader.read_frame().unwrap_err();
        assert!(matches!(err, FrameError::ConnectionClosed));
    }

    #[test]
    fn invalid_magic_in_stream() {
        let bytes = vec![0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let mut reader = FrameReader::new(Cursor::new(bytes));
        let err = reader.read_frame().unwrap_err();
        assert!(matches!(err, FrameError::InvalidMagic));
    }

    #[test]
    fn oversized_frame_in_stream() {
        let mut wire = BytesMut::new();
        wire.put_slice(&MAGIC);
        wire.put_u32_le(1024);
        wire.put_u16_le(1);

        let cfg = FrameConfig {
            max_payload_size: 16,
            ..FrameConfig::default()
        };
        let mut reader = FrameReader::with_config(Cursor::new(wire.to_vec()), cfg);
        let err = reader.read_frame().unwrap_err();
        assert!(matches!(err, FrameError::PayloadTooLarge { .. }));
    }

    #[derive(Debug)]
    struct ByteByByteReader {
        bytes: Vec<u8>,
        pos: usize,
    }

    impl Read for ByteByByteReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.pos >= self.bytes.len() {
                return Ok(0);
            }
            if buf.is_empty() {
                return Ok(0);
            }

            buf[0] = self.bytes[self.pos];
            self.pos += 1;
            Ok(1)
        }
    }

    #[test]
    fn roundtrip_over_pipe() {
        let (left, right) = std::os::unix::net::UnixStream::pair().unwrap();
        let mut writer = crate::writer::FrameWriter::new(left);
        let mut reader = FrameReader::new(right);

        writer.send(1, b"ping").unwrap();
        let frame = reader.read_frame().unwrap();

        assert_eq!(frame.channel, 1);
        assert_eq!(frame.payload.as_ref(), b"ping");
    }

    #[test]
    fn multi_channel_roundtrip() {
        let (left, right) = std::os::unix::net::UnixStream::pair().unwrap();
        let mut writer = crate::writer::FrameWriter::new(left);
        let mut reader = FrameReader::new(right);

        writer.send(1, b"command").unwrap();
        writer.send(2, b"data").unwrap();
        writer.send(3, b"telemetry").unwrap();

        let f1 = reader.read_frame().unwrap();
        let f2 = reader.read_frame().unwrap();
        let f3 = reader.read_frame().unwrap();

        assert_eq!((f1.channel, f1.payload.as_ref()), (1, b"command".as_ref()));
        assert_eq!((f2.channel, f2.payload.as_ref()), (2, b"data".as_ref()));
        assert_eq!(
            (f3.channel, f3.payload.as_ref()),
            (3, b"telemetry".as_ref())
        );
    }

    #[test]
    fn concurrent_reader_writer_threads() {
        let (left, right) = std::os::unix::net::UnixStream::pair().unwrap();
        let mut writer = crate::writer::FrameWriter::new(left);
        let reader = FrameReader::new(right);
        let reader = Arc::new(Mutex::new(reader));

        let reader_thread = {
            let reader = Arc::clone(&reader);
            std::thread::spawn(move || {
                for expected in 0..64u16 {
                    let frame = reader.lock().unwrap().read_frame().unwrap();
                    assert_eq!(frame.channel, expected % 5);
                    assert_eq!(frame.payload.as_ref(), format!("msg-{expected}").as_bytes());
                }
            })
        };

        for i in 0..64u16 {
            let payload = format!("msg-{i}");
            writer.send(i % 5, payload.as_bytes()).unwrap();
        }

        reader_thread.join().unwrap();
    }

    #[test]
    fn accessors_and_into_inner() {
        let cursor = Cursor::new(Vec::<u8>::new());
        let mut reader = FrameReader::new(cursor);

        let _ = reader.get_ref();
        let _ = reader.get_mut();
        let _inner = reader.into_inner();
    }

    #[test]
    fn read_would_block_propagates_io_error() {
        let mut wire = BytesMut::new();
        encode_frame(7, b"ok", &mut wire).unwrap();

        let reader = WouldBlockThenData {
            state: 0,
            bytes: wire.to_vec(),
            pos: 0,
        };
        let mut framed = FrameReader::new(reader);
        let err = framed.read_frame().unwrap_err();
        assert!(matches!(err, FrameError::Io(e) if e.kind() == ErrorKind::WouldBlock));
    }

    struct WouldBlockThenData {
        state: u8,
        bytes: Vec<u8>,
        pos: usize,
    }

    impl Read for WouldBlockThenData {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.state == 0 {
                self.state = 1;
                return Err(std::io::Error::from(ErrorKind::WouldBlock));
            }
            if self.pos >= self.bytes.len() {
                return Ok(0);
            }
            let remaining = self.bytes.len() - self.pos;
            let n = remaining.min(buf.len());
            buf[..n].copy_from_slice(&self.bytes[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
    }

    #[test]
    fn interrupted_read_retries() {
        let mut wire = BytesMut::new();
        encode_frame(8, b"ok", &mut wire).unwrap();

        let reader = InterruptedThenData {
            state: 0,
            bytes: wire.to_vec(),
            pos: 0,
        };
        let mut framed = FrameReader::new(reader);
        let frame = framed.read_frame().unwrap();

        assert_eq!(frame.channel, 8);
        assert_eq!(frame.payload.as_ref(), b"ok");
    }

    struct InterruptedThenData {
        state: u8,
        bytes: Vec<u8>,
        pos: usize,
    }

    impl Read for InterruptedThenData {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.state == 0 {
                self.state = 1;
                return Err(std::io::Error::from(ErrorKind::Interrupted));
            }
            if self.pos >= self.bytes.len() {
                return Ok(0);
            }
            let remaining = self.bytes.len() - self.pos;
            let n = remaining.min(buf.len());
            buf[..n].copy_from_slice(&self.bytes[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
    }

    #[test]
    #[cfg(unix)]
    fn applies_read_timeout_for_ipc_stream() {
        let dir = std::env::temp_dir().join(format!(
            "ipcprims-frame-timeout-reader-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let sock_path = dir.join("test.sock");
        let listener = ipcprims_transport::UnixDomainSocket::bind(&sock_path).unwrap();

        let path_clone = sock_path.clone();
        let connector = std::thread::spawn(move || {
            ipcprims_transport::UnixDomainSocket::connect(path_clone).unwrap()
        });
        let stream = listener.accept().unwrap();
        let _client = connector.join().unwrap();

        let cfg = FrameConfig {
            read_timeout: Some(std::time::Duration::from_millis(10)),
            ..FrameConfig::default()
        };

        let reader = FrameReader::with_config_ipc(stream, cfg);
        assert!(reader.is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[cfg(unix)]
    fn roundtrip_over_ipc_stream_uds() {
        let dir = std::env::temp_dir().join(format!(
            "ipcprims-frame-uds-roundtrip-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let sock_path = dir.join("test.sock");
        let listener = ipcprims_transport::UnixDomainSocket::bind(&sock_path).unwrap();

        let path_clone = sock_path.clone();
        let server = std::thread::spawn(move || {
            let stream = listener.accept().unwrap();
            let mut reader = FrameReader::new(stream);
            let frame = reader.read_frame().unwrap();
            assert_eq!(frame.channel, 11);
            assert_eq!(frame.payload.as_ref(), b"uds");
        });

        let stream = ipcprims_transport::UnixDomainSocket::connect(&path_clone).unwrap();
        let mut writer = crate::writer::FrameWriter::new(stream);
        writer.send(11, b"uds").unwrap();

        server.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
