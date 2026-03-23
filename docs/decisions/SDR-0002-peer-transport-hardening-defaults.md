# SDR-0002: Peer and Transport Hardening Defaults

**Status**: Accepted
**Date**: 2026-02-08
**Deciders**: Architecture Council

## Context

Security review D2-03 identified high-severity gaps in peer channel enforcement,
pre-auth handshake resource bounds, and Unix socket path permissions.

The system accepts untrusted local peer input. Security controls must be
mechanism-default, with any relaxation explicit and intentional.

## Decision

### 1. Enforce negotiated channels on ingress

`Peer` receive paths (`recv`, `recv_on`, and internal wait loops) must reject
frames on channels not negotiated in handshake (except CONTROL channel handling).

Behavior:

- Unnegotiated inbound channel -> disconnect with protocol error.

### 2. Bound buffered frame memory globally

`PeerConfig` includes:

- `max_buffer_per_channel` (existing)
- `max_total_buffered_bytes` (new)

Both limits are enforced for buffered off-channel frames.

### 3. Bound pre-auth handshake payload size

`HandshakeConfig` includes `max_handshake_payload` (default: 16 KiB).

Handshake reader/writer construction uses this value as frame max payload for
pre-auth handshake only.

After successful handshake, runtime frame limits are restored to normal payload
bounds (`DEFAULT_MAX_PAYLOAD`) for regular peer traffic.

Additional auth-token bound:

- `auth_token` max length is enforced (default max: 4096 bytes).

### 4. Harden Unix socket permissions by default

`UnixDomainSocket::bind` must set socket filesystem mode to `0o600`
(owner read/write only) after bind.

Broader permissions require explicit opt-in through
`UnixDomainSocket::bind_with_mode(path, mode)`.

Stale path handling is also hardened:

- Existing path is removed only when it is a Unix socket.
- Existing non-socket path causes bind failure.
- Drop cleanup unlinks only when current path still matches the socket identity
  (device + inode) created by this listener.

### 5. Harden Windows named pipe permissions by default

`NamedPipeListener::accept` creates each pipe instance with:

- `PIPE_REJECT_REMOTE_CLIENTS` — the kernel denies connections from remote machines.
- An explicit owner-only DACL security descriptor — grants `GENERIC_ALL` to the
  current process owner's SID only. This is the Windows equivalent of Unix `0o600`.

The DACL is constructed at each `accept` call from the process token's user SID
(`GetTokenInformation(TokenUser)`). No fallback to the process default DACL occurs.

**`peer_credentials()` on Windows**: not implemented in v0.2.1. Returns `None`.
Windows does not have a direct equivalent of `SO_PEERCRED`. Future releases may
expose the peer process ID via `GetNamedPipeClientProcessId` behind a
platform-aware identity type, but v0.2.1 does not claim Windows peer identity
is available for authorization decisions.

### 6. Make frame encoding length conversion explicit and checked

`encode_frame` is fallible and returns `Result<()>`.

It rejects payload sizes above `u32::MAX` instead of silently truncating with
an unchecked cast.

## Consequences

**Positive:**

- Stronger protocol boundary enforcement for negotiated channels.
- Reduced memory-abuse surface from buffered off-channel traffic.
- Reduced pre-auth resource abuse surface in handshake.
- Handshake cap no longer constrains post-auth runtime payload capacity.
- Predictable secure default for UDS access control.
- Predictable secure default for Windows named pipe access control (owner-only DACL).
- Reduced risk of deleting attacker-replaced files during cleanup.
- No silent framing length truncation on oversized payloads.

**Trade-offs:**

- Minor API change: `encode_frame` call sites now handle `Result`.
- Operators needing shared socket access must opt in via explicit mode.

## References

- D2-03 security review findings: SEC-001, SEC-002, SEC-003, SEC-004, SEC-005
- `crates/ipcprims-peer/src/peer.rs`
- `crates/ipcprims-peer/src/handshake.rs`
- `crates/ipcprims-peer/src/connector.rs`
- `crates/ipcprims-peer/src/listener.rs`
- `crates/ipcprims-transport/src/uds.rs`
- `crates/ipcprims-transport/src/npipes.rs`
- `crates/ipcprims-frame/src/codec.rs`
