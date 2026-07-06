# Release Notes

> **Purge policy**: This file retains the **latest 3 releases** in reverse chronological order.
> Older entries are archived to `docs/releases/v<semver>.md` and removed from this file.
> For the complete changelog, see `CHANGELOG.md`.

---

## v0.2.2 — 2026-07-06

**Breaking wire change for token-authenticated peers:** token-bearing handshakes now encode `auth_token` as bytes rather than a UTF-8 string. No-auth peers remain compatible because the field is omitted, but older token-authenticated peers that send string tokens fail closed. Upgrade both sides together when using token auth.

This release adds the async TypeScript peer surface and hardens auth-token handling across Rust, C FFI, Go, and TypeScript bindings.

### Highlights

- **TypeScript async peer API**: `AsyncPeer`, `AsyncListener`, async receives, async channel receivers, and promise-based send/ping/shutdown are now backed by the Rust async peer implementation.
- **Opaque token bytes**: Auth tokens now cross the Rust handshake, C ABI, Go bindings, and TypeScript bindings as byte buffers instead of strings.
- **Zeroizing ownership**: ipcprims-owned token buffers use zeroizing storage; FFI token retrieval returns caller-owned bytes that must be released with the dedicated zeroizing token free function.
- **Explicit server retrieval**: Server peers expose a distinct no-token state and clear stored token material after retrieval.
- **Binding examples**: Go and TypeScript examples use constant-time comparison patterns with clean length-mismatch rejection before treating a peer as authenticated.

### Compatibility

- **Cross-version token-authenticated peers**: No known deployed cross-version token-authenticated peers exist for this release window; consumers using token auth should upgrade both sides together.

Full release details: [docs/releases/v0.2.2.md](docs/releases/v0.2.2.md)

---

## v0.2.1 — 2026-04-04

Windows named pipe transport (sync + async), full Windows CI/releng, and developer experience improvements.

### Highlights

- **Windows named pipes (sync + async)**: Complete named pipe transport in `ipcprims-transport` with overlapped I/O timeout enforcement, owner-only DACL for access control, and async transport wrapper — `AsyncPeer` and `AsyncPeerListener` now compile and work on Windows
- **Windows CI expansion**: `windows-test`, `windows-test-async`, and `windows-dogfood` jobs; Windows CLI build jobs (x64 + arm64) in release workflow
- **Peer disconnect handling**: `BrokenPipe`/`ConnectionReset` reclassified as `Disconnected` (not `Fatal`) — fixes Windows pipe closure behavior
- **Dev tooling**: `make doctor-env` for environment diagnostics; `make check-unix-clippy` for cross-host lint coverage
- **npm publish fixes**: Idempotent (skips already-published), OIDC npmrc fix, registry API verification

### Platform Scope

- **Windows x64 (sync + async)**: Supported via named pipes
- **Windows ARM64 (sync + async)**: Supported via named pipes
- **Developer guides**: `docs/guides/windows-dev-setup.md` and `docs/guides/windows-arm64-rough-edges.md`

### Known Issues

- **Transitive dep duplication**: `getrandom` (0.2 + 0.3) and `windows-sys` (0.60 + 0.61) via `jsonschema`. No functional impact.

### What's Next

- **v0.3.0**: TCP transport (per DDR-0001), CLI P2 commands

Full release details: [docs/releases/v0.2.1.md](docs/releases/v0.2.1.md)

---

## v0.2.0 — 2026-02-26

Tokio-native async API on Unix (UDS). First minor version bump adding new public API surface since v0.1.0.

### Highlights

- **Async (Tokio, Unix-only)**: Full async stack behind `async` feature flag — `AsyncUnixDomainSocket`/`AsyncIpcStream` transport, `IpcCodec` for `tokio_util::codec::Framed*`, `AsyncPeer` with split Tx/Rx handles, and `async_connect()` convenience function
- **AsyncPeer design**: Background reader task with per-channel `mpsc` receivers, optional external `CancellationToken` for structured shutdown, and automatic reader task cancellation on drop
- **MSRV consistency**: Core crates at 1.85.0; `ipcprims-napi` overrides to 1.88.0 (napi-build); `make msrv` target for local verification; tag-triggered CI MSRV matrix
- **Dev tooling**: `make check-windows*` targets for local Windows cross-checks; AI-assisted commit template at `scripts/commit-template-ai.txt`

### Platform Scope

- **Async**: Unix-only in v0.2.0 (Linux x64/arm64, macOS arm64)
- **Windows**: Named pipes deferred to v0.2.1

### Known Issues

- **Windows async**: Deferred to v0.2.1 (sync named pipes + async follow-on)
- **Transitive dep duplication**: `getrandom` (0.2 + 0.3) and `windows-sys` (0.60 + 0.61) via `jsonschema`. No functional impact.

### What's Next

- ~~**v0.2.1**: Windows named pipes~~ — Shipped

Full release details: [docs/releases/v0.2.0.md](docs/releases/v0.2.0.md)

---
