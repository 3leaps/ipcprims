# ADR-0002: Auth Token Handshake FFI Contract

> **Status**: Proposed
> **Date**: 2026-07-05
> **Authors**: entarch

## Context

ipcprims already carries an optional handshake `auth_token` in the Rust peer
surface and SDR-0004 defines the governing boundary: ipcprims transports token
material and exposes peer credentials, but verification remains consumer policy.

The current token representation is `Option<String>`. That is not a safe
contract for the cross-language FFI surface:

- it constrains tokens to UTF-8 text;
- a C string boundary would truncate at an interior NUL byte;
- token material is cloned and dropped as ordinary `String` data;
- the server retrieve path currently returns `Option<String>`, which is not an
  explicit bytes ownership contract for Go, TypeScript, or future bindings.

The FFI and binding work needs a stable contract before implementation. The
existing no-auth connect/listen APIs must remain unchanged, but authenticated
handshake plumbing must carry arbitrary bytes, scrub memory where ipcprims owns
it, and keep the consumer-policy comparison model from SDR-0004.

This ADR supersedes SDR-0004 only for token representation and FFI export
requirements. SDR-0004 remains authoritative for the mechanism/policy split,
local IPC confidentiality assumptions, and replay responsibility.

## Decision

### 1. Rust token material is opaque zeroizing bytes

The Rust handshake token surface changes from `Option<String>` to
`Option<Zeroizing<Vec<u8>>>` or an equivalent zeroizing byte owner.

This applies to the token-bearing handshake structures and stored peer token:

- `HandshakeConfig.auth_token`
- `HandshakeRequest.auth_token`
- `HandshakeResult.client_auth_token`
- the server-side token retained for `Peer::take_client_auth_token()`

An absent token remains represented as `None`. An empty token is invalid token
material and must not be accepted as a successful authentication input.

Debug output for every token-bearing structure remains redacted and may report
only non-secret metadata such as byte length.

### 2. Token-bearing handshake wire changes to bytes

The handshake request field remains optional, but when present it carries
opaque bytes, not a UTF-8 string. The serde wire representation must be treated
as a token-bearing wire change.

A present-but-empty token on the wire is invalid token material. Decoders must
reject it or preserve it as an explicit empty-token rejection state; they must
not normalize it to `None`, because `None` is reserved for "no token presented."

Compatibility rule:

- no-auth peers remain compatible because the token field is omitted;
- old token-authenticated peers that send a string token are intentionally
  incompatible with the byte-token contract and must fail closed;
- no cross-version token-authenticated peers are known to exist, so the risk is
  low for this release window;
- future token-bearing wire changes require an explicit compatibility decision.

### 3. FFI auth-token input is bytes plus length

The C-ABI must not accept token input through NUL-terminated strings.

Authenticated connect/listen entry points are additive to the existing no-auth
APIs. They must accept token material as `(const uint8_t *data, uintptr_t len)`
or an equivalent byte-buffer field inside a handshake config object.

Required input semantics:

- `data == NULL` with `len == 0` means no token is presented;
- `data == NULL` with `len > 0` is invalid;
- `len == 0` with non-null data is invalid token material for auth;
- `len > MAX_AUTH_TOKEN_LEN` is rejected before allocating or copying;
- no error, log, panic, or debug path formats token bytes.

The existing no-auth `ipc_connect` and listener accept path keep their current
meaning and remain the ergonomic default.

### 4. Server retrieve is explicit and zeroizing

The server-side retrieve API must move the presented client token out of the
peer and clear ipcprims' stored copy, matching the Rust `take` semantics.

The FFI retrieve API must return three distinct states:

- operation failed, using the normal `IpcResult` error taxonomy;
- operation succeeded and no token was presented;
- operation succeeded and token bytes were returned.

"No token presented" must be impossible to confuse with an empty token. The
language bindings should expose this as a distinct result shape, not as a
nullable byte slice that can be skipped accidentally.

Returned token bytes are caller-owned and must be freed with a dedicated
zeroizing free function. The generic string free function and frame free
function are not valid for token buffers.

Binding documentation must tell consumers to compare the returned token and free
it immediately afterward.

### 5. Comparison stays consumer policy, but examples are security controls

ipcprims does not add a built-in token equality helper. Consumers decide whether
and how to authorize a peer, consistent with SDR-0004.

The shipped TypeScript, Go, and Rust examples are part of the security contract
for this surface. They must:

- treat absent and empty presented tokens as rejection;
- use a language-appropriate constant-time primitive;
- handle a length mismatch as a clean rejection, not an error: Node's
  `crypto.timingSafeEqual` throws a `RangeError` on unequal-length inputs, so the
  example must reject a wrong-length token before or instead of letting it throw
  (Go's `crypto/subtle.ConstantTimeCompare` returns `0` on length mismatch). A
  comparison exception must never escape into a fail-open path;
- warn explicitly that `==`, string comparison, `bytes.Equal`, or equivalent
  ordinary equality is a timing oracle for token checks;
- fail closed before exposing the peer as authenticated application state.

Expected primitives:

- Go: `crypto/subtle.ConstantTimeCompare`
- TypeScript/Node/Bun: `crypto.timingSafeEqual`
- Rust: `subtle` or an equivalent constant-time comparison primitive

When an example or binding helper rejects a peer because the token is absent,
empty, or mismatched, it must surface the existing handshake-failed error class
to the caller and close the unauthenticated peer promptly.

### 6. Binding and API coherence

The TypeScript and Go bindings must follow the same auth/handshake idiom:

- current no-auth connect/listen calls stay source-compatible;
- authenticated variants are opt-in and additive;
- token input is bytes, not strings;
- server-side token retrieval is a distinct operation with explicit ownership;
- typed handshake failure is preserved across the binding boundary.

This mirrors the sysprims session-verb FFI pattern: additive exports, explicit
ownership, typed errors, binding documentation that preserves the native
contract, and no parallel vocabulary per language.

The authenticated connect surface must compose with ADR-0001's async receive
model so async and auth do not become divergent binding paths.

## Consequences

### Positive

- Token auth can cross Rust, C, Go, and TypeScript without losing arbitrary
  bytes at UTF-8 or NUL-terminated boundaries.
- ipcprims-owned token memory is scrubbed on drop and on retrieve.
- Server retrieval has an explicit no-token state, reducing accidental
  fail-open examples.
- The existing no-auth API remains stable.
- The FFI contract stays aligned with the sysprims additive-export pattern.

### Negative

- The token-bearing handshake wire format is incompatible with the previous
  Rust-only string-token representation.
- Bindings need a dedicated zeroizing token-buffer free path in addition to the
  existing string and frame free paths.
- Examples carry security significance and must be reviewed with the same care
  as implementation code.

### Neutral

- This ADR does not make ipcprims the authorization policy engine.
- This ADR does not add replay protection, nonce caches, or token rotation.
- This ADR does not change local IPC confidentiality assumptions from SDR-0004
  and SDR-0002.

## Alternatives Considered

### Alternative 1: Keep `String` in Rust and convert at FFI

Rejected. It preserves the UTF-8 constraint and keeps token material in ordinary
string allocations. It also leaves the FFI contract vulnerable to future
NUL-terminated adapters that silently compare prefixes.

### Alternative 2: Accept C strings for token input

Rejected. C strings cannot carry arbitrary bytes and truncate at interior NUL
bytes, which is exactly the failure mode this contract must prevent.

### Alternative 3: Add a built-in token comparison helper

Rejected. SDR-0004 deliberately keeps verification policy in the consumer. The
examples and binding docs must demonstrate safe comparison, but ipcprims should
not become an authorization framework.

### Alternative 4: Defer zeroization

Rejected. The representation change is already required for opaque bytes, and
using a zeroizing byte owner resolves token lifetime hygiene in the same
contract move.

## Test Strategy

- Rust handshake tests cover arbitrary byte tokens, interior NUL bytes,
  oversized rejection, empty-token rejection, redacted debug output, and
  `take_client_auth_token()` clearing stored state.
- FFI tests cover null/length validation, `MAX_AUTH_TOKEN_LEN` enforcement
  before allocation, retrieve-present/no-token states, and zeroizing free usage.
- TypeScript and Go tests run authenticated handshake examples with correct,
  missing, empty, and mismatched tokens.
- Binding type tests ensure token input is bytes-oriented and no string-only
  auth overload becomes the documented path.

## References

- SDR-0004: `docs/decisions/SDR-0004-auth-token-and-credentials-boundary.md`
- SDR-0002: `docs/decisions/SDR-0002-peer-transport-hardening-defaults.md`
- ADR-0001: `docs/decisions/ADR-0001-async-peer-receive-model.md`
- `crates/ipcprims-peer/src/handshake.rs`
- `crates/ipcprims-peer/src/peer.rs`
- `crates/ipcprims-ffi/src/`
- sysprims ADR-0016: session-spawn FFI contract
