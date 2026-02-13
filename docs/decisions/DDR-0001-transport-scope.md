# DDR-0001: Transport Scope — IPC-First, Extensible

**Status**: Accepted
**Date**: 2026-02-06
**Deciders**: Architecture Council

## Context

ipcprims provides framed IPC primitives over Unix domain sockets (and future Windows named pipes). The wire format `[magic:2][length:4][channel:2][payload:N]` and the framing layer (`FrameReader<T: Read>`, `FrameWriter<T: Write>`) are already transport-agnostic — they operate over any `Read + Write` stream.

Enterprise architects raised the question: should ipcprims support TCP as a transport, enabling use as a wide-area agent coordination protocol? This would be relevant for Lanyte Phase 5 (agent-to-agent federation), which calls for "lightweight, authenticated wire protocol over TLS" with schema-validated messages.

### Forces

- The framing, channel multiplexing, schema validation, and handshake protocol are all transport-independent
- TCP brings significant new concerns: TLS, mutual authentication, reconnection, connection pooling, service discovery
- IPC (local) and TCP (remote) have fundamentally different trust models — UDS gets filesystem-level auth for free; TCP requires explicit cryptographic auth
- A separate "wireprims" repo would duplicate framing/schema/peer logic or depend on ipcprims crates anyway
- The name "ipcprims" communicates the primary use case; TCP is an additional transport, not a new identity

## Decision

**Keep ipcprims focused on local IPC for v0.1.0. Design the crate structure to support additional transports in future versions behind feature flags.**

Specifically:

1. **Name stays `ipcprims`**. IPC is the primary use case. TCP is an additional transport option, like how HTTP libraries support both Unix sockets and TCP.

2. **No TCP in v0.1.0 scope**. Delivery 2 (Peer, Schema, CLI) proceeds without TCP.

3. **Future TCP support lives in `ipcprims-transport`** behind a `tcp` feature flag. The `Peer` type returns from both `connect()` (IPC) and a future `connect_tcp()` — same Peer, different entry points.

4. **Frame and Schema crates remain unchanged**. They are already generic over the stream type.

5. **TLS and mutual auth are v0.2.0+ scope**, gated behind `tls` feature flag, likely using `rustls`.

### Transport Roadmap

| Version | Transports                 | Auth Model                        |
| ------- | -------------------------- | --------------------------------- |
| v0.1.0  | UDS (Unix)                 | Filesystem permissions (implicit) |
| v0.1.x  | + Named Pipes (Windows)    | ACL (implicit)                    |
| v0.2.0  | + TCP                      | TLS, optional mutual TLS          |
| v0.2.x  | + TCP+TLS with mutual auth | Agent identity certificates       |

## Consequences

**Positive:**

- v0.1.0 stays focused and shippable
- No unnecessary TLS/networking complexity in the initial release
- The crate structure already supports this evolution (generic `FrameReader<T>`, separate transport crate)
- Lanyte Phase 5 federation can reuse the full ipcprims stack with just a new transport binding

**Negative:**

- Consumers who need TCP now must use `FrameReader<TcpStream>` directly without Peer-layer conveniences
- Feature flag matrix grows with each transport (testing surface increases)

**Neutral:**

- The `Peer` API in D2 must be designed so it doesn't hardcode `IpcStream` — it should own boxed reader/writer or use internal dispatch

## References

- Lanyte Platform Architecture: agent-to-agent wire protocol (Phase 5)
- Lanyte Security Runtime: IPC boundary model
