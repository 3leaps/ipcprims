# CI/CD

This document describes how ipcprims uses GitHub Actions for validation and releases.
It is intentionally high-level; see `RELEASE_CHECKLIST.md` for the manual signing/upload steps.

## Workflows

### `ci.yml` (push/PR validation)

File: `.github/workflows/ci.yml`

Triggers:

- `push` to `main`
- `pull_request` targeting `main`
- `workflow_call` (used by `release.yml`)

Purpose:

- Fast, repeatable signal that the repo is healthy (format, clippy, tests, deny, etc).
- Includes a **Linux-only MSRV** job for core crates.

Notes:

- CI sets `RUSTFLAGS="-Dwarnings"` to keep portability regressions visible.
- Windows is currently validated in CI via `windows-cross-check` (compile-only); full Windows
  workspace test jobs are being expanded in v0.2.1 follow-on CI updates.

### `msrv-matrix.yml` (tag-triggered MSRV confirmation)

File: `.github/workflows/msrv-matrix.yml`

Triggers:

- `push` tags matching `v*`
- `workflow_dispatch`

Purpose:

- MSRV confirmation across OS runners (Linux/macOS/Windows) at the time we cut a release tag.
- This is deliberately scoped to **MSRV only** (build+test), not a full CI replacement.

MSRV scope details:

- Core crates target `rust-version = 1.85.0` (workspace `Cargo.toml`).
- `ipcprims-napi` requires Rust 1.88.0 (napi-build requirement) and is excluded.
- The `ipcprims` crate is built/tested without the `cli` feature for MSRV checks.
  Reason: the current CLI table dependency (`comfy-table`) requires a newer compiler than 1.85.0.

### `release.yml` (tag-triggered release pipeline)

File: `.github/workflows/release.yml`

Triggers:

- `push` tags matching `v*`

Purpose:

- Build/release artifacts and draft the GitHub release.
- Calls `ci.yml` via `workflow_call` as part of the release pipeline.

### Bindings workflows

- `go-bindings.yml`: builds/updates Go binding prebuilt libs. Run manually before tagging.
- `typescript-bindings.yml`: runs the Node/NAPI binding test matrix.
- `typescript-napi-prebuilds.yml`: builds `.node` prebuilds. Run from tag ref after signing.
- `typescript-npm-publish.yml`: publishes to npm via OIDC trusted publishing. Run from tag
  ref after prebuilds complete. Requires all six packages to already exist on npm — see
  `docs/guides/npm-publishing.md` for first-publish instructions and troubleshooting.

## Local Equivalents

Primary local gates:

- `make prepush`: format + clippy + tests + cargo-deny
- `make msrv`: core crates build+test on Rust 1.85.0 (excludes NAPI)
- `make check-windows`: compile-only Windows target checks (no linking)

## Windows Notes

### Current transport status

- Sync Windows named-pipe transport is now implemented in the Rust transport/peer layers and
  validated locally on Windows.
- Async Windows named-pipe transport and CI matrix expansion remain in-progress for v0.2.1.

### Toolchain prerequisites

Windows MSVC builds require:

- Rust toolchain (`rustup` + `cargo`)
- Visual Studio Build Tools (or full Visual Studio) with:
  - **MSVC C++ build tools**
  - **Windows SDK**

### `make bootstrap` on Windows (MSVC linker)

Some Windows shells (notably Git Bash / MSYS2 / Git-for-Windows) can place a non-MSVC `link.exe`
earlier on `PATH`. Rust MSVC builds must use **MSVC** `link.exe`.

To make bootstrap deterministic, [`Makefile`](../Makefile:114) uses `vswhere.exe` to locate
`vcvars64.bat` (see [`vcvars64-path.ps1`](../scripts/windows/vcvars64-path.ps1:1)) and runs
`cargo install ...` steps inside `cmd.exe` with the MSVC environment loaded.
