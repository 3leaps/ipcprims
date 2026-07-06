use std::collections::HashSet;
use std::fmt;
use std::io::{ErrorKind, Read, Write};
use std::time::{Duration, Instant};

#[cfg(feature = "async")]
use bytes::BytesMut;
use ipcprims_frame::{FrameError, FrameReader, FrameWriter, CONTROL};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::error::{PeerError, Result};

#[cfg(feature = "async")]
use ipcprims_frame::HEADER_SIZE;

const MAX_HANDSHAKE_CHANNELS: usize = 256;
const MAX_PROTOCOL_LEN: usize = 32;
const MAX_VERSION_LEN: usize = 16;
const MAX_PEER_ID_LEN: usize = 128;
pub const MAX_AUTH_TOKEN_LEN: usize = 4096;
const AUTH_TOKEN_HEX_FIELD: &str = "hex";

/// Client handshake request sent on CONTROL channel.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HandshakeRequest {
    /// Protocol name. Must be `ipcprims` by default.
    pub protocol: String,
    /// Protocol version string.
    pub version: String,
    /// Channels requested by the client.
    pub channels: Vec<u16>,
    /// Optional authentication token provided by the client.
    /// Treated as opaque credential material and redacted in debug output.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_auth_token",
        deserialize_with = "deserialize_auth_token"
    )]
    pub auth_token: Option<Zeroizing<Vec<u8>>>,
}

/// Server handshake response sent on CONTROL channel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HandshakeResponse {
    /// Protocol name. Must match request protocol.
    pub protocol: String,
    /// Negotiated protocol version.
    pub version: String,
    /// Negotiated channel set.
    pub channels: Vec<u16>,
    /// Opaque server-assigned peer identifier.
    pub peer_id: String,
}

/// Result of a successful handshake.
#[derive(Clone, PartialEq, Eq)]
pub struct HandshakeResult {
    /// Server-assigned peer identifier.
    pub peer_id: String,
    /// Negotiated protocol version.
    pub protocol_version: String,
    /// Negotiated channels available after handshake.
    pub negotiated_channels: Vec<u16>,
    /// Client auth token observed by the server side.
    pub client_auth_token: Option<Zeroizing<Vec<u8>>>,
}

/// Configuration for handshake negotiation.
#[derive(Clone)]
pub struct HandshakeConfig {
    /// Timeout for each blocking handshake operation.
    pub timeout: Duration,
    /// Expected protocol name.
    pub protocol_name: String,
    /// Local protocol version.
    pub protocol_version: String,
    /// Require at least one negotiated channel.
    pub require_channel_overlap: bool,
    /// Maximum handshake frame payload size in bytes.
    pub max_handshake_payload: usize,
    /// Optional auth token sent by the client.
    /// This is transported as plaintext within local IPC and should not be logged.
    pub auth_token: Option<Zeroizing<Vec<u8>>>,
}

impl Default for HandshakeConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            protocol_name: "ipcprims".to_string(),
            protocol_version: "1.0".to_string(),
            require_channel_overlap: true,
            max_handshake_payload: 16 * 1024,
            auth_token: None,
        }
    }
}

impl fmt::Debug for HandshakeRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_struct("HandshakeRequest");
        dbg.field("protocol", &self.protocol)
            .field("version", &self.version)
            .field("channels", &self.channels);
        if let Some(token) = &self.auth_token {
            dbg.field(
                "auth_token",
                &format_args!("<redacted:{} bytes>", token.len()),
            );
        } else {
            dbg.field("auth_token", &Option::<&[u8]>::None);
        }
        dbg.finish()
    }
}

impl fmt::Debug for HandshakeResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_struct("HandshakeResult");
        dbg.field("peer_id", &self.peer_id)
            .field("protocol_version", &self.protocol_version)
            .field("negotiated_channels", &self.negotiated_channels);
        if let Some(token) = &self.client_auth_token {
            dbg.field(
                "client_auth_token",
                &format_args!("<redacted:{} bytes>", token.len()),
            );
        } else {
            dbg.field("client_auth_token", &Option::<&[u8]>::None);
        }
        dbg.finish()
    }
}

impl fmt::Debug for HandshakeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_struct("HandshakeConfig");
        dbg.field("timeout", &self.timeout)
            .field("protocol_name", &self.protocol_name)
            .field("protocol_version", &self.protocol_version)
            .field("require_channel_overlap", &self.require_channel_overlap)
            .field("max_handshake_payload", &self.max_handshake_payload);
        if let Some(token) = &self.auth_token {
            dbg.field(
                "auth_token",
                &format_args!("<redacted:{} bytes>", token.len()),
            );
        } else {
            dbg.field("auth_token", &Option::<&[u8]>::None);
        }
        dbg.finish()
    }
}

fn serialize_auth_token<S>(
    token: &Option<Zeroizing<Vec<u8>>>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match token {
        Some(token) => {
            let encoded = encode_auth_token_hex(token.as_slice());
            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_entry(AUTH_TOKEN_HEX_FIELD, encoded.as_str())?;
            map.end()
        }
        None => serializer.serialize_none(),
    }
}

fn deserialize_auth_token<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<Zeroizing<Vec<u8>>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<SerdeAuthToken>::deserialize(deserializer).map(|token| token.map(|token| token.0))
}

struct SerdeAuthToken(Zeroizing<Vec<u8>>);

fn encode_auth_token_hex(token: &[u8]) -> Zeroizing<String> {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut encoded = String::with_capacity(token.len() * 2);
    for byte in token {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Zeroizing::new(encoded)
}

fn decode_auth_token_hex<E>(
    encoded: Zeroizing<String>,
) -> std::result::Result<Zeroizing<Vec<u8>>, E>
where
    E: serde::de::Error,
{
    if encoded.len() % 2 != 0 {
        return Err(E::custom("auth_token hex length must be even"));
    }

    let decoded_len = encoded.len() / 2;
    if decoded_len > MAX_AUTH_TOKEN_LEN {
        return Err(E::custom(format!(
            "invalid auth_token length: greater than {MAX_AUTH_TOKEN_LEN}"
        )));
    }

    let mut token = Zeroizing::new(Vec::with_capacity(decoded_len));
    for chunk in encoded.as_bytes().chunks_exact(2) {
        let high = decode_hex_digit::<E>(chunk[0])?;
        let low = decode_hex_digit::<E>(chunk[1])?;
        token.push((high << 4) | low);
    }

    validate_auth_token(Some(token.as_slice())).map_err(E::custom)?;
    Ok(token)
}

fn decode_hex_digit<E>(digit: u8) -> std::result::Result<u8, E>
where
    E: serde::de::Error,
{
    match digit {
        b'0'..=b'9' => Ok(digit - b'0'),
        b'a'..=b'f' => Ok(digit - b'a' + 10),
        b'A'..=b'F' => Ok(digit - b'A' + 10),
        _ => Err(E::custom("auth_token hex contains a non-hex digit")),
    }
}

impl<'de> Deserialize<'de> for SerdeAuthToken {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct AuthTokenVisitor;

        impl<'de> serde::de::Visitor<'de> for AuthTokenVisitor {
            type Value = SerdeAuthToken;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a non-empty hex-encoded auth token object")
            }

            fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut encoded = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        AUTH_TOKEN_HEX_FIELD => {
                            if encoded.is_some() {
                                return Err(serde::de::Error::duplicate_field(
                                    AUTH_TOKEN_HEX_FIELD,
                                ));
                            }
                            encoded = Some(Zeroizing::new(map.next_value::<String>()?));
                        }
                        _ => {
                            return Err(serde::de::Error::unknown_field(
                                &key,
                                &[AUTH_TOKEN_HEX_FIELD],
                            ));
                        }
                    }
                }

                let encoded =
                    encoded.ok_or_else(|| serde::de::Error::missing_field(AUTH_TOKEN_HEX_FIELD))?;
                decode_auth_token_hex(encoded).map(SerdeAuthToken)
            }

            fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut token = Zeroizing::new(Vec::new());
                while let Some(byte) = seq.next_element::<u8>()? {
                    if token.len() >= MAX_AUTH_TOKEN_LEN {
                        return Err(serde::de::Error::custom(format!(
                            "invalid auth_token length: greater than {MAX_AUTH_TOKEN_LEN}"
                        )));
                    }
                    token.push(byte);
                }
                validate_auth_token(Some(token.as_slice())).map_err(serde::de::Error::custom)?;
                Ok(SerdeAuthToken(token))
            }

            fn visit_bytes<E>(self, value: &[u8]) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let token = Zeroizing::new(value.to_vec());
                validate_auth_token(Some(token.as_slice())).map_err(E::custom)?;
                Ok(SerdeAuthToken(token))
            }

            fn visit_byte_buf<E>(self, value: Vec<u8>) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let token = Zeroizing::new(value);
                validate_auth_token(Some(token.as_slice())).map_err(E::custom)?;
                Ok(SerdeAuthToken(token))
            }

            fn visit_str<E>(self, _value: &str) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Err(E::custom("auth_token must be bytes, not a string"))
            }

            fn visit_string<E>(self, _value: String) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Err(E::custom("auth_token must be bytes, not a string"))
            }
        }

        deserializer.deserialize_any(AuthTokenVisitor)
    }
}

/// Perform client-side handshake using default configuration.
pub fn handshake_client<R: Read, W: Write>(
    reader: &mut FrameReader<R>,
    writer: &mut FrameWriter<W>,
    requested_channels: &[u16],
) -> Result<HandshakeResult> {
    handshake_client_with_config(
        reader,
        writer,
        requested_channels,
        &HandshakeConfig::default(),
    )
}

/// Perform client-side handshake using explicit configuration.
pub fn handshake_client_with_config<R: Read, W: Write>(
    reader: &mut FrameReader<R>,
    writer: &mut FrameWriter<W>,
    requested_channels: &[u16],
    config: &HandshakeConfig,
) -> Result<HandshakeResult> {
    validate_protocol_name(&config.protocol_name)?;
    validate_version(&config.protocol_version)?;
    validate_auth_token(config.auth_token.as_ref().map(|token| token.as_slice()))?;

    let requested = normalize_channels(requested_channels)?;

    let req = HandshakeRequest {
        protocol: config.protocol_name.clone(),
        version: config.protocol_version.clone(),
        channels: requested.clone(),
        auth_token: config.auth_token.clone(),
    };

    send_control_json(writer, &req)?;

    let deadline = Instant::now() + config.timeout;
    let payload = recv_control_payload(
        reader,
        deadline,
        config.timeout,
        config.max_handshake_payload,
    )?;
    let resp: HandshakeResponse = serde_json::from_slice(&payload)?;

    validate_protocol_name(&resp.protocol)?;
    validate_version(&resp.version)?;
    validate_peer_id(&resp.peer_id)?;

    if resp.protocol != config.protocol_name {
        return Err(PeerError::HandshakeFailed(format!(
            "unknown protocol '{}' (expected '{}')",
            resp.protocol, config.protocol_name
        )));
    }

    if !is_version_compatible(&config.protocol_version, &resp.version)? {
        return Err(PeerError::HandshakeFailed(format!(
            "incompatible version '{}' (local '{}')",
            resp.version, config.protocol_version
        )));
    }

    let negotiated = normalize_channels(&resp.channels)?;
    let requested_set: HashSet<u16> = requested.iter().copied().collect();
    if negotiated
        .iter()
        .any(|channel| !requested_set.contains(channel))
    {
        return Err(PeerError::HandshakeFailed(
            "server returned channels not requested by client".to_string(),
        ));
    }

    if config.require_channel_overlap && negotiated.is_empty() {
        return Err(PeerError::HandshakeFailed(
            "no overlapping channels".to_string(),
        ));
    }

    Ok(HandshakeResult {
        peer_id: resp.peer_id,
        protocol_version: resp.version,
        negotiated_channels: negotiated,
        client_auth_token: None,
    })
}

/// Perform server-side handshake using default configuration.
pub fn handshake_server<R: Read, W: Write>(
    reader: &mut FrameReader<R>,
    writer: &mut FrameWriter<W>,
    supported_channels: &[u16],
    peer_id: &str,
) -> Result<HandshakeResult> {
    handshake_server_with_config(
        reader,
        writer,
        supported_channels,
        peer_id,
        &HandshakeConfig::default(),
    )
}

/// Perform server-side handshake using explicit configuration.
pub fn handshake_server_with_config<R: Read, W: Write>(
    reader: &mut FrameReader<R>,
    writer: &mut FrameWriter<W>,
    supported_channels: &[u16],
    peer_id: &str,
    config: &HandshakeConfig,
) -> Result<HandshakeResult> {
    validate_protocol_name(&config.protocol_name)?;
    validate_version(&config.protocol_version)?;
    validate_peer_id(peer_id)?;

    let supported = normalize_channels(supported_channels)?;

    let deadline = Instant::now() + config.timeout;
    let payload = recv_control_payload(
        reader,
        deadline,
        config.timeout,
        config.max_handshake_payload,
    )?;
    let req: HandshakeRequest = serde_json::from_slice(&payload)?;

    validate_protocol_name(&req.protocol)?;
    validate_version(&req.version)?;
    validate_auth_token(req.auth_token.as_ref().map(|token| token.as_slice()))?;

    if req.protocol != config.protocol_name {
        return Err(PeerError::HandshakeFailed(format!(
            "unknown protocol '{}' (expected '{}')",
            req.protocol, config.protocol_name
        )));
    }

    if !is_version_compatible(&req.version, &config.protocol_version)? {
        return Err(PeerError::HandshakeFailed(format!(
            "incompatible version '{}' (server '{}')",
            req.version, config.protocol_version
        )));
    }

    let requested = normalize_channels(&req.channels)?;
    let negotiated = intersect_channels(&requested, &supported);

    if config.require_channel_overlap && negotiated.is_empty() {
        return Err(PeerError::HandshakeFailed(
            "no overlapping channels".to_string(),
        ));
    }

    let resp = HandshakeResponse {
        protocol: config.protocol_name.clone(),
        version: config.protocol_version.clone(),
        channels: negotiated.clone(),
        peer_id: peer_id.to_string(),
    };
    send_control_json(writer, &resp)?;

    Ok(HandshakeResult {
        peer_id: peer_id.to_string(),
        protocol_version: config.protocol_version.clone(),
        negotiated_channels: negotiated,
        client_auth_token: req.auth_token,
    })
}

fn send_control_json<T: Serialize, W: Write>(writer: &mut FrameWriter<W>, value: &T) -> Result<()> {
    let payload = serde_json::to_vec(value)?;
    writer.send(CONTROL, &payload)?;
    Ok(())
}

#[cfg(feature = "async")]
async fn send_control_json_async<W: tokio::io::AsyncWrite + Unpin>(
    writer: &mut W,
    value: &impl Serialize,
    deadline: Instant,
    timeout: Duration,
) -> Result<()> {
    let now = Instant::now();
    if now >= deadline {
        return Err(PeerError::Timeout(timeout));
    }

    let payload = serde_json::to_vec(value)?;
    let mut buf = BytesMut::new();
    ipcprims_frame::encode_frame(CONTROL, &payload, &mut buf).map_err(PeerError::Frame)?;

    let remaining = deadline.saturating_duration_since(now);
    tokio::time::timeout(remaining, tokio::io::AsyncWriteExt::write_all(writer, &buf))
        .await
        .map_err(|_| PeerError::Timeout(timeout))?
        .map_err(|e| PeerError::Frame(FrameError::Io(e)))?;

    let now = Instant::now();
    if now >= deadline {
        return Err(PeerError::Timeout(timeout));
    }
    let remaining = deadline.saturating_duration_since(now);
    tokio::time::timeout(remaining, tokio::io::AsyncWriteExt::flush(writer))
        .await
        .map_err(|_| PeerError::Timeout(timeout))?
        .map_err(|e| PeerError::Frame(FrameError::Io(e)))?;
    Ok(())
}

#[cfg(all(test, unix, feature = "async"))]
mod async_write_timeout_tests {
    use super::*;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    struct PendingWriter;

    impl tokio::io::AsyncWrite for PendingWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            Poll::Pending
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Pending
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Pending
        }
    }

    #[tokio::test]
    async fn async_handshake_write_is_timed_out() {
        let mut w = PendingWriter;
        let timeout = Duration::from_millis(20);
        let deadline = Instant::now() + timeout;

        let req = HandshakeRequest {
            protocol: "ipcprims".to_string(),
            version: "1.0".to_string(),
            channels: vec![1],
            auth_token: None,
        };
        let err = send_control_json_async(&mut w, &req, deadline, timeout)
            .await
            .unwrap_err();
        assert!(matches!(err, PeerError::Timeout(_)));
    }
}

fn recv_control_payload<R: Read>(
    reader: &mut FrameReader<R>,
    deadline: Instant,
    timeout: Duration,
    max_handshake_payload: usize,
) -> Result<Vec<u8>> {
    loop {
        if Instant::now() >= deadline {
            return Err(PeerError::Timeout(timeout));
        }

        match reader.read_frame() {
            Ok(frame) => {
                if frame.channel != CONTROL {
                    return Err(PeerError::HandshakeFailed(format!(
                        "expected CONTROL channel {}, got {}",
                        CONTROL, frame.channel
                    )));
                }
                if frame.payload.len() > max_handshake_payload {
                    return Err(PeerError::HandshakeFailed(format!(
                        "handshake payload too large: {} (max {})",
                        frame.payload.len(),
                        max_handshake_payload
                    )));
                }
                return Ok(frame.payload.to_vec());
            }
            Err(FrameError::Io(err))
                if err.kind() == ErrorKind::WouldBlock || err.kind() == ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(FrameError::ConnectionClosed) => {
                return Err(PeerError::Disconnected(
                    "connection closed during handshake".to_string(),
                ));
            }
            Err(err) => return Err(PeerError::Frame(err)),
        }
    }
}

#[cfg(feature = "async")]
async fn recv_control_payload_async<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
    deadline: Instant,
    timeout: Duration,
    max_handshake_payload: usize,
) -> Result<Vec<u8>> {
    let now = Instant::now();
    if now >= deadline {
        return Err(PeerError::Timeout(timeout));
    }

    // Read exactly one frame (header then payload), without over-reading. This prevents
    // losing post-handshake bytes if the peer pipelines frames in a single write.
    let mut header = [0u8; HEADER_SIZE];
    let remaining = deadline - now;
    tokio::time::timeout(
        remaining,
        tokio::io::AsyncReadExt::read_exact(reader, &mut header),
    )
    .await
    .map_err(|_| PeerError::Timeout(timeout))?
    .map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            PeerError::Disconnected("connection closed during handshake".to_string())
        } else {
            PeerError::Frame(FrameError::Io(e))
        }
    })?;

    if header[0..2] != [0x49, 0x50] {
        return Err(PeerError::Frame(FrameError::InvalidMagic));
    }

    let payload_len = u32::from_le_bytes(header[2..6].try_into().unwrap()) as usize;
    let channel = u16::from_le_bytes(header[6..8].try_into().unwrap());

    if channel != CONTROL {
        return Err(PeerError::HandshakeFailed(format!(
            "expected CONTROL channel {}, got {}",
            CONTROL, channel
        )));
    }

    if payload_len > max_handshake_payload {
        return Err(PeerError::HandshakeFailed(format!(
            "handshake payload too large: {} (max {})",
            payload_len, max_handshake_payload
        )));
    }

    let now = Instant::now();
    if now >= deadline {
        return Err(PeerError::Timeout(timeout));
    }
    let remaining = deadline - now;

    let mut payload = vec![0u8; payload_len];
    tokio::time::timeout(
        remaining,
        tokio::io::AsyncReadExt::read_exact(reader, &mut payload),
    )
    .await
    .map_err(|_| PeerError::Timeout(timeout))?
    .map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            PeerError::Disconnected("connection closed during handshake".to_string())
        } else {
            PeerError::Frame(FrameError::Io(e))
        }
    })?;

    Ok(payload)
}

/// Perform client-side handshake (async) using explicit configuration.
#[cfg(feature = "async")]
pub async fn async_handshake_client_with_config<R, W>(
    reader: &mut R,
    writer: &mut W,
    requested_channels: &[u16],
    config: &HandshakeConfig,
) -> Result<HandshakeResult>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    validate_protocol_name(&config.protocol_name)?;
    validate_version(&config.protocol_version)?;
    validate_auth_token(config.auth_token.as_ref().map(|token| token.as_slice()))?;

    let requested = normalize_channels(requested_channels)?;
    let req = HandshakeRequest {
        protocol: config.protocol_name.clone(),
        version: config.protocol_version.clone(),
        channels: requested.clone(),
        auth_token: config.auth_token.clone(),
    };

    let deadline = Instant::now() + config.timeout;
    send_control_json_async(writer, &req, deadline, config.timeout).await?;
    let payload = recv_control_payload_async(
        reader,
        deadline,
        config.timeout,
        config.max_handshake_payload,
    )
    .await?;
    let resp: HandshakeResponse = serde_json::from_slice(&payload)?;

    validate_protocol_name(&resp.protocol)?;
    validate_version(&resp.version)?;
    validate_peer_id(&resp.peer_id)?;

    if resp.protocol != config.protocol_name {
        return Err(PeerError::HandshakeFailed(format!(
            "unknown protocol '{}' (expected '{}')",
            resp.protocol, config.protocol_name
        )));
    }

    if !is_version_compatible(&config.protocol_version, &resp.version)? {
        return Err(PeerError::HandshakeFailed(format!(
            "incompatible version '{}' (local '{}')",
            resp.version, config.protocol_version
        )));
    }

    let negotiated = normalize_channels(&resp.channels)?;
    let requested_set: HashSet<u16> = requested.iter().copied().collect();
    if negotiated
        .iter()
        .any(|channel| !requested_set.contains(channel))
    {
        return Err(PeerError::HandshakeFailed(
            "server returned channels not requested by client".to_string(),
        ));
    }

    if config.require_channel_overlap && negotiated.is_empty() {
        return Err(PeerError::HandshakeFailed(
            "no overlapping channels".to_string(),
        ));
    }

    Ok(HandshakeResult {
        peer_id: resp.peer_id,
        protocol_version: resp.version,
        negotiated_channels: negotiated,
        client_auth_token: None,
    })
}

/// Perform server-side handshake (async) using explicit configuration.
#[cfg(feature = "async")]
pub async fn async_handshake_server_with_config<R, W>(
    reader: &mut R,
    writer: &mut W,
    supported_channels: &[u16],
    peer_id: &str,
    config: &HandshakeConfig,
) -> Result<HandshakeResult>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    validate_protocol_name(&config.protocol_name)?;
    validate_version(&config.protocol_version)?;
    validate_peer_id(peer_id)?;

    let supported = normalize_channels(supported_channels)?;

    let deadline = Instant::now() + config.timeout;
    let payload = recv_control_payload_async(
        reader,
        deadline,
        config.timeout,
        config.max_handshake_payload,
    )
    .await?;
    let req: HandshakeRequest = serde_json::from_slice(&payload)?;

    validate_protocol_name(&req.protocol)?;
    validate_version(&req.version)?;
    validate_auth_token(req.auth_token.as_ref().map(|token| token.as_slice()))?;

    if req.protocol != config.protocol_name {
        return Err(PeerError::HandshakeFailed(format!(
            "unknown protocol '{}' (expected '{}')",
            req.protocol, config.protocol_name
        )));
    }

    if !is_version_compatible(&req.version, &config.protocol_version)? {
        return Err(PeerError::HandshakeFailed(format!(
            "incompatible version '{}' (server '{}')",
            req.version, config.protocol_version
        )));
    }

    let requested = normalize_channels(&req.channels)?;
    let negotiated = intersect_channels(&requested, &supported);

    if config.require_channel_overlap && negotiated.is_empty() {
        return Err(PeerError::HandshakeFailed(
            "no overlapping channels".to_string(),
        ));
    }

    let resp = HandshakeResponse {
        protocol: config.protocol_name.clone(),
        version: config.protocol_version.clone(),
        channels: negotiated.clone(),
        peer_id: peer_id.to_string(),
    };
    send_control_json_async(writer, &resp, deadline, config.timeout).await?;

    Ok(HandshakeResult {
        peer_id: peer_id.to_string(),
        protocol_version: config.protocol_version.clone(),
        negotiated_channels: negotiated,
        client_auth_token: req.auth_token,
    })
}

fn normalize_channels(channels: &[u16]) -> Result<Vec<u16>> {
    if channels.len() > MAX_HANDSHAKE_CHANNELS {
        return Err(PeerError::HandshakeFailed(format!(
            "too many channels in handshake: {} (max {})",
            channels.len(),
            MAX_HANDSHAKE_CHANNELS
        )));
    }

    let mut seen = HashSet::with_capacity(channels.len());
    let mut out = Vec::with_capacity(channels.len());

    for &channel in channels {
        if channel == CONTROL {
            return Err(PeerError::HandshakeFailed(
                "CONTROL channel must not be included in negotiated channels".to_string(),
            ));
        }

        if seen.insert(channel) {
            out.push(channel);
        }
    }

    Ok(out)
}

fn intersect_channels(left: &[u16], right: &[u16]) -> Vec<u16> {
    let right_set: HashSet<u16> = right.iter().copied().collect();
    left.iter()
        .copied()
        .filter(|channel| right_set.contains(channel))
        .collect()
}

fn validate_protocol_name(protocol: &str) -> Result<()> {
    if protocol.is_empty() || protocol.len() > MAX_PROTOCOL_LEN {
        return Err(PeerError::HandshakeFailed(format!(
            "invalid protocol name length: {}",
            protocol.len()
        )));
    }
    Ok(())
}

fn validate_version(version: &str) -> Result<()> {
    if version.is_empty() || version.len() > MAX_VERSION_LEN {
        return Err(PeerError::HandshakeFailed(format!(
            "invalid protocol version length: {}",
            version.len()
        )));
    }
    let _ = parse_version(version)?;
    Ok(())
}

fn validate_peer_id(peer_id: &str) -> Result<()> {
    if peer_id.is_empty() || peer_id.len() > MAX_PEER_ID_LEN {
        return Err(PeerError::HandshakeFailed(format!(
            "invalid peer_id length: {}",
            peer_id.len()
        )));
    }
    Ok(())
}

fn validate_auth_token(auth_token: Option<&[u8]>) -> Result<()> {
    if let Some(token) = auth_token {
        if token.is_empty() || token.len() > MAX_AUTH_TOKEN_LEN {
            return Err(PeerError::HandshakeFailed(format!(
                "invalid auth_token length: {}",
                token.len()
            )));
        }
    }
    Ok(())
}

fn is_version_compatible(client_version: &str, server_version: &str) -> Result<bool> {
    let (client_major, client_minor) = parse_version(client_version)?;
    let (server_major, server_minor) = parse_version(server_version)?;

    Ok(client_major == server_major && client_minor >= server_minor)
}

fn parse_version(version: &str) -> Result<(u16, u16)> {
    let mut parts = version.split('.');

    let major = parts.next().ok_or_else(|| {
        PeerError::HandshakeFailed(format!("invalid version '{}': missing major", version))
    })?;
    let minor = parts.next().ok_or_else(|| {
        PeerError::HandshakeFailed(format!("invalid version '{}': missing minor", version))
    })?;

    if parts.next().is_some() {
        return Err(PeerError::HandshakeFailed(format!(
            "invalid version '{}': expected '<major>.<minor>'",
            version
        )));
    }

    let major = major.parse::<u16>().map_err(|_| {
        PeerError::HandshakeFailed(format!("invalid version '{}': non-numeric major", version))
    })?;
    let minor = minor.parse::<u16>().map_err(|_| {
        PeerError::HandshakeFailed(format!("invalid version '{}': non-numeric minor", version))
    })?;

    Ok((major, minor))
}

#[cfg(all(test, unix))]
mod tests {
    use std::io::{Cursor, ErrorKind, Read};
    use std::os::unix::net::UnixStream;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use super::*;

    #[test]
    fn successful_handshake() {
        let (left, right) = UnixStream::pair().unwrap();

        let server = thread::spawn(move || {
            let mut reader = FrameReader::new(left.try_clone().unwrap());
            let mut writer = FrameWriter::new(left);
            handshake_server(&mut reader, &mut writer, &[1, 2, 3], "peer-1").unwrap()
        });

        let mut reader = FrameReader::new(right.try_clone().unwrap());
        let mut writer = FrameWriter::new(right);
        let client_result = handshake_client(&mut reader, &mut writer, &[1, 2, 3]).unwrap();
        let server_result = server.join().unwrap();

        assert_eq!(client_result.peer_id, "peer-1");
        assert_eq!(client_result.protocol_version, "1.0");
        assert_eq!(client_result.negotiated_channels, vec![1, 2, 3]);
        assert!(client_result.client_auth_token.is_none());
        assert_eq!(server_result.negotiated_channels, vec![1, 2, 3]);
        assert!(server_result.client_auth_token.is_none());
    }

    #[test]
    fn channel_negotiation_intersection() {
        let (left, right) = UnixStream::pair().unwrap();

        let server = thread::spawn(move || {
            let mut reader = FrameReader::new(left.try_clone().unwrap());
            let mut writer = FrameWriter::new(left);
            handshake_server(&mut reader, &mut writer, &[2, 3, 4], "peer-2").unwrap()
        });

        let mut reader = FrameReader::new(right.try_clone().unwrap());
        let mut writer = FrameWriter::new(right);
        let client_result = handshake_client(&mut reader, &mut writer, &[1, 2, 3]).unwrap();
        let server_result = server.join().unwrap();

        assert_eq!(client_result.negotiated_channels, vec![2, 3]);
        assert_eq!(server_result.negotiated_channels, vec![2, 3]);
    }

    #[test]
    fn no_channel_overlap() {
        let (left, right) = UnixStream::pair().unwrap();

        let server = thread::spawn(move || {
            let mut reader = FrameReader::new(left.try_clone().unwrap());
            let mut writer = FrameWriter::new(left);
            handshake_server(&mut reader, &mut writer, &[2], "peer-3")
        });

        let mut reader = FrameReader::new(right.try_clone().unwrap());
        let mut writer = FrameWriter::new(right);
        let client_result = handshake_client(&mut reader, &mut writer, &[1]);
        let server_result = server.join().unwrap();

        assert!(matches!(client_result, Err(PeerError::Disconnected(_))));
        assert!(matches!(server_result, Err(PeerError::HandshakeFailed(_))));
    }

    #[test]
    fn wrong_protocol_name_rejected() {
        let (left, right) = UnixStream::pair().unwrap();

        let server = thread::spawn(move || {
            let mut reader = FrameReader::new(left.try_clone().unwrap());
            let mut writer = FrameWriter::new(left);
            handshake_server(&mut reader, &mut writer, &[1], "peer-4")
        });

        let mut reader = FrameReader::new(right.try_clone().unwrap());
        let mut writer = FrameWriter::new(right);
        let cfg = HandshakeConfig {
            protocol_name: "foobar".to_string(),
            ..HandshakeConfig::default()
        };
        let client_result = handshake_client_with_config(&mut reader, &mut writer, &[1], &cfg);

        assert!(matches!(client_result, Err(PeerError::Disconnected(_))));
        assert!(matches!(
            server.join().unwrap(),
            Err(PeerError::HandshakeFailed(_))
        ));
    }

    #[test]
    fn invalid_json_rejected() {
        let (left, right) = UnixStream::pair().unwrap();
        let mut raw_writer = FrameWriter::new(left);
        raw_writer.send(CONTROL, b"{not-json").unwrap();

        let mut reader = FrameReader::new(right.try_clone().unwrap());
        let mut writer = FrameWriter::new(right);
        let result = handshake_server(&mut reader, &mut writer, &[1], "peer-5");

        assert!(matches!(result, Err(PeerError::Json(_))));
    }

    #[test]
    fn legacy_string_auth_token_rejected() {
        let (left, right) = UnixStream::pair().unwrap();
        let mut raw_writer = FrameWriter::new(left);
        raw_writer
            .send(
                CONTROL,
                br#"{"protocol":"ipcprims","version":"1.0","channels":[1],"auth_token":"token-123"}"#,
            )
            .unwrap();

        let mut reader = FrameReader::new(right.try_clone().unwrap());
        let mut writer = FrameWriter::new(right);
        let result = handshake_server(&mut reader, &mut writer, &[1], "peer-string-token");

        assert!(matches!(result, Err(PeerError::Json(_))));
    }

    #[test]
    fn present_empty_auth_token_rejected() {
        let (left, right) = UnixStream::pair().unwrap();
        let mut raw_writer = FrameWriter::new(left);
        raw_writer
            .send(
                CONTROL,
                br#"{"protocol":"ipcprims","version":"1.0","channels":[1],"auth_token":[]}"#,
            )
            .unwrap();

        let mut reader = FrameReader::new(right.try_clone().unwrap());
        let mut writer = FrameWriter::new(right);
        let result = handshake_server(&mut reader, &mut writer, &[1], "peer-empty-token");

        assert!(matches!(result, Err(PeerError::Json(_))));
    }

    #[test]
    fn auth_token_serializes_compactly_under_default_payload_cap() {
        let request = HandshakeRequest {
            protocol: "ipcprims".to_string(),
            version: "1.0".to_string(),
            channels: vec![1],
            auth_token: Some(Zeroizing::new(vec![0xff; MAX_AUTH_TOKEN_LEN])),
        };

        let payload = serde_json::to_vec(&request).expect("request should serialize");
        assert!(
            payload.len() <= HandshakeConfig::default().max_handshake_payload,
            "serialized payload length {} exceeded default cap",
            payload.len()
        );

        let value: serde_json::Value =
            serde_json::from_slice(&payload).expect("request should be valid JSON");
        assert_eq!(value["auth_token"]["hex"].as_str().unwrap().len(), 8192);
    }

    #[test]
    fn handshake_timeout() {
        let mut reader = FrameReader::new(AlwaysTimedOutReader);
        let mut writer = FrameWriter::new(Cursor::new(Vec::<u8>::new()));
        let cfg = HandshakeConfig {
            timeout: Duration::from_millis(25),
            ..HandshakeConfig::default()
        };

        let result = handshake_client_with_config(&mut reader, &mut writer, &[1], &cfg);
        assert!(matches!(result, Err(PeerError::Timeout(_))));
    }

    #[test]
    fn version_mismatch() {
        let (left, right) = UnixStream::pair().unwrap();

        let server = thread::spawn(move || {
            let mut reader = FrameReader::new(left.try_clone().unwrap());
            let mut writer = FrameWriter::new(left);
            let cfg = HandshakeConfig {
                protocol_version: "2.0".to_string(),
                ..HandshakeConfig::default()
            };
            handshake_server_with_config(&mut reader, &mut writer, &[1], "peer-6", &cfg)
        });

        let mut reader = FrameReader::new(right.try_clone().unwrap());
        let mut writer = FrameWriter::new(right);
        let result = handshake_client(&mut reader, &mut writer, &[1]);

        assert!(matches!(result, Err(PeerError::Disconnected(_))));
        assert!(matches!(
            server.join().unwrap(),
            Err(PeerError::HandshakeFailed(_))
        ));
    }

    #[test]
    fn uds_integration_handshake_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "ipcprims-peer-handshake-uds-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let sock_path = dir.join("test.sock");
        let listener = ipcprims_transport::UnixDomainSocket::bind(&sock_path).unwrap();

        let (tx, rx) = mpsc::channel();

        let server = thread::spawn(move || {
            let stream = listener.accept().unwrap();
            let mut reader = FrameReader::new(stream.try_clone().unwrap());
            let mut writer = FrameWriter::new(stream);
            let result = handshake_server(&mut reader, &mut writer, &[1, 2], "peer-uds").unwrap();
            tx.send(result).unwrap();
        });

        let stream = ipcprims_transport::UnixDomainSocket::connect(&sock_path).unwrap();
        let mut reader = FrameReader::new(stream.try_clone().unwrap());
        let mut writer = FrameWriter::new(stream);
        let client_result = handshake_client(&mut reader, &mut writer, &[2, 3]).unwrap();

        server.join().unwrap();
        let server_result = rx.recv().unwrap();

        assert_eq!(client_result.peer_id, "peer-uds");
        assert_eq!(client_result.negotiated_channels, vec![2]);
        assert_eq!(server_result.negotiated_channels, vec![2]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reject_control_channel_in_negotiation() {
        let mut reader = FrameReader::new(Cursor::new(Vec::<u8>::new()));
        let mut writer = FrameWriter::new(Cursor::new(Vec::<u8>::new()));
        let client_result = handshake_client(&mut reader, &mut writer, &[CONTROL, 1]);

        assert!(matches!(client_result, Err(PeerError::HandshakeFailed(_))));
    }

    #[test]
    fn auth_token_passthrough() {
        let (left, right) = UnixStream::pair().unwrap();

        let server = thread::spawn(move || {
            let mut reader = FrameReader::new(left.try_clone().unwrap());
            let mut writer = FrameWriter::new(left);
            handshake_server(&mut reader, &mut writer, &[1], "peer-auth").unwrap()
        });

        let mut reader = FrameReader::new(right.try_clone().unwrap());
        let mut writer = FrameWriter::new(right);
        let cfg = HandshakeConfig {
            auth_token: Some(Zeroizing::new(b"token-123".to_vec())),
            ..HandshakeConfig::default()
        };
        let client_result =
            handshake_client_with_config(&mut reader, &mut writer, &[1], &cfg).unwrap();
        let server_result = server.join().unwrap();

        assert!(client_result.client_auth_token.is_none());
        assert_eq!(
            server_result
                .client_auth_token
                .as_ref()
                .map(|token| token.as_slice()),
            Some(b"token-123".as_slice())
        );
    }

    #[test]
    fn auth_token_allows_interior_nul_bytes() {
        let (left, right) = UnixStream::pair().unwrap();

        let server = thread::spawn(move || {
            let mut reader = FrameReader::new(left.try_clone().unwrap());
            let mut writer = FrameWriter::new(left);
            handshake_server(&mut reader, &mut writer, &[1], "peer-auth").unwrap()
        });

        let mut reader = FrameReader::new(right.try_clone().unwrap());
        let mut writer = FrameWriter::new(right);
        let cfg = HandshakeConfig {
            auth_token: Some(Zeroizing::new(b"token\0with\0nul".to_vec())),
            ..HandshakeConfig::default()
        };
        let client_result =
            handshake_client_with_config(&mut reader, &mut writer, &[1], &cfg).unwrap();
        let server_result = server.join().unwrap();

        assert!(client_result.client_auth_token.is_none());
        assert_eq!(
            server_result
                .client_auth_token
                .as_ref()
                .map(|token| token.as_slice()),
            Some(b"token\0with\0nul".as_slice())
        );
    }

    #[test]
    fn max_auth_token_passthrough() {
        let (left, right) = UnixStream::pair().unwrap();
        let expected = vec![0xff; MAX_AUTH_TOKEN_LEN];
        let expected_server = expected.clone();

        let server = thread::spawn(move || {
            let mut reader = FrameReader::new(left.try_clone().unwrap());
            let mut writer = FrameWriter::new(left);
            handshake_server(&mut reader, &mut writer, &[1], "peer-auth-max").unwrap()
        });

        let mut reader = FrameReader::new(right.try_clone().unwrap());
        let mut writer = FrameWriter::new(right);
        let cfg = HandshakeConfig {
            auth_token: Some(Zeroizing::new(expected)),
            ..HandshakeConfig::default()
        };
        let client_result =
            handshake_client_with_config(&mut reader, &mut writer, &[1], &cfg).unwrap();
        let server_result = server.join().unwrap();

        assert!(client_result.client_auth_token.is_none());
        assert_eq!(
            server_result
                .client_auth_token
                .as_ref()
                .map(|token| token.as_slice()),
            Some(expected_server.as_slice())
        );
    }

    #[test]
    fn rejects_oversized_auth_token() {
        let mut reader = FrameReader::new(Cursor::new(Vec::<u8>::new()));
        let mut writer = FrameWriter::new(Cursor::new(Vec::<u8>::new()));
        let cfg = HandshakeConfig {
            auth_token: Some(Zeroizing::new(vec![b'x'; MAX_AUTH_TOKEN_LEN + 1])),
            ..HandshakeConfig::default()
        };
        let client_result = handshake_client_with_config(&mut reader, &mut writer, &[1], &cfg);
        assert!(matches!(client_result, Err(PeerError::HandshakeFailed(_))));
    }

    #[test]
    fn rejects_oversized_handshake_payload() {
        let (left, right) = UnixStream::pair().unwrap();

        let server = thread::spawn(move || {
            let mut reader = FrameReader::new(left.try_clone().unwrap());
            let mut writer = FrameWriter::new(left);
            let cfg = HandshakeConfig {
                max_handshake_payload: 64,
                ..HandshakeConfig::default()
            };
            handshake_server_with_config(&mut reader, &mut writer, &[1], "peer-limit", &cfg)
        });

        let mut reader = FrameReader::new(right.try_clone().unwrap());
        let mut writer = FrameWriter::new(right);
        let cfg = HandshakeConfig {
            auth_token: Some(Zeroizing::new(vec![b'a'; 256])),
            ..HandshakeConfig::default()
        };
        let result = handshake_client_with_config(&mut reader, &mut writer, &[1], &cfg);
        assert!(matches!(result, Err(PeerError::Disconnected(_))));
        assert!(matches!(
            server.join().unwrap(),
            Err(PeerError::HandshakeFailed(_))
        ));
    }

    #[test]
    fn debug_output_redacts_auth_token() {
        let request = HandshakeRequest {
            protocol: "ipcprims".to_string(),
            version: "1.0".to_string(),
            channels: vec![1, 2],
            auth_token: Some(Zeroizing::new(b"super-secret".to_vec())),
        };
        let request_debug = format!("{request:?}");
        assert!(request_debug.contains("<redacted:12 bytes>"));
        assert!(!request_debug.contains("super-secret"));

        let config = HandshakeConfig {
            auth_token: Some(Zeroizing::new(b"another-secret".to_vec())),
            ..HandshakeConfig::default()
        };
        let config_debug = format!("{config:?}");
        assert!(config_debug.contains("<redacted:14 bytes>"));
        assert!(!config_debug.contains("another-secret"));

        let result = HandshakeResult {
            peer_id: "peer-1".to_string(),
            protocol_version: "1.0".to_string(),
            negotiated_channels: vec![1],
            client_auth_token: Some(Zeroizing::new(b"token-123".to_vec())),
        };
        let result_debug = format!("{result:?}");
        assert!(result_debug.contains("<redacted:9 bytes>"));
        assert!(!result_debug.contains("token-123"));
    }

    struct AlwaysTimedOutReader;

    impl Read for AlwaysTimedOutReader {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(ErrorKind::TimedOut))
        }
    }
}
