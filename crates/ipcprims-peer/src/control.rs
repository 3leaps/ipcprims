use serde::{Deserialize, Serialize};

/// CONTROL message type: ping request.
pub const CONTROL_PING: &str = "ping";
/// CONTROL message type: ping response.
pub const CONTROL_PONG: &str = "pong";
/// CONTROL message type: graceful shutdown request.
pub const CONTROL_SHUTDOWN_REQUEST: &str = "shutdown_request";
/// CONTROL message type: graceful shutdown acknowledgement.
pub const CONTROL_SHUTDOWN_ACK: &str = "shutdown_ack";
/// CONTROL message type: force-close request.
pub const CONTROL_SHUTDOWN_FORCE: &str = "shutdown_force";

/// CONTROL channel message payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ControlMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

impl ControlMessage {
    /// Create a ping message.
    pub fn ping() -> Self {
        Self {
            msg_type: CONTROL_PING.to_string(),
            payload: None,
            timestamp: None,
        }
    }

    /// Create a pong message.
    pub fn pong() -> Self {
        Self {
            msg_type: CONTROL_PONG.to_string(),
            payload: None,
            timestamp: None,
        }
    }

    /// Create a shutdown request.
    pub fn shutdown_request(reason: Option<&str>) -> Self {
        let payload = reason.map(|reason| serde_json::json!({ "reason": reason }));
        Self {
            msg_type: CONTROL_SHUTDOWN_REQUEST.to_string(),
            payload,
            timestamp: None,
        }
    }

    /// Create a shutdown acknowledgement.
    pub fn shutdown_ack() -> Self {
        Self {
            msg_type: CONTROL_SHUTDOWN_ACK.to_string(),
            payload: None,
            timestamp: None,
        }
    }

    /// Create a force-shutdown message.
    pub fn shutdown_force() -> Self {
        Self {
            msg_type: CONTROL_SHUTDOWN_FORCE.to_string(),
            payload: None,
            timestamp: None,
        }
    }
}
