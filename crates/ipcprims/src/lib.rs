//! Reliable inter-process communication with permissive licensing.
//!
//! ipcprims provides permissively licensed, cross-platform IPC primitives — named pipes,
//! Unix sockets, message framing, schema validation, and channel multiplexing.
//!
//! # Crate Structure
//!
//! - [`transport`] — Low-level transport abstraction (UDS, named pipes)
//! - [`frame`] — Length-prefixed message framing with channel multiplexing
//! - [`schema`] — Optional JSON Schema validation (behind `schema` feature)
//! - [`peer`] — High-level peer connection management (behind `peer` feature)

/// Re-export transport types.
pub mod transport {
    pub use ipcprims_transport::*;
}

/// Re-export frame types.
pub mod frame {
    pub use ipcprims_frame::*;
}

/// Re-export schema types (requires `schema` feature).
#[cfg(feature = "schema")]
pub mod schema {
    pub use ipcprims_schema::*;
}

/// Re-export peer types (requires `peer` feature).
#[cfg(feature = "peer")]
pub mod peer {
    pub use ipcprims_peer::*;
}
