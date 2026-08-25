# Release Notes

> **Purge policy**: This file retains the **latest 3 releases** in reverse chronological order.
> Older entries are archived to `docs/releases/v<semver>.md` and removed from this file.
> For the complete changelog, see `CHANGELOG.md`.

---

## v0.2.5 — 2026-08-25

Patch release for TypeScript package interoperability and SchemaRegistry guidance.

### Highlights

- **CommonJS and ESM entrypoints**: `@3leaps/ipcprims` now exposes explicit conditional exports with matching declaration files. ESM consumers can use named imports while CommonJS `require()` remains supported.
- **SchemaRegistry guide**: new documentation covers configuration defaults, strict-mode behavior, safe directory loading, Rust peer integration, CLI validation, and standalone TypeScript validation.

### Compatibility

- **Node.js**: the TypeScript package continues to require Node.js 20+.
- **Runtime behavior**: no wire format, peer transport, or SchemaRegistry runtime behavior changed.

### Known Issues

- **NAPI-RS major update deferred**: `@napi-rs/cli` 3.x and Rust `napi`/`napi-derive` 3.x remain on 2.x.

Full release details: [docs/releases/v0.2.5.md](docs/releases/v0.2.5.md)

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
