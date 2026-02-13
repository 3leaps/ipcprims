/// Errors that can occur in peer operations.
#[derive(Debug, thiserror::Error)]
pub enum PeerError {
    /// Transport-level error.
    #[error("transport error: {0}")]
    Transport(#[from] ipcprims_transport::TransportError),

    /// Frame-level error.
    #[error("frame error: {0}")]
    Frame(#[from] ipcprims_frame::FrameError),

    /// Handshake failed.
    #[error("handshake failed: {0}")]
    HandshakeFailed(String),

    /// Peer disconnected.
    #[error("peer disconnected: {0}")]
    Disconnected(String),

    /// The requested channel is not supported by the peer.
    #[error("channel {0} not supported by peer")]
    UnsupportedChannel(u16),

    /// Channel buffer is full while waiting on another channel.
    #[error("channel {0} buffer full")]
    BufferFull(u16),

    /// JSON serialization/deserialization error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Schema validation error.
    #[cfg(feature = "schema")]
    #[error("schema validation error: {0}")]
    Schema(#[from] ipcprims_schema::SchemaError),

    /// Request timed out.
    #[error("request timed out after {0:?}")]
    Timeout(std::time::Duration),

    /// Graceful shutdown failed.
    #[error("shutdown failed: {0}")]
    ShutdownFailed(String),
}

pub type Result<T> = std::result::Result<T, PeerError>;
