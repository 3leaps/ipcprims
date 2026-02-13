use std::path::PathBuf;

/// Errors that can occur in IPC transport operations.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    /// Failed to bind to the specified address.
    #[error("failed to bind to {path}: {source}")]
    Bind {
        path: PathBuf,
        source: std::io::Error,
    },

    /// Failed to connect to the specified address.
    #[error("failed to connect to {path}: {source}")]
    Connect {
        path: PathBuf,
        source: std::io::Error,
    },

    /// Failed to accept an incoming connection.
    #[error("failed to accept connection: {0}")]
    Accept(std::io::Error),

    /// An I/O error occurred on the transport stream.
    #[error("transport I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The socket path is too long for the platform.
    #[error("socket path too long ({len} bytes, max {max}): {path}")]
    PathTooLong {
        path: PathBuf,
        len: usize,
        max: usize,
    },

    /// The transport has been shut down.
    #[error("transport shut down")]
    Shutdown,
}

pub type Result<T> = std::result::Result<T, TransportError>;
