#!/usr/bin/env bash
# Upload signed release assets to GitHub
# Usage: upload-release-assets.sh <tag> [dir]
#
# Uploads checksum files, signatures, public keys, and release notes
# Requires: gh CLI authenticated with write permissions
set -euo pipefail

TAG=${1:?"usage: upload-release-assets.sh <tag> [dir]"}
DIR=${2:-dist/release}

if [ ! -d "$DIR" ]; then
	echo "Error: Directory $DIR does not exist"
	exit 1
fi

cd "$DIR"

echo "Uploading signed assets for $TAG..."

# Verify we have the required files
REQUIRED_FILES=(
	"SHA256SUMS"
	"SHA256SUMS.minisig"
	"SHA512SUMS"
	"SHA512SUMS.minisig"
	"ipcprims-minisign.pub"
)

for file in "${REQUIRED_FILES[@]}"; do
	if [ ! -f "$file" ]; then
		echo "Error: Required file missing: $file"
		echo "Run the signing workflow first:"
		echo "  make release-checksums"
		echo "  make release-sign"
		echo "  make release-export-keys"
		exit 1
	fi
done

# Build upload list
UPLOAD_FILES=(
	"SHA256SUMS"
	"SHA256SUMS.minisig"
	"SHA512SUMS"
	"SHA512SUMS.minisig"
	"ipcprims-minisign.pub"
)

# Add optional PGP files if present
for optional in "SHA256SUMS.asc" "SHA512SUMS.asc" "ipcprims-release-signing-key.asc"; do
	if [ -f "$optional" ]; then
		UPLOAD_FILES+=("$optional")
	fi
done

# Add release notes if present
RELEASE_NOTES="release-notes-${TAG}.md"
if [ -f "$RELEASE_NOTES" ]; then
	UPLOAD_FILES+=("$RELEASE_NOTES")
fi

# Add source archives if present
for archive in "ipcprims-"*"-source.tar.gz" "ipcprims-"*"-source.zip"; do
	if [ -f "$archive" ]; then
		UPLOAD_FILES+=("$archive")
	fi
done

# Add licenses if present
for lic in LICENSE-MIT LICENSE-APACHE; do
	if [ -f "$lic" ]; then
		UPLOAD_FILES+=("$lic")
	fi
done

# --- Stubs for future asset types ---
# CLI binaries (v0.2.0+):
#   for bin in ipcprims-*.tar.gz; do UPLOAD_FILES+=("$bin"); done
#
# FFI libraries (v0.2.0+):
#   for ffi in ipcprims-ffi-*.tar.gz; do UPLOAD_FILES+=("$ffi"); done

echo "Uploading files:"
printf '  %s\n' "${UPLOAD_FILES[@]}"
echo ""

# Upload to release
gh release upload "$TAG" "${UPLOAD_FILES[@]}" --clobber

# Update release notes if we have them
if [ -f "$RELEASE_NOTES" ]; then
	echo ""
	echo "Updating release notes..."
	gh release edit "$TAG" --notes-file "$RELEASE_NOTES"
fi

echo ""
echo "[ok] Release $TAG assets uploaded"
echo "View at: https://github.com/3leaps/ipcprims/releases/tag/$TAG"
