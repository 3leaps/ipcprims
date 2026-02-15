#!/usr/bin/env bash
# Download release assets from GitHub
# Usage: download-release-assets.sh <tag> [dest_dir]
#
# Requires: gh CLI authenticated
set -euo pipefail

TAG=${1:?"usage: download-release-assets.sh <tag> [dest_dir]"}
DEST=${2:-dist/release}

echo "Downloading release assets for $TAG to $DEST..."

mkdir -p "$DEST"

# Download release assets: licenses, FFI bundle, header, SBOM
gh release download "$TAG" --dir "$DEST" --clobber \
	--pattern 'LICENSE-*' \
	--pattern 'ipcprims-ffi-*.tar.gz' \
	--pattern 'ipcprims.h' \
	--pattern 'sbom-*.json' \
	2>/dev/null || true

# Download source archives (GitHub auto-generates these but doesn't list as assets)
# We fetch them explicitly so they're included in checksum manifests
echo "Downloading source archives..."
gh api "repos/3leaps/ipcprims/tarball/$TAG" >"$DEST/ipcprims-${TAG#v}-source.tar.gz"
gh api "repos/3leaps/ipcprims/zipball/$TAG" >"$DEST/ipcprims-${TAG#v}-source.zip"

# Copy licenses from repo if not present in release assets
for lic in LICENSE-MIT LICENSE-APACHE; do
	if [ ! -f "$DEST/$lic" ] && [ -f "$lic" ]; then
		cp "$lic" "$DEST/$lic"
	fi
done

# FFI bundle, header, and SBOM are included in the main download above.
# TypeScript N-API prebuilds are published via npm, not GitHub assets.

echo "Downloaded to $DEST:"
ls -la "$DEST"
