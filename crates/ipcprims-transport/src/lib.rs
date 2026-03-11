//! Cross-platform IPC transport abstraction.
//!
//! Provides a unified interface over different local transport mechanisms:
//! - Unix domain sockets (Linux/macOS)
//! - Named pipes (Windows)
//!
//! This is the lowest layer of ipcprims. Everything else builds on top of
//! the [`IpcStream`] type provided here.

pub mod error;
pub mod traits;

#[cfg(windows)]
pub mod npipes;
#[cfg(unix)]
pub mod uds;

pub use error::{Result, TransportError};
pub use traits::IpcStream;

#[cfg(windows)]
pub use npipes::{NamedPipeListener, NamedPipeStream};
#[cfg(unix)]
pub use uds::UnixDomainSocket;

#[cfg(all(windows, feature = "async"))]
pub mod async_npipes;
#[cfg(all(unix, feature = "async"))]
pub mod async_uds;

#[cfg(all(windows, feature = "async"))]
pub use async_npipes::{AsyncIpcStream, AsyncNamedPipeSocket};
#[cfg(all(unix, feature = "async"))]
pub use async_uds::{AsyncIpcStream, AsyncUnixDomainSocket};
