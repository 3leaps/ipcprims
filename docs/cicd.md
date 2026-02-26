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
- Windows is validated via a dedicated `windows-cross-check` job (compile-only).

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

- `go-bindings.yml`: builds/updates Go binding prebuilt libs.
- `typescript-bindings.yml`: runs the Node/NAPI binding test matrix.
- `typescript-napi-prebuilds.yml`: builds `.node` prebuilds.
- `typescript-npm-publish.yml`: publishes to npm (OIDC).

## Local Equivalents

Primary local gates:

- `make prepush`: format + clippy + tests + cargo-deny
- `make msrv`: core crates build+test on Rust 1.85.0 (excludes NAPI)
- `make check-windows`: compile-only Windows target checks (no linking)
