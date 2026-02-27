//! High-level peer connection management for IPC.
//!
//! This is the "just works" layer. Connect to peers, send and receive
//! framed messages on named channels, with optional schema validation.

pub mod control;
pub mod error;

#[cfg(unix)]
pub mod connector;
#[cfg(unix)]
pub mod handshake;
#[cfg(unix)]
pub mod listener;
#[cfg(unix)]
pub mod peer;

#[cfg(all(unix, feature = "async"))]
pub mod async_connector;
#[cfg(all(unix, feature = "async"))]
pub mod async_listener;
#[cfg(all(unix, feature = "async"))]
pub mod async_peer;

#[cfg(unix)]
pub use connector::{connect, connect_with_config};
pub use control::{
    ControlMessage, CONTROL_PING, CONTROL_PONG, CONTROL_SHUTDOWN_ACK, CONTROL_SHUTDOWN_FORCE,
    CONTROL_SHUTDOWN_REQUEST,
};
pub use error::{PeerError, Result};
#[cfg(unix)]
pub use handshake::{
    handshake_client, handshake_client_with_config, handshake_server, handshake_server_with_config,
    HandshakeConfig, HandshakeRequest, HandshakeResponse, HandshakeResult,
};
#[cfg(unix)]
pub use listener::PeerListener;
#[cfg(unix)]
pub use peer::{Peer, PeerConfig};

#[cfg(all(unix, feature = "async"))]
pub use async_connector::{async_connect, async_connect_with_config};
#[cfg(all(unix, feature = "async"))]
pub use async_listener::AsyncPeerListener;
#[cfg(all(unix, feature = "async"))]
pub use async_peer::{AnyReceiver, AsyncPeer, AsyncPeerRx, AsyncPeerTx, ChannelReceiver};
