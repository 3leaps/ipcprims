# Release Notes

> **Purge policy**: This file retains the **latest 3 releases** in reverse chronological order.
> Older entries are archived to `docs/releases/v<semver>.md` and removed from this file.
> For the complete changelog, see `CHANGELOG.md`.

---

## v0.2.4 — 2026-08-21

Patch release for a compatible Rust lockfile refresh, including `h2` 0.4.18 on the schema HTTP resolver path.

### Highlights

- **Compatible lockfile refresh**: workspace `Cargo.lock` updated within existing constraints, including `h2` 0.4.18.
- **Schema HTTP path only**: `h2` is reached via `jsonschema` → `reqwest`. UDS and named-pipe peer transport are unchanged.
- **Go Bindings Prep required**: rebuild Go prebuilts for this cut even if FFI source did not change; the workflow builds `--locked`.

### Compatibility

- **Rust / Node**: MSRV remains 1.88.0. TypeScript runtime floor remains Node.js 20+.
- **Downstream lockfiles**: this does not force consumers with their own `Cargo.lock` to take the new `h2`. It fixes repo-built artifacts and CI.
- **Held**: `jsonschema` 0.46.10, rustls 0.23.x, napi 2.16.x.

### Known Issues

- **NAPI-RS major update deferred**: `@napi-rs/cli` 3.x and Rust `napi`/`napi-derive` 3.x remain on 2.x.
- **Transitive dep duplication**: `getrandom` (0.2 + 0.3) and `hashbrown` (0.16 + 0.17) remain via the `jsonschema`/HTTP/TLS graph.

Full release details: [docs/releases/v0.2.4.md](docs/releases/v0.2.4.md)

---

## v0.2.3 — 2026-07-07

Fast-follow release for dependency hygiene, MSRV alignment, and TypeScript runtime floor cleanup after v0.2.2.

### Highlights

- **Rust MSRV 1.88.0**: Workspace metadata, CI MSRV gates, `make msrv`, and docs now use Rust 1.88.0 consistently.
- **Schema validation refresh**: `jsonschema` moved to 0.46.10 with explicit `resolve-http`, `resolve-file`, and `tls-ring` features to retain the ring-backed TLS posture.
- **Node runtime floor**: TypeScript bindings now declare Node.js `>=20`.
- **TypeScript 6**: TypeScript dev tooling moved to 6.0.3 with TS 6-compatible module resolution/test emit config.
- **Dependency hygiene**: Rust lockfile refreshed within compatible constraints; CLI `envinfo` now reports `jsonschema` 0.46.

### Compatibility

- **Rust**: MSRV is now 1.88.0.
- **Node.js**: TypeScript package runtime floor is now Node.js 20+.
- **Schema validation**: `jsonschema` 0.46 may tighten behavior for schemas or instances that older validation accepted.

### Known Issues

- **NAPI-RS major update deferred**: `@napi-rs/cli` 3.x and Rust `napi`/`napi-derive` 3.x need source/workflow migration and remain on 2.x for this release.
- **Transitive dep duplication**: `getrandom` (0.2 + 0.3) and `hashbrown` (0.16 + 0.17) remain via the current `jsonschema`/HTTP/TLS graph. No functional impact.

Full release details: [docs/releases/v0.2.3.md](docs/releases/v0.2.3.md)

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
