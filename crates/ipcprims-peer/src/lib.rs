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

#[cfg(feature = "async")]
pub mod async_connector;
#[cfg(feature = "async")]
pub mod async_listener;
#[cfg(feature = "async")]
pub mod async_peer;

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

#[cfg(feature = "async")]
pub use async_connector::{async_connect, async_connect_with_config};
#[cfg(feature = "async")]
pub use async_listener::AsyncPeerListener;
#[cfg(feature = "async")]
pub use async_peer::{AnyReceiver, AsyncPeer, AsyncPeerRx, AsyncPeerTx, ChannelReceiver};
