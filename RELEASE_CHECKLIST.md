# Release Checklist

This document walks maintainers through the build/sign/upload flow for each ipcprims release.

## Prerequisites

- GPG and minisign installed
- Signing keys configured (shared 3leaps release signing keys)
- Environment variables set (see step 2 below)
- `gh` CLI authenticated with push access

## 1. Pre-Release Preparation

### Version & Documentation

- [ ] Update `VERSION` file with new semver (e.g., `0.2.0`)
- [ ] Sync version to all manifests: `make version-sync`
  - Syncs `Cargo.toml` workspace, `Cargo.lock`, and all `bindings/typescript` `package.json` files
  - **Do not skip**: version drift between `VERSION` and `Cargo.toml` is a hard failure in `make prepush`
- [ ] Update `CHANGELOG.md` (move Unreleased section to new version heading)
- [ ] Create release notes: `docs/releases/vX.Y.Z.md`

### Pre-Tag Quality Gates

- [ ] **Run preflight checks**: `make release-preflight`

  This is the single authoritative gate. It runs, in order:
  1. Working tree clean check
  2. `make prepush` — fmt, clippy, tests, cargo-deny, **version consistency**
  3. `make version-check` — full consistency: `VERSION`, `Cargo.toml`, all TypeScript packages
  4. Release notes exist at `docs/releases/vX.Y.Z.md`
  5. Local/remote sync (no unpushed or unpulled commits)

  **Must pass before pushing or tagging.**

### Commit & Push to main

- [ ] Commit all changes with proper attribution:

  ```bash
  git add -A
  git commit -m "chore: bump version to vX.Y.Z"
  # (see AGENTS.md for full attribution trailer format)
  ```

  > **Note**: The commit message must say `vX.Y.Z` (the real version), not `vX.Y.Z-dev`.
  > The `-dev` suffix is only meaningful in the post-release bump commit (step 4).

- [ ] Push to main:
  ```bash
  git push origin main
  ```

### CI Verification on main (REQUIRED before tagging)

**Do not tag until CI on `main` is green.** Tagging a broken commit creates an unusable release.

- [ ] Monitor CI run:
  ```bash
  gh run list --branch main --limit 3
  gh run watch <run-id>
  ```
- [ ] Confirm all jobs pass — no failures, no skipped required jobs
- [ ] Note the annotation `Restore cache failed: go.sum not found` on Go darwin job is a known
      non-fatal warning (the Go bindings dir is a CGo module without a `go.sum` at root)

### Bindings (Pre-Tag) — skip for source-only releases

> **Note**: These steps apply when releasing binary/FFI artifacts (v0.1.2+).
> For source-only releases, skip to tagging.

- [ ] **Go bindings prep** (MUST happen before tagging):
  1. Run `go-bindings.yml` workflow via GitHub Actions (manual dispatch, input: version)
  2. Workflow builds FFI for all platforms and creates PR with prebuilt libs
  3. Review and merge the PR
  4. Tag the **merge commit** (critical: release tag must include Go prebuilt libs)
- [ ] Go submodule tag: `git tag -a "bindings/go/ipcprims/v${VERSION}" -m "Go bindings v${VERSION}"`
- [ ] Verify `go test ./...` passes in `bindings/go/ipcprims/`
- [ ] Verify `npm test` and `npm run typecheck` pass in `bindings/typescript/`

### Create and Push Tag

- [ ] Create annotated tag:
  ```bash
  VERSION=$(cat VERSION)
  git tag -a "v${VERSION}" -m "v${VERSION}: <brief description of release>"
  ```
- [ ] Push tag (triggers release workflow):
  ```bash
  git push origin "v${VERSION}"
  ```

### CI Verification on Tag

- [ ] Wait for GitHub Actions release workflow to complete on the tag
- [ ] Verify CI status is green: `gh run list --branch "v${VERSION}"`
- [ ] Check release draft has expected artifacts (binaries for all platforms)

### Bindings (Post-Signing) — skip for source-only releases

- [ ] **TypeScript N-API prebuilds**: Run `typescript-napi-prebuilds.yml` on the tagged commit
- [ ] **TypeScript npm publish**: Run `typescript-npm-publish.yml` with OIDC trusted publishing

## 2. Manual Signing (Local Machine)

> **Note**: MFA is required for signing. Signing keys are protected by hardware token.
> The maintainer must be physically present to complete this step.

### Set Environment Variables

```bash
# Source the vars file:
source ~/devsecops/vars/3leaps-ipcprims-cicd.sh

# Or set individually:
export IPCPRIMS_RELEASE_TAG=v$(cat VERSION)
export IPCPRIMS_MINISIGN_KEY=/path/to/signing.key
export IPCPRIMS_MINISIGN_PUB=/path/to/signing.pub
export IPCPRIMS_PGP_KEY_ID="keyid!"
export IPCPRIMS_GPG_HOMEDIR=/path/to/gpg/homedir  # optional
```

### Signing Steps

1. **Clean previous release artifacts**

   ```bash
   make release-clean
   ```

2. **Download artifacts from GitHub release**

   ```bash
   make release-download
   ```

3. **Generate checksum manifests**

   ```bash
   make release-checksums
   ```

   Produces: `SHA256SUMS`, `SHA512SUMS`

4. **Sign checksum manifests** (minisign + PGP)

   ```bash
   make release-sign
   ```

   Produces: `.minisig` and `.asc` signatures for both checksum files

5. **Export public keys**

   ```bash
   make release-export-keys
   ```

   Produces: `ipcprims-minisign.pub`, `ipcprims-release-signing-key.asc`

6. **Verify everything before upload**

   ```bash
   make release-verify
   ```

   Validates:
   - Checksums match artifacts
   - Signatures verify correctly
   - Exported keys are public-only (no secret key material)

7. **Copy release notes**

   ```bash
   make release-notes
   ```

   Copies `docs/releases/vX.Y.Z.md` to `dist/release/release-notes-vX.Y.Z.md`

8. **Upload signed artifacts to GitHub**
   ```bash
   make release-upload
   ```
   > **Note:** Uses `--clobber` to overwrite existing assets. Safe to rerun.

Or run the full workflow in one command:

```bash
make release
```

## 3. Post-Release Verification

- [ ] Verify release is public: `gh release view v$(cat VERSION)`
- [ ] Verify checksums match: download and verify locally
- [ ] Verify signatures with public keys

### Verification Example

```bash
VERSION=$(cat VERSION)

# Download and verify
curl -LO "https://github.com/3leaps/ipcprims/releases/download/v${VERSION}/SHA256SUMS"
curl -LO "https://github.com/3leaps/ipcprims/releases/download/v${VERSION}/SHA256SUMS.minisig"
curl -LO "https://github.com/3leaps/ipcprims/releases/download/v${VERSION}/ipcprims-minisign.pub"

# Verify checksum
shasum -a 256 -c SHA256SUMS --ignore-missing

# Verify signature (minisign)
minisign -Vm SHA256SUMS -p ipcprims-minisign.pub
```

## 4. Post-Release Version Bump

After the release is uploaded and verified, bump VERSION for the next development cycle:

```bash
make version-patch   # 0.2.0 -> 0.2.1
# or: make version-minor  # 0.2.0 -> 0.3.0
# or: make version-major  # 0.2.0 -> 1.0.0

make version-sync    # sync new version to Cargo.toml and package.json files

git add VERSION Cargo.toml Cargo.lock bindings/typescript
git commit -m "chore: bump version to v$(cat VERSION)-dev"
git push origin main
```

> **Important**: `make version-sync` must be run immediately after the version bump.
> The `-dev` suffix in the commit message is a convention marking this as a development
> snapshot — it does not affect semver. `make prepush` will catch any drift between
> `VERSION` and `Cargo.toml` before the next release.

## Quick Reference: All Release Targets

| Target                           | Description                                                                    |
| -------------------------------- | ------------------------------------------------------------------------------ |
| `make release-preflight`         | **REQUIRED**: Verify pre-tag requirements (tree, checks, version, notes, sync) |
| `make release-guard-tag-version` | Verify git tag matches VERSION file (runs automatically in `make release`)     |
| `make release-check`             | Version consistency + package check                                            |
| `make release-clean`             | Remove dist/release contents                                                   |
| `make release-download`          | Download CI artifacts from GitHub                                              |
| `make release-checksums`         | Generate SHA256SUMS and SHA512SUMS                                             |
| `make release-sign`              | Sign checksums with minisign + PGP (requires MFA/hardware token)               |
| `make release-export-keys`       | Export public signing keys                                                     |
| `make release-verify`            | Verify checksums, signatures, and keys                                         |
| `make release-notes`             | Copy release notes to dist                                                     |
| `make release-upload`            | Upload signed artifacts to GitHub                                              |
| `make release`                   | Full workflow (clean -> upload)                                                |

## Troubleshooting

### "IPCPRIMS_MINISIGN_KEY not set"

Source the vars file or set the environment variable:

```bash
source ~/devsecops/vars/3leaps-ipcprims-cicd.sh
```

### "No release notes found"

Create the release notes file:

```bash
mkdir -p docs/releases
# Write release notes to docs/releases/vX.Y.Z.md
```

### Version mismatch in prepush or preflight

```bash
make version-sync    # sync VERSION -> Cargo.toml + package.json
make version-check   # verify all are consistent
```

### CI on main failed before tagging

1. Fix the issue on main, push the fix
2. Wait for CI to go green
3. Only then proceed to tag

### CI on tag failed after tagging

1. Check GitHub Actions logs: `gh run list --branch "v${VERSION}"`
2. Fix the issue on main
3. Delete the tag and release draft:
   ```bash
   git tag -d "v${VERSION}"
   git push origin --delete "v${VERSION}"
   gh release delete "v${VERSION}" --yes
   ```
4. Start over from the tagging step

### Signature verification failed

1. Ensure you used the correct signing key
2. Re-run `make release-sign`
3. Re-run `make release-verify` to confirm

## Key Rotation

If rotating signing keys, update:

- [ ] `RELEASE_CHECKLIST.md` - verification example public key
- [ ] `README.md` - verification snippet (when added)

## Versioning Policy

- **Patch** (0.2.1): Bug fixes, security patches
- **Minor** (0.3.0): New features, backward-compatible
- **Major** (1.0.0): Breaking changes, API changes

See `docs/decisions/` for versioning decisions.
