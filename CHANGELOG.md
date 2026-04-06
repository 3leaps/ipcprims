# Changelog

All notable changes to this project are documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Purge policy**: This file retains the **latest 10 releases** in reverse chronological order.
> Older entries are archived to `docs/releases/v<semver>.md` and removed from this file.
> For individual release summaries, see `RELEASE_NOTES.md`.

---

## [0.2.1] — 2026-04-04

Windows named pipe transport (sync + async), CI/releng expansion, and developer experience improvements.

### Added

- **Windows named pipes (sync)**: `NamedPipeListener` and `NamedPipeStream` replace the `Unavailable` placeholder in `ipcprims-transport`; peer connect/listen paths wired to named pipes on Windows
- **Windows named pipes (async)**: `AsyncNamedPipeStream` transport wrapper; async peer/platform gates lifted so `AsyncPeer`/`AsyncPeerListener`/`async_connect` compile and work on Windows
- **Named pipe timeouts**: Overlapped I/O enforcement of read/write timeouts on Windows named pipes, with timeout reset/clamp tests
- **Named pipe access control**: Owner-only DACL for Windows named-pipe listener instances (aligns with Unix 0o600 semantics)
- **Windows test coverage**: Multi-client reconnect, DACL verification, async transport roundtrip, timeout behavior, peer unsupported-behavior tests
- **Windows CLI dogfood**: `cli-matrix.sh` supports Windows named pipe paths with retry helper and explicit process tracking
- **Windows CI/releng**: `windows-test`, `windows-test-async`, `windows-dogfood` jobs in CI; Windows CLI build jobs (x64 + arm64) in release workflow
- **Dev tooling**: `make doctor-env` target for environment diagnostics; `make check-unix-clippy` for cross-host lint coverage of Unix-gated code
- **Documentation**: `docs/guides/windows-dev-setup.md`, `docs/guides/windows-arm64-rough-edges.md`, `docs/guides/npm-publishing.md`
- **Roles**: `devrev` role definition for code review agents

### Changed

- **Peer disconnect handling**: `BrokenPipe` and `ConnectionReset` frame errors classified as `Disconnected` (not `Fatal`) — fixes echo server exit on Windows pipe closure
- **CI windows-cross-check**: Scoped to foundation crates (`ipcprims-transport`, `ipcprims-frame`) to match local `make check-windows*` targets
- **npm publish**: Idempotent — skips already-published versions; OIDC npmrc fix ported from sysprims; registry API used for package verification
- **cbindgen**: 0.29.0 → 0.29.2; header refreshed

### Fixed

- **RUSTSEC-2026-0049**: `rustls-webpki` 0.103.9 → 0.103.10 (CRL distribution point matching)
- **npm publish E404**: `setup-node` `always-auth=true` override fixed by writing clean `NPM_CONFIG_USERCONFIG`
- **Windows prepush**: Unix-gated clippy paths caught via `make check-unix-clippy` on Windows hosts
- **Named pipe write-timeout test**: Handle `Ok(0)` to prevent infinite loop on pipe close

### Known Issues

- **Transitive dep duplication**: `getrandom` (0.2 + 0.3) and `windows-sys` (0.60 + 0.61) via `jsonschema` dep tree. No functional impact.

## [0.2.0] — 2026-02-26

Tokio-native async API on Unix (UDS), MSRV consistency fixes, and dev tooling improvements.

### Added

- **Async (Tokio, Unix-only)**: `ipcprims-transport` async UDS transport (`AsyncUnixDomainSocket`, `AsyncIpcStream`), `ipcprims-frame` async codec (`IpcCodec` for `tokio_util::codec::Framed*`), and `ipcprims-peer` async peer API (`AsyncPeer` with split Tx/Rx). (feature: `async`)
- **Async test hardening**: `tokio_util::codec::Framed` integration test for `IpcCodec`, oversize-length rejection regression test, and async UDS hardening parity tests.
- **AsyncPeer cancellation**: optional external `CancellationToken` wiring for structured shutdown, and reader task cancellation on drop (prevents leaked background read tasks).
- **Dev tooling**: `scripts/commit-template-ai.txt` helper for correct AI-assisted commit trailers.
- **Makefile**: `check-windows`, `check-windows-msvc`, `check-windows-gnu`, `check-windows-arm64-msvc` targets for local Windows target `cargo check` (no link) of core crates — mirrors CI `windows-cross-check` with `RUSTFLAGS="-Dwarnings"`.
- **CI MSRV**: tag-triggered `msrv-matrix.yml` workflow for release-time MSRV confirmation (Linux/macOS/Windows).
- **ADR-0001**: accepted design decision for async peer receive model (arrival-ordered global queue vs per-channel fanout).

### Changed

- **PeerConfig**: added `enable_any_delivery` (async-only; defaults to enabled) to allow construction-time disabling of the arrival-ordered `any_rx` queue for channel-only consumers.
- **MSRV policy**: README badge updated to `1.85+`; core crate MSRV remains 1.85.0; `ipcprims-napi` overrides to 1.88.0 (napi-build requirement).
- **Makefile**: added `make msrv` target for local MSRV verification (excludes `ipcprims-napi`).

### Fixed

- **AsyncPeer**: reader task now cancels on drop (fixes resource leak for background read tasks).
- **MSRV alignment**: envinfo test fixture updated to 1.85.0; NAPI Cargo.toml override documented.

## [0.1.2] — 2026-02-15

Release pipeline: multi-platform FFI build matrix, SBOM generation, and structured draft releases — matching the sysprims/docprims release pattern.

### Changed

- **release.yml**: Complete rewrite from 4-job skeleton to 14 release-specific jobs (35 total including CI call) — multi-platform FFI builds, cbindgen header, CycloneDX SBOM, FFI bundle packaging, and draft GitHub release with platform matrix table
- **ipcprims-peer**: cfg-gated Unix-specific imports behind `#[cfg(unix)]` so `ipcprims-ffi` compiles on Windows targets; non-Unix builds return explicit `Unsupported` errors (no fake runtime support)

### Fixed

- **release.yml known issue**: Resolved the v0.1.1 known issue — release pipeline now produces full multi-platform FFI bundles instead of the minimal skeleton

### Known Issues

- **Go prebuilt libs**: Not yet populated — `go-bindings.yml` must run before tagging to create prebuilt libs PR. Tracked as v0.1.2 pre-tag checklist item (d4-02).
- **Transitive dep duplication**: `getrandom` (0.2 + 0.3) and `windows-sys` (0.60 + 0.61) via `jsonschema` dep tree. No functional impact.

## [0.1.1] — 2026-02-15

Infrastructure release: cross-language binding scaffolds (FFI, Go, TypeScript) and CI/release pipeline maturation.

### Added

- **ipcprims-ffi**: C-ABI crate with `staticlib` + `cdylib` outputs; exports for init, listener, peer, frame, schema; `cbindgen`-generated header (`be2dd5d`)
- **Go bindings**: CGo module at `bindings/go/ipcprims` with Listener, Peer, SchemaRegistry; stub FFI bridge for platforms without prebuilt libs (`965fbf1`)
- **TypeScript bindings**: NAPI-RS package `@3leaps/ipcprims` at `bindings/typescript` with Listener, Peer, SchemaRegistry; 5-platform prebuild matrix (`319d037`)
- **CI**: `windows-cross-check` job — `cargo check` for 3 Windows targets (msvc x64, gnu x64, msvc arm64) without SDK
- **CI**: `test-musl` job — build + test on `x86_64-unknown-linux-musl`
- **CI**: `ffi` job — cbindgen header generation + C smoke test (Linux, macOS)
- **CI**: `go-bindings` job — Go build + lint + test (Linux, macOS)
- **CI**: `typescript-bindings` job — NAPI build + test + typecheck (Linux, macOS)
- **Workflows**: `go-bindings.yml` — multi-platform FFI build pipeline with PR creation for prebuilt libs
- **Workflows**: `typescript-bindings.yml` — cross-platform test matrix (4 OS + Alpine musl)
- **Workflows**: `typescript-napi-prebuilds.yml` — build `.node` prebuilds for 5 platforms via zig cross-compilation
- **Workflows**: `typescript-npm-publish.yml` — OIDC trusted publishing to npm
- **Release scripts**: download, upload, checksum scripts now handle FFI bundles, C headers, and SBOM artifacts
- **Go workspace**: root `go.mod` + `go.work` for repo-level Go tooling compatibility
- **actionlint config**: `.github/actionlint.yaml` with self-hosted runner labels

### Changed

- **RELEASE_CHECKLIST.md**: Added Go bindings pre-tag workflow and TypeScript post-signing publish steps
- **Makefile**: Binding workflow instructions replace v0.2.0 stubs
- **cbindgen.toml**: Updated from placeholder to production configuration for ipcprims-ffi

### Fixed

- FFI `map_peer_error` match exhaustiveness under Cargo workspace feature unification (`error.rs`)
- FFI `ipc_schema_registry_free` needless-return lint under conditional compilation (`schema.rs`)
- TypeScript test file: replaced `any` types with typed interfaces (`ipcprims.test.ts`)
- TypeScript loader: replaced `&&` chain with optional chaining (`index.js`)
- goneat assess config: disabled shellcheck, added Go linting note (sysprims parity)

### Known Issues

- **Async API**: Feature flags declared but no async code exists. Planned for v0.2.0.
- **Transitive dep duplication**: `getrandom` (0.2 + 0.3) and `windows-sys` (0.60 + 0.61) via `jsonschema` dep tree. No functional impact.
- **release.yml**: Still uses minimal v0.1.0 skeleton (validate + create release). Multi-platform build matrix planned for v0.1.2.
- **Go prebuilt libs**: Not yet populated — `go-bindings.yml` workflow creates the PR. Stub bridge compiles but FFI calls return `ErrFFIUnavailable` without prebuilts.

## [0.1.0] — 2026-02-13

First functional release. Transport, framing, schema validation, peer management, and CLI.

### Added

- **ipcprims-transport**: Unix domain socket transport with bind/accept/connect, hardened default permissions (0o600), automatic socket cleanup on drop (`aff997d`, `5557234`)
- **ipcprims-frame**: Length-prefixed wire format codec (`[magic:2][length:4 LE][channel:2 LE][payload]`), sync FrameReader/FrameWriter with partial-read handling and configurable timeouts (`aff997d`)
- **ipcprims-frame**: Built-in channel constants (CONTROL=0, COMMAND=1, DATA=2, TELEMETRY=3, ERROR=4) with user-defined range 256-65535 (`aff997d`)
- **ipcprims-schema**: JSON Schema 2020-12 validation via SchemaRegistry with strict mode, directory loading, symlink rejection, and file-size limits (`46fa086`)
- **ipcprims-peer**: Handshake protocol over CONTROL channel with version negotiation, channel intersection, and optional auth token (`3e50d00`)
- **ipcprims-peer**: Peer API with send/recv/recv_on/request patterns, bounded per-channel buffering, control flood protection, ping/pong, graceful shutdown (`67c0da0`)
- **ipcprims (CLI)**: P0 commands — `listen`, `send`, `echo`, `version` with `--format json|table|pretty|raw` output and rsfulmen-aligned exit codes (`33ea05a`)
- **ipcprims (CLI)**: P1 commands — `info`, `doctor`, `envinfo`, `version --extended` with build provenance (`f1bff63`)
- **ipcprims (CLI)**: `send --wait` with `--wait-timeout` and ERROR channel negotiation (`175319f`)
- **ipcprims (CLI)**: `echo --validate` with JSON error payloads on ERROR channel (`e6124dd`)
- Examples: `echo-server` and `multi-channel` in umbrella crate (`39ed82f`)
- Dogfooding infrastructure: `scripts/dogfood/cli-matrix.sh`, guide at `docs/guides/cli-dogfooding.md`, `make dogfood-cli` target (`64d7b92`)
- Decision records: DDR-0001 (transport scope), DDR-0002 (CLI precepts), SDR-0001 through SDR-0005 (security boundaries) (`5c92612`, `7c8869d`, `9220e93`)
- Architecture overview at `docs/architecture.md` (`5c92612`)
- CI/CD: GitHub Actions workflows for CI and release, Makefile with quality gates (`f084488`)
- Agentic roles: devlead, deliverylead, secrev, qa, releng, cicd, infoarch (`5557234`, `39ed82f`)

### Fixed

- FrameReader now propagates WouldBlock as IO error instead of retrying unconditionally, restoring Peer timeout semantics on macOS (`175319f`)
- `send --wait` negotiates ERROR channel for validation error responses (`175319f`)
- `echo --validate` sends schema error payloads on ERROR channel and continues serving (`e6124dd`)
- `send --wait` receives on sent channel via `recv_on` instead of any channel (`e6124dd`)
- Auth token no longer exposed via `handshake_result()` accessor; requires explicit `take_client_auth_token()` (`044a23a`)
- Auth token redacted from Debug output of HandshakeResult (`32dab4d`)
- Schema directory loading hardened: symlink rejection, file-size cap, schema-count cap (`bd428fb`)
- Windows file-identity check added for schema loader race hardening (`691067b`)
- Peer and transport defaults hardened per SDR-0002 (`9c4b213`)
- Build target triple exported for `envinfo` via build.rs (`7d94887`)
- Envinfo version test made dynamic via CARGO_PKG_VERSION (`67a70df`)
- Doctor non-Unix platform transport check made explicit (`67a70df`)

### Known Issues

- Async feature flags (`async`) are declared across crates but no async code exists yet. Planned for v0.2.0.
- Transitive dependency duplication: `getrandom` (0.2 + 0.3) and `windows-sys` (0.60 + 0.61) via `jsonschema` dependency tree. No functional impact; tracked for supply chain awareness.
- `cbindgen.toml` is present as a placeholder; the `ffi/` crate does not exist yet. Shipped in v0.1.1.

[0.2.1]: https://github.com/3leaps/ipcprims/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/3leaps/ipcprims/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/3leaps/ipcprims/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/3leaps/ipcprims/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/3leaps/ipcprims/releases/tag/v0.1.0
