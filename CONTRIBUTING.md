# Contributing to ipcprims

Thanks for helping build **ipcprims** — permissively licensed IPC primitives + embeddable APIs.

This repo aims to be:

- **small and predictable** (library-first, framed-by-default),
- **cross-platform** (Linux/macOS/Windows),
- **license-clean** (MIT/Apache-2.0 throughout, tight dependency policy),
- **binding-friendly** (Go/Python/TypeScript).

## Quick start (contributors)

1. Install Rust (stable) and the repo toolchain:

- `rustup toolchain install stable`
- `rustup component add rustfmt clippy`

2. Run the full local quality loop:

- `cargo fmt --all`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo test --workspace --all-features`

> CI is the source of truth; see `make check`.

## How to contribute

### Issues

- **Bug reports**: include OS, architecture, Rust version, expected vs actual behavior, and minimal repro steps.
- **Feature requests**: explain the use case, expected behavior, and any wire-format impacts.

### Pull requests

PRs should be small, focused, and include tests.

**PR checklist**

- [ ] Tests added or updated (unit/integration/stress)
- [ ] `cargo fmt` clean
- [ ] `cargo clippy` clean
- [ ] Wire format changes documented if applicable
- [ ] Docs updated (if behavior changed)

## Commit Messages

Follow the [3leaps commit style](https://crucible.3leaps.dev/repository/commit-style):

```
<type>(<scope>): <subject>

<body>

Co-Authored-By: <Model> <noreply@3leaps.net>
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

## Dependency and licensing policy (high-level)

- Allowed: MIT / Apache-2.0 / BSD / ISC / 0BSD (see CI allowlist)
- Disallowed: GPL / AGPL
- Avoid: LGPL unless explicitly reviewed and documented

If you propose a new dependency:

- explain why it is needed
- prefer `default-features = false`
- consider a feature-gated "heavy" option instead of a hard dependency

## Code of Conduct and Security

This project follows organization-wide policies. See SECURITY and code-of-conduct references in the org policy repository (linked from the repo README).

If you find a security issue, **do not open a public issue**; email security@3leaps.net for private disclosure.

## License

By contributing, you agree that your contributions will be licensed under the MIT OR Apache-2.0 license.

## Maintainers

Maintainers may request:

- API adjustments to preserve stability
- Wire format versioning updates
- Additional cross-platform tests
