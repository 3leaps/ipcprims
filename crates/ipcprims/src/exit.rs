use std::fmt;
use std::io;

use ipcprims_frame::FrameError;
use ipcprims_peer::PeerError;
use ipcprims_transport::TransportError;

// Exit code constants aligned with rsfulmen/DDR-0002 semantics.
pub const SUCCESS: i32 = 0;
pub const FAILURE: i32 = 1;
pub const TRANSPORT_ERROR: i32 = 3;
#[allow(dead_code)]
pub const HEALTH_CHECK_FAILED: i32 = 30;
pub const PERMISSION_DENIED: i32 = 50;
pub const DATA_INVALID: i32 = 60;
pub const USAGE: i32 = 64;
pub const TIMEOUT: i32 = 124;
pub const INTERNAL: i32 = 125;

pub type CliResult<T> = Result<T, CliError>;

#[derive(Debug)]
pub struct CliError {
    pub code: i32,
    pub message: String,
}

impl CliError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CliError {}

pub fn io_error(context: &str, err: io::Error) -> CliError {
    let code = match err.kind() {
        io::ErrorKind::PermissionDenied => PERMISSION_DENIED,
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => TIMEOUT,
        io::ErrorKind::ConnectionRefused => FAILURE,
        _ => INTERNAL,
    };
    CliError::new(code, format!("{context}: {err}"))
}

pub fn transport_error(context: &str, err: TransportError) -> CliError {
    match err {
        TransportError::Bind { source, .. }
        | TransportError::Connect { source, .. }
        | TransportError::Accept(source)
        | TransportError::Io(source) => io_error(context, source),
        other => CliError::new(TRANSPORT_ERROR, format!("{context}: {other}")),
    }
}

pub fn frame_error(context: &str, err: FrameError) -> CliError {
    match err {
        FrameError::Io(source) => io_error(context, source),
        FrameError::PayloadTooLarge { .. } => {
            CliError::new(DATA_INVALID, format!("{context}: {err}"))
        }
        FrameError::ConnectionClosed => CliError::new(FAILURE, format!("{context}: {err}")),
        other => CliError::new(INTERNAL, format!("{context}: {other}")),
    }
}

pub fn peer_error(context: &str, err: PeerError) -> CliError {
    match err {
        PeerError::Transport(err) => transport_error(context, err),
        PeerError::Frame(err) => frame_error(context, err),
        PeerError::Schema(err) => CliError::new(DATA_INVALID, format!("{context}: {err}")),
        PeerError::Timeout(_) => CliError::new(TIMEOUT, format!("{context}: {err}")),
        PeerError::Json(err) => CliError::new(DATA_INVALID, format!("{context}: {err}")),
        PeerError::UnsupportedChannel(_) => CliError::new(USAGE, format!("{context}: {err}")),
        PeerError::Disconnected(_) => CliError::new(FAILURE, format!("{context}: {err}")),
        other => CliError::new(INTERNAL, format!("{context}: {other}")),
    }
}
