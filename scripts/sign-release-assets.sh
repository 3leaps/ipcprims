#!/usr/bin/env bash
# Sign release checksum manifests with minisign (and optionally PGP)
# Usage: sign-release-assets.sh <tag> [dir]
#
# Environment variables:
#   IPCPRIMS_MINISIGN_KEY  - Path to minisign secret key (required)
#   IPCPRIMS_PGP_KEY_ID    - PGP key ID for optional GPG signing (optional)
#   IPCPRIMS_GPG_HOMEDIR   - Custom GPG home directory (optional)
#
# Requires: minisign, optionally gpg
set -euo pipefail

TAG=${1:?"usage: sign-release-assets.sh <tag> [dir]"}
DIR=${2:-dist/release}

if [ ! -d "$DIR" ]; then
	echo "Error: Directory $DIR does not exist"
	exit 1
fi

if [ -z "${IPCPRIMS_MINISIGN_KEY:-}" ]; then
	echo "Error: IPCPRIMS_MINISIGN_KEY environment variable not set"
	echo ""
	echo "Set to path of your minisign secret key:"
	echo "  export IPCPRIMS_MINISIGN_KEY=/path/to/signing.key"
	exit 1
fi

if [ ! -f "$IPCPRIMS_MINISIGN_KEY" ]; then
	echo "Error: Minisign key not found: $IPCPRIMS_MINISIGN_KEY"
	exit 1
fi

cd "$DIR"

# Guard: verify checksum manifests exist before signing
MISSING=0
for manifest in SHA256SUMS SHA512SUMS; do
	if [ ! -f "$manifest" ]; then
		echo "Error: $manifest not found in $DIR"
		MISSING=$((MISSING + 1))
	fi
done
if [ $MISSING -gt 0 ]; then
	echo ""
	echo "Did you forget to run checksums first?"
	echo "  make release-checksums"
	exit 1
fi

echo "Signing release $TAG..."

# Sign with minisign
echo ""
echo "=== Minisign Signatures ==="

for manifest in SHA256SUMS SHA512SUMS; do
	if [ -f "$manifest" ]; then
		echo "Signing $manifest with minisign..."
		minisign -S -s "$IPCPRIMS_MINISIGN_KEY" \
			-m "$manifest" \
			-t "ipcprims $TAG - $(date -u +%Y-%m-%dT%H:%M:%SZ)" \
			-x "${manifest}.minisig"
		echo "[ok] Created ${manifest}.minisig"
	fi
done

# Optional PGP signing
if [ -n "${IPCPRIMS_PGP_KEY_ID:-}" ]; then
	echo ""
	echo "=== PGP Signatures ==="

	GPG_OPTS=()
	if [ -n "${IPCPRIMS_GPG_HOMEDIR:-}" ]; then
		GPG_OPTS+=("--homedir" "$IPCPRIMS_GPG_HOMEDIR")
	fi

	for manifest in SHA256SUMS SHA512SUMS; do
		if [ -f "$manifest" ]; then
			echo "Signing $manifest with PGP..."
			gpg "${GPG_OPTS[@]}" \
				--armor \
				--detach-sign \
				--local-user "$IPCPRIMS_PGP_KEY_ID" \
				--output "${manifest}.asc" \
				"$manifest"
			echo "[ok] Created ${manifest}.asc"
		fi
	done
else
	echo ""
	echo "[--] PGP signing skipped (IPCPRIMS_PGP_KEY_ID not set)"
fi

echo ""
echo "[ok] Signing complete"
ls -la ./*.minisig ./*.asc 2>/dev/null || true
