# ADR-0001: AsyncPeer Receive Model (Arrival Order, Split Handles, Bounded Buffers)

**Status**: Accepted
**Date**: 2026-02-24
**Deciders**: devlead, devrev (four-eyes)

## Context

ipcprims v0.2.0 adds a Tokio async API surface behind the existing `async` feature flag.
The sync `Peer` provides:

- `recv()` that returns frames in on-wire arrival order (excluding CONTROL auto-handling)
- `recv_on(channel)` that filters a channel while buffering off-channel frames
- buffer hardening: per-channel frame count and global total-bytes caps (SDR-0002)

The async design brief calls for a background reader task that demultiplexes frames into
per-channel `mpsc` queues, and also a `recv()`/`Stream` API that yields frames "in arrival order".

Three design tensions must be resolved before implementation to avoid churn:

1. **Arrival ordering vs per-channel receivers**
   A `recv()` implemented by selecting across multiple per-channel receivers cannot
   guarantee strict arrival order.

2. **Receiver ownership vs `&mut self`**
   Returning `&mut mpsc::Receiver<Frame>` or requiring `&mut self` to receive fights the
   async goal of concurrent send/receive and pushes users toward coarse-grained mutexes.

3. **Buffering semantics and memory amplification**
   Count-only bounds (for example "256 frames per channel") can reintroduce a large memory
   amplification surface when frames are near `DEFAULT_MAX_PAYLOAD`.

## Decision

### 1. Define a single arrival-ordered receive path

AsyncPeer MUST provide an "any frames" receive path that preserves on-wire arrival order.

- The background reader task reads frames sequentially from the transport.
- For each non-CONTROL frame, it enqueues the frame onto a single `any_tx` queue in the
  same order the frames were decoded.
- `AsyncPeerRx::recv()` and any `Stream` implementation (for `AnyReceiver`) are driven exclusively from `any_rx`.

This is the only mechanism allowed to claim "arrival order" across channels.

### 2. Provide per-channel receivers as convenience fanout

AsyncPeer MAY provide per-channel receivers for ergonomic demux.

- The same reader task also sends each non-CONTROL frame to a per-channel sender
  (if that channel is enabled/negotiated).
- Per-channel receivers preserve FIFO order within that channel.
- Per-channel receivers do NOT provide global ordering guarantees across channels.
- Fanout is **tee semantics**: if a consumer drains both `any_rx` and a per-channel receiver,
  the same frame will be observed twice by design (once via each path).

### 3. Use a split-handle API to avoid `&mut self` contention

The async peer API MUST support concurrent usage without requiring callers to wrap the
entire peer in a mutex.

Preferred shape:

- `AsyncPeer::into_split() -> (AsyncPeerTx, AsyncPeerRx)`
- `AsyncPeerTx` owns the write path (`send`, `send_json`, control sends).
- `AsyncPeerRx` owns the read path (`recv`, `any_stream`, channel receivers).

Per-channel receiver access is by ownership, not by `&mut` borrowing:

- `AsyncPeerRx::take_channel_receiver(channel: u16) -> Option<mpsc::Receiver<Frame>>`
- `AsyncPeerRx::take_any_receiver() -> mpsc::Receiver<Frame>` (if `recv()` is not used)

### 4. Enforce both per-channel and global byte-based buffering bounds

AsyncPeer buffering MUST preserve the hardening intent of SDR-0002:

- `max_buffer_per_channel` (frame count)
- `max_total_buffered_bytes` (payload + framing overhead)

Enforcement requirements:

- The reader task accounts buffered bytes per decoded frame payload, held once and released
  when the last queued reference is dropped (a shared permit/guard pattern). This avoids
  artificially halving effective capacity when frames are fanned out to multiple queues.
- If enqueue would exceed limits, AsyncPeer treats this as a protocol/resource violation
  and disconnects (consumer receives an error) rather than silently dropping frames.

Note: This intentionally differs from a pure "per-channel N frames" design because count-only
limits are insufficient against near-max-payload flooding.

### 5. CONTROL frames remain internal

CONTROL frames are handled internally in the reader task (ping/pong, shutdown) and are not
delivered via `any_rx` or per-channel receivers, matching the sync peer behavior.

## Consequences

**Positive:**

- `recv()` and `Stream` can make a defensible "arrival order" guarantee.
- Async send/receive can be driven concurrently without coarse-grained mutexing.
- Buffering behavior remains defensible against memory amplification attacks (SDR-0002).

**Trade-offs:**

- Fanout to both `any_rx` and per-channel queues duplicates queue entries.
  This is acceptable because payloads are `Bytes` (refcounted) in frames; however, queue
  metadata still adds overhead and must be budgeted.
- Users who only want per-channel receivers should be guided to avoid also consuming from
  `any_rx` to reduce total buffering.

## Alternatives Considered

1. **Select across per-channel receivers for `recv()`**
   Rejected: cannot guarantee strict arrival order.

2. **Expose only a single arrival-ordered receiver**
   Rejected: forces all demux into consumer code, loses ergonomic per-channel routing.

3. **Expose only per-channel receivers**
   Rejected: cannot support a global arrival-ordered stream contract.

## Test Strategy (Non-Normative)

- Interleave frames across channels and assert `recv()` yields in arrival order.
- Assert per-channel receivers see only their channel and preserve FIFO within that channel.
- Flood near-max-payload frames across multiple channels and assert `max_total_buffered_bytes`
  is enforced (disconnect/error rather than unbounded growth).

## References

- `docs/decisions/SDR-0002-peer-transport-hardening-defaults.md`
- `docs/decisions/SDR-0005-ordering-and-replay-boundary.md`
- `crates/ipcprims-peer/src/peer.rs`
- `crates/ipcprims-frame/src/codec.rs`
