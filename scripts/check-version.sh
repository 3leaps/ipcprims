#!/usr/bin/env bash
# Version consistency check for ipcprims
# Validates that VERSION file matches Cargo.toml workspace version

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

VERSION_FILE="$PROJECT_ROOT/VERSION"
CARGO_TOML="$PROJECT_ROOT/Cargo.toml"
RELEASE_NOTES="$PROJECT_ROOT/RELEASE_NOTES.md"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

error() {
	echo -e "${RED}[ERROR]${NC} $*" >&2
}

warn() {
	echo -e "${YELLOW}[WARN]${NC} $*" >&2
}

ok() {
	echo -e "${GREEN}[OK]${NC} $*"
}

info() {
	echo "[INFO] $*"
}

# Check VERSION file exists and is readable
if [[ ! -f "$VERSION_FILE" ]]; then
	error "VERSION file not found: $VERSION_FILE"
	exit 1
fi

VERSION_FROM_FILE=$(cat "$VERSION_FILE" | tr -d '[:space:]')

if [[ -z "$VERSION_FROM_FILE" ]]; then
	error "VERSION file is empty"
	exit 1
fi

# Validate version format (semver)
if ! echo "$VERSION_FROM_FILE" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.-]+)?(\+[a-zA-Z0-9.-]+)?$'; then
	error "VERSION file contains invalid semver: $VERSION_FROM_FILE"
	exit 1
fi

ok "VERSION file: $VERSION_FROM_FILE"

# Extract version from Cargo.toml workspace.package.version
if [[ ! -f "$CARGO_TOML" ]]; then
	error "Cargo.toml not found: $CARGO_TOML"
	exit 1
fi

# Parse workspace.package.version from Cargo.toml
VERSION_FROM_CARGO=$(grep -A 20 '^\[workspace\.package\]' "$CARGO_TOML" | grep '^version' | head -1 | sed 's/.*"\(.*\)".*/\1/')

if [[ -z "$VERSION_FROM_CARGO" ]]; then
	error "Could not extract version from Cargo.toml [workspace.package]"
	exit 1
fi

ok "Cargo.toml workspace version: $VERSION_FROM_CARGO"

# Compare versions
if [[ "$VERSION_FROM_FILE" != "$VERSION_FROM_CARGO" ]]; then
	error "Version mismatch!"
	error "  VERSION file:    $VERSION_FROM_FILE"
	error "  Cargo.toml:      $VERSION_FROM_CARGO"
	error ""
	error "Run 'make version-sync' to sync Cargo.toml to VERSION file"
	exit 1
fi

ok "VERSION matches Cargo.toml workspace version"

# Check that RELEASE_NOTES.md has an entry for this version
if [[ ! -f "$RELEASE_NOTES" ]]; then
	warn "RELEASE_NOTES.md not found: $RELEASE_NOTES"
else
	if grep -q "^## v$VERSION_FROM_FILE" "$RELEASE_NOTES"; then
		ok "RELEASE_NOTES.md has entry for v$VERSION_FROM_FILE"
	else
		warn "RELEASE_NOTES.md does not have an entry for v$VERSION_FROM_FILE"
		warn "Consider adding a release notes section before publishing"
	fi
fi

# Validate that all crate Cargo.toml files use version.workspace = true
info "Checking crate Cargo.toml files..."

CRATE_DIRS=("$PROJECT_ROOT"/crates/*)
FAILED_CRATES=()

for crate_dir in "${CRATE_DIRS[@]}"; do
	if [[ ! -d "$crate_dir" ]]; then
		continue
	fi

	crate_name=$(basename "$crate_dir")
	crate_toml="$crate_dir/Cargo.toml"

	if [[ ! -f "$crate_toml" ]]; then
		warn "Cargo.toml not found for crate: $crate_name"
		continue
	fi

	# Check for version.workspace = true in [package] section
	if grep -A 10 '^\[package\]' "$crate_toml" | grep -q '^version\.workspace\s*=\s*true'; then
		ok "  $crate_name: using workspace version"
	else
		# Check if it has a hardcoded version
		if grep -A 10 '^\[package\]' "$crate_toml" | grep -q '^version\s*='; then
			error "  $crate_name: uses hardcoded version instead of workspace"
			FAILED_CRATES+=("$crate_name")
		else
			warn "  $crate_name: no version field found"
		fi
	fi
done

if [[ ${#FAILED_CRATES[@]} -gt 0 ]]; then
	error ""
	error "The following crates have hardcoded versions instead of version.workspace = true:"
	for crate in "${FAILED_CRATES[@]}"; do
		error "  - $crate"
	done
	error ""
	error "Update their Cargo.toml to use: version.workspace = true"
	exit 1
fi

ok "All crates use workspace version"

# Check TypeScript package.json versions
info "Checking TypeScript package.json files..."

TS_ROOT="$PROJECT_ROOT/bindings/typescript"
TS_FAILED=()

if [[ -f "$TS_ROOT/package.json" ]]; then
	TS_VERSION=$(grep '"version"' "$TS_ROOT/package.json" | head -1 | sed 's/.*"\([0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\)".*/\1/')
	if [[ "$TS_VERSION" == "$VERSION_FROM_FILE" ]]; then
		ok "  typescript/package.json: $TS_VERSION"
	else
		error "  typescript/package.json: $TS_VERSION (expected $VERSION_FROM_FILE)"
		TS_FAILED+=("bindings/typescript/package.json")
	fi

	# Check optionalDependencies versions
	while IFS= read -r line; do
		dep_ver=$(echo "$line" | sed 's/.*": "\([0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\)".*/\1/')
		dep_name=$(echo "$line" | sed 's/.*"\(@3leaps\/[^"]*\)".*/\1/')
		if [[ "$dep_ver" != "$VERSION_FROM_FILE" ]]; then
			error "  optionalDependency $dep_name: $dep_ver (expected $VERSION_FROM_FILE)"
			TS_FAILED+=("$dep_name")
		fi
	done < <(grep '@3leaps/ipcprims-' "$TS_ROOT/package.json")

	# Check npm platform packages
	for pkg in "$TS_ROOT"/npm/*/package.json; do
		if [[ -f "$pkg" ]]; then
			pkg_name=$(basename "$(dirname "$pkg")")
			pkg_ver=$(grep '"version"' "$pkg" | head -1 | sed 's/.*"\([0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\)".*/\1/')
			if [[ "$pkg_ver" == "$VERSION_FROM_FILE" ]]; then
				ok "  npm/$pkg_name/package.json: $pkg_ver"
			else
				error "  npm/$pkg_name/package.json: $pkg_ver (expected $VERSION_FROM_FILE)"
				TS_FAILED+=("npm/$pkg_name")
			fi
		fi
	done
fi

if [[ ${#TS_FAILED[@]} -gt 0 ]]; then
	error ""
	error "TypeScript version mismatches found. Run 'make version-sync' to fix."
	exit 1
fi

ok "All TypeScript packages match version"
echo ""
ok "✓ Version consistency check passed"
