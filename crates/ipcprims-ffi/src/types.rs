use std::ffi::c_void;

use ipcprims_peer::{Peer, PeerListener};

#[cfg(feature = "schema")]
use ipcprims_schema::SchemaRegistry;

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcResult {
    Ok = 0,
    InvalidArgument = 1,
    TransportError = 2,
    FrameError = 3,
    HandshakeFailed = 4,
    Disconnected = 5,
    UnsupportedChannel = 6,
    BufferFull = 7,
    Timeout = 8,
    ShutdownFailed = 9,
    SchemaError = 10,
    Internal = 99,
}

#[allow(dead_code)]
pub const IPC_OK: IpcResult = IpcResult::Ok;
#[allow(dead_code)]
pub const IPC_ERR_INVALID_ARGUMENT: IpcResult = IpcResult::InvalidArgument;
#[allow(dead_code)]
pub const IPC_ERR_TRANSPORT: IpcResult = IpcResult::TransportError;
#[allow(dead_code)]
pub const IPC_ERR_FRAME: IpcResult = IpcResult::FrameError;
#[allow(dead_code)]
pub const IPC_ERR_HANDSHAKE_FAILED: IpcResult = IpcResult::HandshakeFailed;
#[allow(dead_code)]
pub const IPC_ERR_DISCONNECTED: IpcResult = IpcResult::Disconnected;
#[allow(dead_code)]
pub const IPC_ERR_UNSUPPORTED_CHANNEL: IpcResult = IpcResult::UnsupportedChannel;
#[allow(dead_code)]
pub const IPC_ERR_BUFFER_FULL: IpcResult = IpcResult::BufferFull;
#[allow(dead_code)]
pub const IPC_ERR_TIMEOUT: IpcResult = IpcResult::Timeout;
#[allow(dead_code)]
pub const IPC_ERR_SHUTDOWN_FAILED: IpcResult = IpcResult::ShutdownFailed;
#[allow(dead_code)]
pub const IPC_ERR_SCHEMA: IpcResult = IpcResult::SchemaError;
#[allow(dead_code)]
pub const IPC_ERR_INTERNAL: IpcResult = IpcResult::Internal;

#[allow(dead_code)]
pub const IPC_CHANNEL_CONTROL: u16 = 0;
#[allow(dead_code)]
pub const IPC_CHANNEL_COMMAND: u16 = 1;
#[allow(dead_code)]
pub const IPC_CHANNEL_DATA: u16 = 2;
#[allow(dead_code)]
pub const IPC_CHANNEL_TELEMETRY: u16 = 3;
#[allow(dead_code)]
pub const IPC_CHANNEL_ERROR: u16 = 4;

#[repr(C)]
#[derive(Debug)]
pub struct IpcFrame {
    pub channel: u16,
    pub data: *mut u8,
    pub len: usize,
}

impl Default for IpcFrame {
    fn default() -> Self {
        Self {
            channel: 0,
            data: std::ptr::null_mut(),
            len: 0,
        }
    }
}

pub type IpcPeerHandle = *mut c_void;
pub type IpcListenerHandle = *mut c_void;
pub type IpcSchemaRegistryHandle = *mut c_void;

pub(crate) struct PeerHandle {
    pub(crate) peer: Option<Peer>,
}

pub(crate) struct ListenerHandle {
    pub(crate) listener: PeerListener,
}

#[cfg(feature = "schema")]
pub(crate) struct SchemaRegistryHandle {
    pub(crate) registry: SchemaRegistry,
}
