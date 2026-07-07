#!/usr/bin/env bash
# Validate npm trusted-publishing runtime requirements in the publish workflow.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
WORKFLOW="${PROJECT_ROOT}/.github/workflows/typescript-npm-publish.yml"

MIN_NODE="22.14.0"
MIN_NPM="11.5.1"

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

ok "npm publish workflow prerequisites: Node $node_version, npm $npm_version"
