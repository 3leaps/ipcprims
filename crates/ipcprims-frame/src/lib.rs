//! Length-prefixed message framing with channel multiplexing for IPC.
//!
//! This is the core value-add layer of ipcprims. Every message is framed with:
//! - A 2-byte magic number ("IP") for stream synchronization
//! - A 4-byte little-endian payload length
//! - A 2-byte little-endian channel ID for multiplexing
//!
//! No partial reads, no buffer management in user code.

pub mod channel;
pub mod codec;
pub mod error;
pub mod reader;
pub mod writer;

pub use channel::{COMMAND, CONTROL, DATA, ERROR, TELEMETRY, USER_CHANNEL_START};
pub use codec::{decode_frame, encode_frame, Frame, FrameConfig, DEFAULT_MAX_PAYLOAD, HEADER_SIZE};
pub use error::{FrameError, Result};
pub use reader::FrameReader;
pub use writer::FrameWriter;
