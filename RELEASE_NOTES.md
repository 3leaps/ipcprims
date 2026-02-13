# Release Notes

> **Purge policy**: This file retains the **latest 3 releases** in reverse chronological order.
> Older entries are archived to `docs/releases/v<semver>.md` and removed from this file.
> For the complete changelog, see `CHANGELOG.md`.

---

## v0.1.0 — 2026-02-13

First functional release of ipcprims — permissively licensed, cross-platform IPC primitives with framed messaging, channel multiplexing, schema validation, peer management, and a diagnostic CLI.

### Highlights

- **Transport**: Unix domain socket transport with bind/accept/connect, hardened permissions (0o600), automatic socket cleanup on drop
- **Frame codec**: Length-prefixed wire format (`[magic:2 "IP"][length:4 LE][channel:2 LE][payload]`); 16 MiB default max payload; wire format frozen for 0.x series
- **Channel system**: Built-in channels (CONTROL=0, COMMAND=1, DATA=2, TELEMETRY=3, ERROR=4) with user-defined range 256-65535
- **Framed reader/writer**: Sync FrameReader/FrameWriter with partial-read handling, WouldBlock propagation, configurable timeouts
- **Schema validation**: JSON Schema 2020-12 via SchemaRegistry; strict mode (`deny_unknown_fields`), directory loading with symlink rejection and file-size limits
- **Peer management**: Handshake protocol over CONTROL channel (version negotiation, channel intersection, optional auth token); Peer API with send/recv/recv_on/request, bounded per-channel buffering, control flood protection, graceful shutdown
- **CLI (8 commands)**: `listen`, `send`, `echo`, `info`, `doctor`, `envinfo`, `version --extended`; `--format json|table|pretty|raw`; rsfulmen-aligned exit codes; tracing to stderr
- **Security**: 5 accepted SDRs covering schema boundaries, peer hardening, auth token handling, ordering/replay
- **Dogfooding**: End-to-end CLI behavior matrix; P0-P3 findings all remediated
- **Quality**: 118 tests passing, zero clippy warnings, all deps permissively licensed (cargo deny)

### Known Issues

- **Async API**: Feature flags (`async`) are declared but no async code exists. Planned for v0.2.0.
- **Transitive dep duplication**: `getrandom` (0.2 + 0.3) and `windows-sys` (0.60 + 0.61) via `jsonschema` dep tree. No functional impact; tracked for supply chain awareness.
- **FFI placeholder**: `cbindgen.toml` present but `ffi/` crate does not exist yet. Planned for v0.2.0.

### What's Next (v0.2.0)

- Tokio-native async API behind `async` feature flag
- TCP transport behind feature flag (per DDR-0001)
- CLI P2: `connect --interactive`, `monitor`, `bench`
- FFI crate + Go bindings

Full release details: [docs/releases/v0.1.0.md](docs/releases/v0.1.0.md)
