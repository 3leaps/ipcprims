#!/usr/bin/env bash
# Validate npm trusted-publishing runtime requirements in the publish workflow.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
WORKFLOW="${PROJECT_ROOT}/.github/workflows/typescript-publish-npm.yml"

MIN_NODE="22.14.0"
MIN_NPM="11.5.1"
PUBLISH_ENVIRONMENT="publish-npm"
REPOSITORY_URL="git+https://github.com/3leaps/ipcprims.git"
PACKAGE_JSONS=(
	"bindings/typescript/package.json"
	"bindings/typescript/npm/darwin-arm64/package.json"
	"bindings/typescript/npm/linux-arm64-gnu/package.json"
	"bindings/typescript/npm/linux-x64-gnu/package.json"
	"bindings/typescript/npm/linux-x64-musl/package.json"
	"bindings/typescript/npm/win32-x64-msvc/package.json"
)

fail() {
	echo "[!!] $*" >&2
	exit 1
}

ok() {
	echo "[ok] $*"
}

normalize_version() {
	local raw="${1#v}"
	local major minor patch

	major="$(printf '%s' "$raw" | cut -d. -f1 | sed 's/[^0-9].*$//')"
	minor="$(printf '%s' "$raw" | cut -s -d. -f2 | sed 's/[^0-9].*$//')"
	patch="$(printf '%s' "$raw" | cut -s -d. -f3 | sed 's/[^0-9].*$//')"

	printf '%s.%s.%s' "${major:-0}" "${minor:-0}" "${patch:-0}"
}

version_ge() {
	local got min
	got="$(normalize_version "$1")"
	min="$(normalize_version "$2")"

	local got_major got_minor got_patch min_major min_minor min_patch
	IFS=. read -r got_major got_minor got_patch <<<"$got"
	IFS=. read -r min_major min_minor min_patch <<<"$min"

	if ((got_major > min_major)); then
		return 0
	elif ((got_major < min_major)); then
		return 1
	elif ((got_minor > min_minor)); then
		return 0
	elif ((got_minor < min_minor)); then
		return 1
	fi
	((got_patch >= min_patch))
}

if [[ ! -f "$WORKFLOW" ]]; then
	fail "workflow not found: $WORKFLOW"
fi

if ! grep -q "environment: ${PUBLISH_ENVIRONMENT}" "$WORKFLOW"; then
	fail "publish workflow must use GitHub environment ${PUBLISH_ENVIRONMENT}"
fi

node_version="$(
	sed -nE "s/^[[:space:]]*node-version:[[:space:]]*['\"]?([^'\"]+)['\"]?[[:space:]]*$/\1/p" "$WORKFLOW" |
		head -n 1
)"

if [[ -z "$node_version" ]]; then
	fail "could not find setup-node node-version in $WORKFLOW"
fi

if ! version_ge "$node_version" "$MIN_NODE"; then
	fail "TypeScript npm publish workflow uses Node $node_version; npm trusted publishing requires Node >= $MIN_NODE"
fi

npm_version="$(
	sed -nE 's/.*npm install -g npm@([0-9]+[.][0-9]+[.][0-9]+).*/\1/p' "$WORKFLOW" |
		head -n 1
)"

if [[ -z "$npm_version" ]]; then
	fail "could not find explicit npm install version in $WORKFLOW"
fi

if ! version_ge "$npm_version" "$MIN_NPM"; then
	fail "TypeScript npm publish workflow installs npm $npm_version; trusted publishing requires npm >= $MIN_NPM"
fi

if ! grep -q 'github.event.repository.default_branch' "$WORKFLOW"; then
	fail "publish workflow recovery path must compare non-tag dispatches to the repository default branch"
fi

if ! grep -q '\[ "${GITHUB_REF_NAME}" = "$DEFAULT_BRANCH" \]' "$WORKFLOW"; then
	fail "publish workflow recovery path must require GITHUB_REF_NAME to equal DEFAULT_BRANCH"
fi

for rel_path in "${PACKAGE_JSONS[@]}"; do
	package_json="${PROJECT_ROOT}/${rel_path}"
	if [[ ! -f "$package_json" ]]; then
		fail "npm package manifest not found: $rel_path"
	fi

	repository_url="$(node -e "const pkg=require(process.argv[1]); console.log(pkg.repository && pkg.repository.url || '')" "$package_json")"
	if [[ "$repository_url" != "$REPOSITORY_URL" ]]; then
		fail "$rel_path repository.url must be $REPOSITORY_URL"
	fi
done

ok "npm publish workflow prerequisites: $(basename "$WORKFLOW"), environment ${PUBLISH_ENVIRONMENT}, Node $node_version, npm $npm_version"
