#!/usr/bin/env bash
# Export public signing keys to release directory
# Usage: export-release-keys.sh [dir]
#
# Environment variables:
#   IPCPRIMS_MINISIGN_PUB  - Path to minisign public key (or derives from IPCPRIMS_MINISIGN_KEY)
#   IPCPRIMS_MINISIGN_KEY  - Path to minisign secret key (used to derive public key location)
#   IPCPRIMS_PGP_KEY_ID    - PGP key ID for optional export (optional)
#   IPCPRIMS_GPG_HOMEDIR   - Custom GPG home directory (optional)
set -euo pipefail

DIR=${1:-dist/release}

if [ ! -d "$DIR" ]; then
	echo "Error: Directory $DIR does not exist"
	exit 1
fi

echo "Exporting public keys to $DIR..."

# Export minisign public key
echo ""
echo "=== Minisign Public Key ==="

MINISIGN_PUB="${IPCPRIMS_MINISIGN_PUB:-}"

# Try to derive from secret key path if not explicitly set
if [ -z "$MINISIGN_PUB" ] && [ -n "${IPCPRIMS_MINISIGN_KEY:-}" ]; then
	# Replace .key with .pub
	MINISIGN_PUB="${IPCPRIMS_MINISIGN_KEY%.key}.pub"
fi

if [ -n "$MINISIGN_PUB" ] && [ -f "$MINISIGN_PUB" ]; then
	cp "$MINISIGN_PUB" "$DIR/ipcprims-minisign.pub"
	echo "[ok] Exported $DIR/ipcprims-minisign.pub"
	cat "$DIR/ipcprims-minisign.pub"
else
	echo "[!!] Minisign public key not found"
	echo "Set IPCPRIMS_MINISIGN_PUB or ensure .pub file exists alongside .key"
fi

# Export PGP public key
if [ -n "${IPCPRIMS_PGP_KEY_ID:-}" ]; then
	echo ""
	echo "=== PGP Public Key ==="

	GPG_OPTS=()
	if [ -n "${IPCPRIMS_GPG_HOMEDIR:-}" ]; then
		GPG_OPTS+=("--homedir" "$IPCPRIMS_GPG_HOMEDIR")
	fi

	gpg "${GPG_OPTS[@]}" \
		--armor \
		--export "$IPCPRIMS_PGP_KEY_ID" \
		>"$DIR/ipcprims-release-signing-key.asc"

	echo "[ok] Exported $DIR/ipcprims-release-signing-key.asc"
else
	echo ""
	echo "[--] PGP key export skipped (IPCPRIMS_PGP_KEY_ID not set)"
fi

echo ""
echo "[ok] Key export complete"
