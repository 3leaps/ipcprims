# npm Publishing Guide

This document is the single source of truth for publishing ipcprims TypeScript
bindings to npm. `RELEASE_CHECKLIST.md` references this guide for the publishing
steps — read this when the automated workflow fails or when doing a first publish.

## Package Structure

ipcprims publishes **six** packages to npm per release:

| Package | Contents |
|---------|----------|
| `@3leaps/ipcprims` | Root package — optional deps on all platform packages |
| `@3leaps/ipcprims-darwin-arm64` | macOS arm64 `.node` prebuild |
| `@3leaps/ipcprims-linux-x64-gnu` | Linux x64 glibc `.node` prebuild |
| `@3leaps/ipcprims-linux-x64-musl` | Linux x64 musl `.node` prebuild |
| `@3leaps/ipcprims-linux-arm64-gnu` | Linux arm64 glibc `.node` prebuild |
| `@3leaps/ipcprims-win32-x64-msvc` | Windows x64 `.node` prebuild |

**All six must be published together.** Publishing only the root package means
consumers install it successfully but get no native binary — the module silently
fails to load at runtime.

## Normal Flow (Automated, OIDC)

For v0.2.0+, publishing is fully automated via GitHub Actions OIDC trusted
publishing. No npm token required.

```bash
# After signing and undrafting the release:
VERSION=$(cat VERSION)

# Step 1: Build platform .node binaries and stage npm dirs
gh workflow run "TypeScript N-API Prebuilds" --ref "v${VERSION}"

# Step 2: Publish all six packages via OIDC
gh workflow run "TypeScript npm Publish" --ref "v${VERSION}"
```

OIDC trusted publishing requires:
- Workflow running from a `v*` tag ref
- `publish-npm` environment protection configured on the repo
- All six packages already exist on npm (see First Publish below)

## First Publish (Manual)

OIDC trusted publishing can only update **existing** packages. It cannot create
a brand-new package on npm. If any of the six packages does not yet exist, the
workflow will fail with:

```
npm error code E404
npm error 404 Not Found - PUT https://registry.npmjs.org/@3leaps%2fipcprims-darwin-arm64
npm error 404  The requested resource '@3leaps/ipcprims-darwin-arm64@X.Y.Z' could not be found
```

This happened at v0.1.2 (only root was published manually; platform packages were
never created) and again at v0.2.0 (platform packages still absent).

**Check which packages exist before running the workflow:**

```bash
npm view @3leaps/ipcprims version 2>/dev/null || echo "MISSING"
for pkg in darwin-arm64 linux-x64-gnu linux-x64-musl linux-arm64-gnu win32-x64-msvc; do
  result=$(npm view "@3leaps/ipcprims-${pkg}" version 2>/dev/null || echo "MISSING")
  echo "@3leaps/ipcprims-${pkg}: ${result}"
done
```

If any are missing, do the manual first publish before running the OIDC workflow.

### Manual First Publish Steps

1. **Download the staged npm packages** from the prebuilds workflow artifact:

   ```bash
   VERSION=$(cat VERSION)
   PREBUILDS_RUN_ID=$(gh run list \
     --workflow=typescript-napi-prebuilds.yml \
     --status=success --limit=10 \
     --json databaseId,headSha \
     --jq ".[] | select(.headSha == \"$(git rev-parse v${VERSION}^{})\") | .databaseId" \
     | head -1)

   echo "Prebuilds run: ${PREBUILDS_RUN_ID}"
   gh run download "${PREBUILDS_RUN_ID}" --name ts-npm-dir --dir /tmp/ipcprims-npm-packages
   ls /tmp/ipcprims-npm-packages/
   ```

2. **Log in to npm** (requires your npm account with publish access to `@3leaps`):

   ```bash
   npm login
   # Authenticate via browser — MFA required
   ```

3. **Publish all five platform packages:**

   ```bash
   for dir in /tmp/ipcprims-npm-packages/*/; do
     pkg=$(basename "$dir")
     echo "Publishing @3leaps/ipcprims-${pkg}..."
     (cd "$dir" && npm publish --access public)
   done
   ```

4. **Build and publish the root package:**

   ```bash
   cd bindings/typescript
   npm install --omit=optional
   npm run build
   npm publish --access public
   cd -
   ```

5. **Verify all six are on npm** (uses registry API directly — no auth required):

   ```bash
   VERSION=$(cat VERSION)
   for pkg in "@3leaps/ipcprims" "@3leaps/ipcprims-darwin-arm64" "@3leaps/ipcprims-linux-x64-gnu" \
              "@3leaps/ipcprims-linux-x64-musl" "@3leaps/ipcprims-linux-arm64-gnu" "@3leaps/ipcprims-win32-x64-msvc"; do
     result=$(curl -sf "https://registry.npmjs.org/${pkg}/${VERSION}" 2>/dev/null \
       | python3 -c "import json,sys; print(json.load(sys.stdin).get('version','?'))" 2>/dev/null \
       || echo "NOT FOUND")
     echo "${pkg}: ${result}"
   done
   ```

   > Do not use `npm view` for verification — it requires a valid local auth token and will
   > report 404 for packages that actually exist if the token has expired mid-session.
   > The registry API check above works without auth.

6. **Re-run the OIDC workflow** (optional, for audit trail — it will be a no-op since
   the packages are already published, or skip and move on):

   ```bash
   gh workflow run "TypeScript npm Publish" --ref "v${VERSION}"
   ```

### After First Publish

Once all six packages exist on npm, all future releases use the automated OIDC
workflow with no manual intervention. The manual steps above should never be
needed again for an existing package.

## Troubleshooting

### E404 on platform package

The package doesn't exist on npm yet. Follow the Manual First Publish steps above.
Do **not** only publish the root package — all six must be created.

### OIDC token error / `always-auth` warning / E404 on existing package

`actions/setup-node` sets `NPM_CONFIG_USERCONFIG` as a job-level environment variable
pointing at its own generated npmrc. That file contains `always-auth=true` and a
`NODE_AUTH_TOKEN` placeholder. Because `NPM_CONFIG_USERCONFIG` is set at job level,
exporting a new path in a step has no effect — the step sees the job-level value.

The only reliable fix is to **overwrite the file in place** at the start of each
publish step, before unsetting the token:

```bash
printf '%s\n' 'registry=https://registry.npmjs.org/' 'always-auth=false' > "$NPM_CONFIG_USERCONFIG"
unset NODE_AUTH_TOKEN NPM_TOKEN
```

If this is missing and `always-auth=true` remains, npm will attempt token auth and
get `E404` on a `PUT` even for packages that exist — the registry rejects the
malformed auth rather than falling through to OIDC provenance.

### Prebuilds commit mismatch

The publish workflow validates that the prebuilds artifact was built from the same
commit as the tag. If they diverge (e.g. you retagged after running prebuilds),
pass the run ID explicitly:

```bash
gh workflow run "TypeScript npm Publish" --ref "v${VERSION}" \
  -f prebuilds_run_id="<run-id>"
```

Find the run ID:
```bash
gh run list --workflow=typescript-napi-prebuilds.yml --limit 5
```

### `npm login` vs OIDC

Do **not** use `npm login` for automated publishing — it leaves a long-lived token
in `~/.npmrc` that is not scoped to the workflow and creates a security risk.
`npm login` is only appropriate for the one-time manual first publish. All subsequent
publishes use OIDC.
