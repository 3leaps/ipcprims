# Changelog

All notable changes to this project are documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Purge policy**: This file retains the **latest 10 releases** in reverse chronological order.
> Older entries are archived to `docs/releases/v<semver>.md` and removed from this file.
> For individual release summaries, see `RELEASE_NOTES.md`.

---

## [Unreleased]

_No unreleased changes._

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
- `cbindgen.toml` is present as a placeholder; the `ffi/` crate does not exist yet. Planned for v0.2.0.

[Unreleased]: https://github.com/3leaps/ipcprims/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/3leaps/ipcprims/releases/tag/v0.1.0
