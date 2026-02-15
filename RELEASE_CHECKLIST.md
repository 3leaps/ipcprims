# Release Checklist

This document walks maintainers through the build/sign/upload flow for each ipcprims release.

## Prerequisites

- GPG and minisign installed
- Signing keys configured (shared 3leaps release signing keys)
- Environment variables set (see step 2 below)
- `gh` CLI authenticated with push access

## 1. Pre-Release Preparation

### Code Quality Gates

- [ ] Ensure `main` is clean: `git status` shows no uncommitted changes
- [ ] Run pre-push checks: `make prepush` passes
- [ ] Run full test suite: `cargo test --workspace --all-features`
- [ ] Verify cargo-deny passes: `cargo deny check`

### Version & Documentation

- [ ] Update `VERSION` file with new semver (e.g., `0.1.1`)
- [ ] Sync version to Cargo.toml: `make version-sync`
- [ ] Update `CHANGELOG.md` (move Unreleased to new version section)
- [ ] Create release notes: `docs/releases/vX.Y.Z.md`

### Pre-Tag Verification

- [ ] **Run preflight checks**: `make release-preflight`
  - Validates: working tree clean, prepush checks pass, version synced, release notes exist, local/remote sync
  - **Must pass before tagging**

### Commit & Tag

- [ ] Commit changes:
  ```bash
  git add -A
  git commit -m "release: prepare vX.Y.Z"
  ```
- [ ] Push to main:
  ```bash
  git push origin main
  ```
- [ ] **Verify local/remote sync** (required before tagging):

  ```bash
  git fetch origin
  # Must show no output (no divergence):
  git log --oneline origin/main..HEAD
  git log --oneline HEAD..origin/main
  ```

- [ ] Create and push tag:
  ```bash
  VERSION=$(cat VERSION)
  git tag -a "v${VERSION}" -m "v${VERSION}: <brief description>"
  git push origin "v${VERSION}"
  ```

### Bindings (Pre-Tag) — skip for source-only releases

> **Note**: These steps apply when releasing binary/FFI artifacts (v0.1.2+).
> For source-only releases (v0.1.0, v0.1.1), skip to CI Verification.

- [ ] **Go bindings prep** (MUST happen before tagging):
  1. Run `go-bindings.yml` workflow via GitHub Actions (manual dispatch, input: version)
  2. Workflow builds FFI for all platforms and creates PR with prebuilt libs
  3. Review and merge the PR
  4. Tag the **merge commit** (critical: release tag must include Go prebuilt libs)
- [ ] Go submodule tag: `git tag -a "bindings/go/ipcprims/v${VERSION}" -m "Go bindings v${VERSION}"`
- [ ] Verify `go test ./...` passes in `bindings/go/ipcprims/`
- [ ] Verify `npm test` and `npm run typecheck` pass in `bindings/typescript/`

### Bindings (Post-Signing) — skip for source-only releases

- [ ] **TypeScript N-API prebuilds**: Run `typescript-napi-prebuilds.yml` on the tagged commit
- [ ] **TypeScript npm publish**: Run `typescript-npm-publish.yml` with OIDC trusted publishing

### CI Verification

- [ ] Wait for GitHub Actions release workflow to complete
- [ ] Verify CI status is green on the tag
- [ ] Check release has expected artifacts

## 2. Manual Signing (Local Machine)

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

After release, bump VERSION for next development cycle:

```bash
make version-patch   # 0.1.0 -> 0.1.1
# or: make version-minor  # 0.1.0 -> 0.2.0
# or: make version-major  # 0.1.0 -> 1.0.0

git add VERSION
git commit -m "chore: bump version to $(cat VERSION)-dev"
git push origin main
```

## Quick Reference: All Release Targets

| Target                           | Description                                                                    |
| -------------------------------- | ------------------------------------------------------------------------------ |
| `make release-preflight`         | **REQUIRED**: Verify pre-tag requirements (tree, checks, version, notes, sync) |
| `make release-guard-tag-version` | Verify git tag matches VERSION file (runs automatically in `make release`)     |
| `make release-check`             | Version consistency + package check                                            |
| `make release-clean`             | Remove dist/release contents                                                   |
| `make release-download`          | Download CI artifacts from GitHub                                              |
| `make release-checksums`         | Generate SHA256SUMS and SHA512SUMS                                             |
| `make release-sign`              | Sign checksums with minisign + PGP                                             |
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

### CI workflow failed

1. Check GitHub Actions logs
2. Fix the issue on main
3. Delete the tag and release draft
4. Start over from step 1

### Signature verification failed

1. Ensure you used the correct signing key
2. Re-run `make release-sign`
3. Re-run `make release-verify` to confirm

## Key Rotation

If rotating signing keys, update:

- [ ] `RELEASE_CHECKLIST.md` - verification example public key
- [ ] `README.md` - verification snippet (when added)

## Versioning Policy

- **Patch** (0.1.1): Bug fixes, security patches
- **Minor** (0.2.0): New features, backward-compatible
- **Major** (1.0.0): Breaking changes, API changes

See `docs/decisions/` for versioning decisions.
