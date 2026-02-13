//! High-level peer connection management for IPC.
//!
//! This is the "just works" layer. Connect to peers, send and receive
//! framed messages on named channels, with optional schema validation.

pub mod connector;
pub mod control;
pub mod error;
pub mod handshake;
pub mod listener;
pub mod peer;

pub use connector::{connect, connect_with_config};
pub use control::{
    ControlMessage, CONTROL_PING, CONTROL_PONG, CONTROL_SHUTDOWN_ACK, CONTROL_SHUTDOWN_FORCE,
    CONTROL_SHUTDOWN_REQUEST,
};
pub use error::{PeerError, Result};
pub use handshake::{
    handshake_client, handshake_client_with_config, handshake_server, handshake_server_with_config,
    HandshakeConfig, HandshakeRequest, HandshakeResponse, HandshakeResult,
};
pub use listener::PeerListener;
pub use peer::{Peer, PeerConfig};
