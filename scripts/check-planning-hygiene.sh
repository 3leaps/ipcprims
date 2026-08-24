#!/usr/bin/env bash
#
# Planning-reference hygiene guard.
#
# Planning artifacts (briefs, task boards, IDs, memos) live in a private,
# maintainer-managed system OUTSIDE this repository tree — see AGENTS.md →
# "Planning Artifacts". A `.gitignore` entry is a convenience filter, not a
# security boundary, so this guard asserts that no *tracked* file reintroduces
# a reference to the private planning plane.
#
# This file stays self-sterile: the guarded tokens are spelled with character
# classes ([.], [/], [-]) so the script never reintroduces the very substrings
# it guards. The single permitted tracked reference is the anchored ignore
# entry (an exact allowlist below), which names the retired directory as
# defense-in-depth; every other guarded hit must fail.
set -euo pipefail

pattern='[.]plans|planning[/]|brief[-]ipcp|IPCP[-]TASK|IPCP[-][0-9]'
permitted='^\.gitignore:[0-9]+:/[.]plans/$'

# Content: allow only the single anchored ignore entry; reject every other
# guarded hit in any tracked file.
hits="$(git grep -nE "$pattern" || true)"
if [ -n "$hits" ]; then
	bad="$(printf '%s\n' "$hits" | grep -vE "$permitted" || true)"
	if [ -n "$bad" ]; then
		printf '%s\n' "$bad"
		echo "::error::A tracked file references the private planning plane. Keep" \
			"planning artifacts out of the repository tree (see AGENTS.md →" \
			"Planning Artifacts)."
		exit 1
	fi
fi
perm_count="$(printf '%s\n' "$hits" | grep -cE "$permitted" || true)"
if [ "$perm_count" -ne 1 ]; then
	echo "::error::Expected exactly one permitted ignore entry, found $perm_count."
	exit 1
fi

# Paths: no tracked path may reintroduce a planning-plane directory segment
# (a content grep alone cannot see a sterile file added under a leaky path).
tracked_paths="$(git ls-files | grep -E "$pattern" || true)"
if [ -n "$tracked_paths" ]; then
	printf '%s\n' "$tracked_paths"
	echo "::error::A tracked file path references the private planning plane."
	exit 1
fi

echo "OK: no private planning-plane references in tracked files."
