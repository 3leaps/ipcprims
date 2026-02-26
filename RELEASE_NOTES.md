# Release Notes

> **Purge policy**: This file retains the **latest 3 releases** in reverse chronological order.
> Older entries are archived to `docs/releases/v<semver>.md` and removed from this file.
> For the complete changelog, see `CHANGELOG.md`.

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

- **v0.2.1**: Windows named pipes (sync first, async follow-on), full Windows CI integration

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
- **Async API**: Feature flags declared but no async code exists. Planned for v0.2.0.

### What's Next

- **v0.1.2 pre-tag**: Run `go-bindings.yml` to populate prebuilt libs, merge PR, then tag
- **v0.2.0**: Tokio-native async API, named pipe transport for Windows, TCP transport (per DDR-0001), CLI P2 commands

Full release details: [docs/releases/v0.1.2.md](docs/releases/v0.1.2.md)

---

## v0.1.1 — 2026-02-15

Infrastructure release: cross-language binding scaffolds and CI/release pipeline maturation. No new Rust API surface — bindings wrap the existing v0.1.0 API.

### Highlights

- **FFI crate** (`ipcprims-ffi`): C-ABI exports for listener, peer, frame, and schema operations; `staticlib` + `cdylib` outputs; `cbindgen`-generated C header; smoke test in CI
- **Go bindings** (`bindings/go/ipcprims`): CGo module with Listener, Peer, SchemaRegistry; stub FFI bridge for platforms without prebuilt libs; golangci-lint in CI
- **TypeScript bindings** (`bindings/typescript`): NAPI-RS package `@3leaps/ipcprims` with 5-platform prebuild matrix; npm platform packages for optional native addon resolution
- **CI matrix expanded**: Windows cross-check (3 targets), Linux musl build+test, FFI header generation + C smoke, Go lint+test, TypeScript build+test+typecheck
- **4 new workflows**: `go-bindings.yml` (multi-platform FFI build + PR), `typescript-bindings.yml` (cross-platform test), `typescript-napi-prebuilds.yml` (prebuild .node files), `typescript-npm-publish.yml` (OIDC trusted publishing)
- **Release scripts activated**: download, upload, and checksum scripts now handle FFI bundles, C headers, and SBOM artifacts

### Known Issues

- **release.yml**: Still uses minimal v0.1.0 skeleton. Multi-platform build matrix with FFI bundle packaging planned for v0.1.2.
- **Go prebuilt libs**: Not yet populated — `go-bindings.yml` workflow creates the PR. Stub bridge compiles but FFI calls return `ErrFFIUnavailable` without prebuilts.
- **Async API**: Feature flags declared but no async code exists. Planned for v0.2.0.

### What's Next

- **v0.1.2**: Release pipeline rewrite — multi-platform FFI build matrix, SBOM generation, structured FFI bundle packaging
- **v0.2.0**: Tokio-native async API, TCP transport (per DDR-0001), CLI P2 commands

Full release details: [docs/releases/v0.1.1.md](docs/releases/v0.1.1.md)
