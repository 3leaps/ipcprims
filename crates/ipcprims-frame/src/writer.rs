use std::io::{ErrorKind, Write};

use bytes::BytesMut;
use ipcprims_transport::IpcStream;

use crate::codec::{encode_frame, Frame, FrameConfig};
use crate::error::{FrameError, Result};

const INITIAL_BUFFER_CAPACITY: usize = 8 * 1024;

/// Writes complete frames to any `Write` stream.
pub struct FrameWriter<T> {
    inner: T,
    buf: BytesMut,
    config: FrameConfig,
}

impl<T: Write> FrameWriter<T> {
    /// Create a new frame writer with default configuration.
    pub fn new(inner: T) -> Self {
        Self::with_config(inner, FrameConfig::default())
    }

    /// Create a new frame writer with explicit configuration.
    pub fn with_config(inner: T, config: FrameConfig) -> Self {
        Self {
            inner,
            buf: BytesMut::with_capacity(INITIAL_BUFFER_CAPACITY),
            config,
        }
    }

    /// Write a complete frame (blocking).
    pub fn write_frame(&mut self, frame: &Frame) -> Result<()> {
        self.send(frame.channel, frame.payload.as_ref())
    }

    /// Encode and send a payload on a channel.
    pub fn send(&mut self, channel: u16, payload: &[u8]) -> Result<()> {
        if payload.len() > self.config.max_payload_size {
            return Err(FrameError::PayloadTooLarge {
                size: payload.len(),
                max: self.config.max_payload_size,
            });
        }

        self.buf.clear();
        encode_frame(channel, payload, &mut self.buf)?;

        let mut offset = 0usize;
        while offset < self.buf.len() {
            match self.inner.write(&self.buf[offset..]) {
                Ok(0) => return Err(FrameError::ConnectionClosed),
                Ok(n) => offset += n,
                Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                Err(err) if err.kind() == ErrorKind::WouldBlock => continue,
                Err(err) => return Err(FrameError::Io(err)),
            }
        }

        self.flush()
    }

    /// Flush the underlying stream.
    pub fn flush(&mut self) -> Result<()> {
        loop {
            match self.inner.flush() {
                Ok(()) => return Ok(()),
                Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                Err(err) if err.kind() == ErrorKind::WouldBlock => continue,
                Err(err) => return Err(FrameError::Io(err)),
            }
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

    /// Consume the writer and return the inner stream.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Update maximum payload size for subsequent frame encoding.
    pub fn set_max_payload_size(&mut self, max_payload_size: usize) {
        self.config.max_payload_size = max_payload_size;
    }

    /// Current frame writer configuration.
    pub fn config(&self) -> &FrameConfig {
        &self.config
    }
}

impl FrameWriter<IpcStream> {
    /// Create a frame writer for `IpcStream` and apply write timeout from config.
    pub fn with_config_ipc(inner: IpcStream, config: FrameConfig) -> Result<Self> {
        inner
            .set_write_timeout(config.write_timeout)
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
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use bytes::BytesMut;

    use super::*;
    use crate::codec::{decode_frame, Frame};

    #[test]
    fn write_single_frame() {
        let cursor = Cursor::new(Vec::<u8>::new());
        let mut writer = FrameWriter::new(cursor);

        writer.send(1, b"hello").unwrap();

        let inner = writer.into_inner();
        let mut wire = BytesMut::from(inner.into_inner().as_slice());
        let frame = decode_frame(&mut wire, usize::MAX).unwrap().unwrap();
        assert_eq!(frame.channel, 1);
        assert_eq!(frame.payload.as_ref(), b"hello");
    }

    #[test]
    fn write_multiple_frames() {
        let cursor = Cursor::new(Vec::<u8>::new());
        let mut writer = FrameWriter::new(cursor);

        writer.send(1, b"one").unwrap();
        writer.send(2, b"two").unwrap();
        writer.send(3, b"three").unwrap();

        let inner = writer.into_inner();
        let mut wire = BytesMut::from(inner.into_inner().as_slice());

        let f1 = decode_frame(&mut wire, usize::MAX).unwrap().unwrap();
        let f2 = decode_frame(&mut wire, usize::MAX).unwrap().unwrap();
        let f3 = decode_frame(&mut wire, usize::MAX).unwrap().unwrap();

        assert_eq!((f1.channel, f1.payload.as_ref()), (1, b"one".as_ref()));
        assert_eq!((f2.channel, f2.payload.as_ref()), (2, b"two".as_ref()));
        assert_eq!((f3.channel, f3.payload.as_ref()), (3, b"three".as_ref()));
    }

    #[test]
    fn payload_too_large_rejected() {
        let cfg = FrameConfig {
            max_payload_size: 4,
            ..FrameConfig::default()
        };
        let cursor = Cursor::new(Vec::<u8>::new());
        let mut writer = FrameWriter::with_config(cursor, cfg);

        let err = writer.send(1, b"oversized").unwrap_err();
        assert!(matches!(err, FrameError::PayloadTooLarge { .. }));
    }

    #[test]
    fn send_convenience_method() {
        let cursor = Cursor::new(Vec::<u8>::new());
        let mut writer = FrameWriter::new(cursor);

        writer.send(42, b"payload").unwrap();

        let inner = writer.into_inner();
        let mut wire = BytesMut::from(inner.into_inner().as_slice());
        let frame = decode_frame(&mut wire, usize::MAX).unwrap().unwrap();

        assert_eq!(frame.channel, 42);
        assert_eq!(frame.payload.as_ref(), b"payload");
    }

    #[test]
    fn flush_propagates() {
        let sink = FlushTrackingWriter::default();
        let flag = Arc::clone(&sink.flushed);
        let mut writer = FrameWriter::new(sink);

        writer.send(1, b"x").unwrap();

        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn write_frame_method() {
        let cursor = Cursor::new(Vec::<u8>::new());
        let mut writer = FrameWriter::new(cursor);
        let frame = Frame::new(2, "abc");

        writer.write_frame(&frame).unwrap();

        let inner = writer.into_inner();
        let mut wire = BytesMut::from(inner.into_inner().as_slice());
        let decoded = decode_frame(&mut wire, usize::MAX).unwrap().unwrap();

        assert_eq!(decoded.channel, 2);
        assert_eq!(decoded.payload.as_ref(), b"abc");
    }

    #[test]
    fn accessors_and_into_inner() {
        let cursor = Cursor::new(Vec::<u8>::new());
        let mut writer = FrameWriter::new(cursor);

        let _ = writer.get_ref();
        let _ = writer.get_mut();
        let _inner = writer.into_inner();
    }

    #[test]
    fn handles_interrupted_write_and_flush() {
        let writer_impl = InterruptedWriteThenFlush {
            wrote_once: false,
            flush_interrupted: false,
            data: Vec::new(),
        };

        let mut writer = FrameWriter::new(writer_impl);
        writer.send(5, b"retry").unwrap();

        let inner = writer.into_inner();
        assert!(!inner.data.is_empty());
    }

    #[test]
    fn handles_would_block_write_and_flush() {
        let writer_impl = WouldBlockWriteThenFlush {
            wrote_once: false,
            flush_would_block: false,
            data: Vec::new(),
        };

        let mut writer = FrameWriter::new(writer_impl);
        writer.send(6, b"retry").unwrap();

        let inner = writer.into_inner();
        assert!(!inner.data.is_empty());
    }

    #[test]
    fn connection_closed_when_write_returns_zero() {
        let mut writer = FrameWriter::new(ZeroWriter);
        let err = writer.send(1, b"x").unwrap_err();
        assert!(matches!(err, FrameError::ConnectionClosed));
    }

    #[test]
    #[cfg(unix)]
    fn applies_write_timeout_for_ipc_stream() {
        let dir = std::env::temp_dir().join(format!(
            "ipcprims-frame-timeout-writer-{}",
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
            write_timeout: Some(std::time::Duration::from_millis(10)),
            ..FrameConfig::default()
        };

        let writer = FrameWriter::with_config_ipc(stream, cfg);
        assert!(writer.is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[derive(Default)]
    struct FlushTrackingWriter {
        flushed: Arc<AtomicBool>,
        data: Vec<u8>,
    }

    impl Write for FlushTrackingWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.data.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.flushed.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    struct InterruptedWriteThenFlush {
        wrote_once: bool,
        flush_interrupted: bool,
        data: Vec<u8>,
    }

    impl Write for InterruptedWriteThenFlush {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if !self.wrote_once {
                self.wrote_once = true;
                return Err(std::io::Error::from(ErrorKind::Interrupted));
            }
            self.data.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            if !self.flush_interrupted {
                self.flush_interrupted = true;
                return Err(std::io::Error::from(ErrorKind::Interrupted));
            }
            Ok(())
        }
    }

    struct WouldBlockWriteThenFlush {
        wrote_once: bool,
        flush_would_block: bool,
        data: Vec<u8>,
    }

    impl Write for WouldBlockWriteThenFlush {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if !self.wrote_once {
                self.wrote_once = true;
                return Err(std::io::Error::from(ErrorKind::WouldBlock));
            }
            self.data.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            if !self.flush_would_block {
                self.flush_would_block = true;
                return Err(std::io::Error::from(ErrorKind::WouldBlock));
            }
            Ok(())
        }
    }

    struct ZeroWriter;

    impl Write for ZeroWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Ok(0)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn written_bytes_decode() {
        let cursor = Cursor::new(Vec::<u8>::new());
        let mut writer = FrameWriter::new(cursor);

        writer.send(3, b"z").unwrap();

        let mut wire = writer.into_inner().into_inner();
        let mut framed = crate::reader::FrameReader::new(Cursor::new(std::mem::take(&mut wire)));
        let frame = framed.read_frame().unwrap();
        assert_eq!(frame.channel, 3);
        assert_eq!(frame.payload.as_ref(), b"z");
    }
}
