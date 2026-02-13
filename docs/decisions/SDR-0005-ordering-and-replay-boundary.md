# SDR-0005: Ordering and Replay Boundary

**Status**: Accepted
**Date**: 2026-02-09
**Deciders**: Architecture Council

## Context

Phase 2 `secrev` scope item P2-F requests explicit guidance on ordering,
replay handling, and idempotency responsibilities at the ipcprims boundary.

ipcprims provides framed transport, channel multiplexing, and simple request
helpers, but does not impose application-level message identity semantics.

## Decision

1. Ordering guarantees are transport/frame scoped.

- Byte-stream ordering is preserved by underlying transport.
- Frame decode preserves on-wire order.
- Per-channel buffered queues preserve FIFO order for queued frames.

2. Replay/idempotency are consumer policy.

ipcprims does not implement:

- replay caches
- nonce/timestamp validation
- duplicate suppression ledgers
- idempotency-key storage

Consumers that need replay resistance or idempotent command processing must
embed message identity (for example correlation IDs/nonces) in payload schemas
and enforce policy in their service layer.

3. `Peer::request*` remains intentionally minimal.

`Peer::request`/`Peer::request_json` provide a single in-flight convenience
pattern on the COMMAND channel. They do not perform semantic response matching
beyond channel selection.

For CLI parity, `ipcprims send --wait` is channel-correlated and waits on the
same channel that was sent, reducing accidental cross-channel capture.

## Consequences

**Positive:**

- Keeps ipcprims generic and policy-neutral.
- Avoids hard-coding an identity format across all consumers.
- Aligns with existing mechanism/policy split from SDR-0001 and SDR-0004.

**Trade-offs:**

- Consumers needing strict replay protection must implement extra logic.
- Request helpers are not sufficient for advanced multiplexed RPC semantics.

## References

- D2-03 punchlist: P2-F
- `crates/ipcprims-peer/src/peer.rs`
- `docs/decisions/SDR-0001-schema-validation-scope.md`
- `docs/decisions/SDR-0004-auth-token-and-credentials-boundary.md`
