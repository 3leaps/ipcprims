# SDR-0004: auth_token and Peer Credentials Boundary

**Status**: Accepted
**Date**: 2026-02-09
**Deciders**: Architecture Council

## Context

Phase 2 `secrev` review (P2-C) requires explicit handling guidance for:

- auth token exposure and logging risk
- credential lifetime in memory after handshake
- transport confidentiality assumptions for local IPC
- replay/idempotency responsibilities at the ipcprims boundary

ipcprims provides authentication mechanisms (`auth_token` passthrough and
Linux peer credentials), but policy enforcement lives in consumers.

## Decision

1. Token-bearing debug output must be redacted.

`HandshakeRequest`, `HandshakeConfig`, and `HandshakeResult` redact token fields
in `Debug` output.

2. Peer exposes an explicit token-consumption API.

`Peer::take_client_auth_token()` allows consumers to move token material out of
the peer object and clear the in-memory copy as soon as policy checks complete.
`Peer::handshake_result()` intentionally does not expose token material.

3. UDS confidentiality assumptions are explicit.

`auth_token` is transported as local IPC payload data (not encrypted by
ipcprims). Security relies on local trust boundaries:

- UDS filesystem permissions (default hardened to `0o600`)
- process identity controls
- optional `peer_credentials()` verification on platforms that support it

4. Replay protection remains consumer policy.

ipcprims does not implement token nonce/timestamp caches or replay ledgers.
Consumers should enforce replay/idempotency policy where needed.

## Consequences

**Positive:**

- Lowers accidental secret leakage risk in diagnostics.
- Gives consumers a clear way to reduce token residence time in memory.
- Clarifies transport threat model for auth material.
- Keeps mechanism/policy split consistent with existing SDR-0001.

**Trade-offs:**

- Replay defense still requires consumer-side implementation.
- `auth_token` remains an optional opaque string, not a full auth protocol.

## References

- D2-03 punchlist: P2-C
- `crates/ipcprims-peer/src/handshake.rs`
- `crates/ipcprims-peer/src/peer.rs`
- `crates/ipcprims-transport/src/uds.rs`
- `crates/ipcprims-transport/src/traits.rs`
