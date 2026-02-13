/// Errors that can occur during frame encoding/decoding.
#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    /// The frame header contains an invalid magic number.
    #[error("invalid frame magic (expected 0x4950 \"IP\")")]
    InvalidMagic,

    /// The payload exceeds the configured maximum size.
    #[error("payload too large ({size} bytes, max {max})")]
    PayloadTooLarge { size: usize, max: usize },

    /// An I/O error occurred while reading or writing frames.
    #[error("frame I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The connection was closed before a complete frame was received.
    #[error("connection closed (incomplete frame)")]
    ConnectionClosed,
}

pub type Result<T> = std::result::Result<T, FrameError>;
