# Release Notes

> **Purge policy**: This file retains the **latest 3 releases** in reverse chronological order.
> Older entries are archived to `docs/releases/v<semver>.md` and removed from this file.
> For the complete changelog, see `CHANGELOG.md`.

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

## v0.1.2 — 2026-02-15

Release pipeline: multi-platform FFI build matrix, SBOM generation, and structured draft releases. Resolves the v0.1.1 known issue — release.yml is now a full production pipeline.

### Highlights

- **release.yml rewrite**: 14 release-specific jobs (35 total with CI call) — builds CLI for 6 platforms (Linux 4 + macOS 2), FFI for 8 platforms (adds Windows x64 GNU/MSVC + arm64 MSVC), generates C header via cbindgen, CycloneDX SBOM via syft, and packages a structured FFI bundle with `MANIFEST.json`
- **Peer crate Windows compilation**: cfg-gated `ipcprims-peer` Unix-specific imports so `ipcprims-ffi` compiles on all 3 Windows targets (msvc x64, gnu x64, msvc arm64) — prerequisite for Windows FFI builds in release pipeline
- **Draft release with platform matrix**: release creates a DRAFT GitHub release with CLI archives, FFI bundle, C header, SBOM, licenses, and a platform support table in the body

### Known Issues

- **Go prebuilt libs**: Not yet populated — `go-bindings.yml` must run before tagging (d4-02).

### What's Next

- **v0.1.2 pre-tag**: Run `go-bindings.yml` to populate prebuilt libs, merge PR, then tag
- **v0.2.0**: Tokio-native async API, named pipe transport for Windows, TCP transport (per DDR-0001), CLI P2 commands

Full release details: [docs/releases/v0.1.2.md](docs/releases/v0.1.2.md)
