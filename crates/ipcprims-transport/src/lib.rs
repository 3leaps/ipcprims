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

#[cfg(unix)]
pub mod uds;

pub use error::{Result, TransportError};
pub use traits::IpcStream;

#[cfg(unix)]
pub use uds::UnixDomainSocket;
