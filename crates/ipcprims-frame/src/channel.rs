//! Built-in channel IDs.
//!
//! Channels 0-255 are reserved for built-in use.
//! Channels 256-65535 are available for application-defined use.

/// Connection management (handshake, ping/pong, shutdown).
pub const CONTROL: u16 = 0;

/// Structured commands (request/response).
pub const COMMAND: u16 = 1;

/// Bulk data transfer.
pub const DATA: u16 = 2;

/// Metrics, logs, health signals.
pub const TELEMETRY: u16 = 3;

/// Error notifications.
pub const ERROR: u16 = 4;

/// First user-defined channel ID.
pub const USER_CHANNEL_START: u16 = 256;

/// Returns a human-readable name for a channel ID.
pub fn channel_name(id: u16) -> &'static str {
    match id {
        CONTROL => "CONTROL",
        COMMAND => "COMMAND",
        DATA => "DATA",
        TELEMETRY => "TELEMETRY",
        ERROR => "ERROR",
        5..=255 => "RESERVED",
        _ => "USER",
    }
}

/// Returns true if the channel ID is in the reserved range.
pub fn is_reserved(id: u16) -> bool {
    id < USER_CHANNEL_START
}

/// Returns true if the channel ID is a built-in channel.
pub fn is_builtin(id: u16) -> bool {
    id <= ERROR
}
