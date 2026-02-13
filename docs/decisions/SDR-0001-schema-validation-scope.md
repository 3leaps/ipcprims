# SDR-0001: Schema Validation at IPC Boundary

**Status**: Accepted
**Date**: 2026-02-06
**Deciders**: Architecture Council

## Context

ipcprims is the IPC substrate for the Lanyte platform, where the IPC Gateway is the primary security enforcement point. The Lanyte security model requires:

- All messages validated against JSON Schema on ingress and egress
- `deny_unknown_fields` enforced on all schemas
- No shared memory between processes
- Separate command and data channels
- Deny-by-default for new connection types
- Rate limiting per peer and per message type
- Audit logging of all cross-boundary messages

ipcprims is also a general-purpose library used outside Lanyte. The schema validation design must serve both the strict security requirements of Lanyte and the lightweight needs of general consumers.

## Decision

### What Lives in ipcprims

**SchemaRegistry** (ipcprims-schema crate, behind `schema` feature flag):

- Load, compile, and store JSON Schema 2020-12 validators keyed by channel ID
- Validate frame payloads against registered schemas
- Configurable strictness via `RegistryConfig`:
  - `strict_mode: bool` — when true, adds `deny_unknown_fields` semantics (additional properties rejected)
  - `fail_on_missing_schema: bool` — when true, frames on channels without a registered schema are rejected
  - `validate_on_send: bool` — validate outbound frames (default: true when registry attached)
  - `validate_on_recv: bool` — validate inbound frames (default: true when registry attached)

- Integration with `Peer`: when a `SchemaRegistry` is attached to a Peer, validation occurs automatically on send/recv. No registry = no validation overhead.

**Graceful Shutdown Protocol** (ipcprims-peer crate):

- Define CONTROL channel message types: `SHUTDOWN_REQUEST`, `SHUTDOWN_ACK`, `SHUTDOWN_FORCE`
- `Peer::shutdown()` sends SHUTDOWN_REQUEST, waits for ACK (with timeout), then closes
- Generally useful for any IPC consumer, not Lanyte-specific

**Ping/Pong Heartbeat** (ipcprims-peer crate):

- Define CONTROL channel message types: `PING`, `PONG`
- Optional keepalive with configurable interval
- `Peer` tracks last heartbeat time, exposes `is_alive()` check
- Generally useful for health monitoring

**Peer Identity in Handshake** (ipcprims-peer crate):

- Extend handshake to carry optional `auth_token` field (opaque bytes)
- Expose `SO_PEERCRED` on Linux via `IpcStream::peer_credentials()` returning (uid, gid, pid)
- These are building blocks; policy enforcement is consumer's responsibility

### What Lives in Lanyte (NOT in ipcprims)

**Rate limiting**: Per-peer, per-message-type rate limiting is application policy. Lanyte Gateway wraps ipcprims `Peer` with rate limiting logic. ipcprims provides the send/recv surface; Lanyte decides what's too fast.

**Audit logging**: Lanyte subscribes to ipcprims `tracing` events and routes to its audit trail. ipcprims emits structured `tracing::info!` events on frame send/recv with channel, size, and peer_id fields. Lanyte's subscriber captures these — no special hooks API needed.

**Channel authorization**: Lanyte decides which peers may use which channels. ipcprims negotiates channels in handshake; Lanyte Gateway enforces which channels a peer is allowed to request.

**Secrets isolation**: Entirely Lanyte scope. ipcprims has no concept of secrets, credentials, or key material (except optional auth_token passthrough in handshake).

**Peer registration and lifecycle policy**: Lanyte manages which services may connect, restart policies, health escalation. ipcprims provides the connection; Lanyte manages the fleet.

### The Boundary Principle

ipcprims provides **mechanisms**. Lanyte provides **policy**.

| Concern           | ipcprims (mechanism)                           | Lanyte (policy)                                   |
| ----------------- | ---------------------------------------------- | ------------------------------------------------- |
| Schema validation | SchemaRegistry with configurable strictness    | Always strict_mode + fail_on_missing_schema       |
| Authentication    | auth_token in handshake + SO_PEERCRED exposure | Verify token against allowed peers list           |
| Rate limiting     | — (not in scope)                               | Per-peer, per-channel rate limiting               |
| Shutdown          | SHUTDOWN_REQUEST/ACK/FORCE protocol            | Coordinated multi-peer shutdown sequence          |
| Audit             | tracing events on send/recv                    | Subscriber captures to tamper-evident audit trail |
| Channel access    | Negotiate intersection in handshake            | Restrict which channels each peer may request     |

## Consequences

**Positive:**

- ipcprims stays general-purpose — no Lanyte-specific policy baked in
- Lanyte gets the building blocks it needs without forking or wrapping awkwardly
- `tracing`-based observability is idiomatic Rust; no custom callback API to maintain
- Schema validation is opt-in (no overhead for consumers who don't need it)

**Negative:**

- Lanyte must build its Gateway layer on top of ipcprims rather than getting enforcement "for free"
- `strict_mode` with deny_unknown_fields requires careful JSON Schema construction (consumers must understand what it means)

**Risks:**

- If ipcprims tracing events change structure, Lanyte's audit subscriber may need updates — mitigated by treating event fields as a documented contract

## References

- Lanyte Security Runtime: IPC boundary model, threat model
- Lanyte Platform Architecture: IPC Gateway specification
- JSON Schema 2020-12: `additionalProperties: false` for deny_unknown_fields semantics
