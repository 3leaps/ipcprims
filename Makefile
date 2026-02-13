# ipcprims Makefile
# GPL-free, cross-platform IPC primitives library
#
# Quick Reference:
#   make help       - Show all available targets
#   make bootstrap  - Install tools (sfetch -> goneat)
#   make check      - Run all quality checks (fmt, lint, test, deny)
#   make fmt        - Format code (cargo fmt)
#   make build      - Build all crates

.PHONY: all help bootstrap bootstrap-force tools check test fmt fmt-check lint build clean version install dogfood-cli
.PHONY: precommit prepush deny audit
.PHONY: build-release
.PHONY: version-patch version-minor version-major version-set version-sync version-check
.PHONY: ci release-check release-preflight
.PHONY: release-guard-tag-version
.PHONY: release-clean release-download release-checksums release-sign release-export-keys
.PHONY: release-verify release-verify-checksums release-verify-signatures release-verify-keys
.PHONY: release-notes release-upload release

# -----------------------------------------------------------------------------
# Configuration
# -----------------------------------------------------------------------------

VERSION := $(shell cargo metadata --format-version 1 2>/dev/null | \
	grep -o '"version":"[^"]*"' | head -1 | cut -d'"' -f4 || echo "dev")

BIN_DIR := $(CURDIR)/bin

SFETCH_VERSION := latest
GONEAT_VERSION ?= v0.5.1

SFETCH = $(shell [ -x "$(BIN_DIR)/sfetch" ] && echo "$(BIN_DIR)/sfetch" || command -v sfetch 2>/dev/null)
GONEAT = $(shell command -v goneat 2>/dev/null)

CARGO = cargo

# -----------------------------------------------------------------------------
# Default and Help
# -----------------------------------------------------------------------------

all: check

help: ## Show available targets
	@echo "ipcprims - GPL-free IPC Primitives"
	@echo "Framed-by-default inter-process communication."
	@echo ""
	@echo "Development:"
	@echo "  help            Show this help message"
	@echo "  bootstrap       Install tools (sfetch -> goneat)"
	@echo "  build           Build all crates (debug)"
	@echo "  build-release   Build all crates (release)"
	@echo "  install         Install ipcprims binary to ~/.local/bin"
	@echo "  dogfood-cli     Run end-to-end CLI dogfooding matrix"
	@echo "  clean           Remove build artifacts"
	@echo ""
	@echo "Quality gates:"
	@echo "  check           Run all quality checks (fmt, lint, test, deny)"
	@echo "  ci              Run exactly what CI runs (fmt, clippy, test, deny, version-check)"
	@echo "  test            Run test suite"
	@echo "  fmt             Format code (cargo fmt)"
	@echo "  lint            Run linting (cargo clippy + goneat lint)"
	@echo "  precommit       Pre-commit checks (fast: fmt, clippy)"
	@echo "  prepush         Pre-push checks (thorough: fmt, clippy, test, deny)"
	@echo "  deny            Run cargo-deny license and advisory checks"
	@echo "  audit           Run cargo-audit security scan"
	@echo ""
	@echo "Release:"
	@echo "  release-preflight  Verify all pre-tag requirements (REQUIRED before tagging)"
	@echo "  release-guard-tag-version  Verify tag matches VERSION file"
	@echo "  release-check      Version consistency + package check"
	@echo "  release-clean      Remove dist/release contents"
	@echo "  release-download   Download release assets from GitHub"
	@echo "  release-checksums  Generate SHA256SUMS and SHA512SUMS"
	@echo "  release-sign       Sign checksum manifests (minisign + PGP)"
	@echo "  release-export-keys Export public signing keys"
	@echo "  release-verify     Verify checksums, signatures, and keys"
	@echo "  release-notes      Copy release notes to dist"
	@echo "  release-upload     Upload signed artifacts to GitHub"
	@echo "  release            Full signing workflow (clean -> upload)"
	@echo ""
	@echo "Version management:"
	@echo "  version         Print current version"
	@echo "  version-check   Validate version consistency across files"
	@echo "  version-patch   Bump patch version (0.1.0 -> 0.1.1)"
	@echo "  version-minor   Bump minor version (0.1.0 -> 0.2.0)"
	@echo "  version-major   Bump major version (0.1.0 -> 1.0.0)"
	@echo "  version-set     Set explicit version (V=X.Y.Z)"
	@echo "  version-sync    Sync VERSION to Cargo.toml"
	@echo ""
	@echo "Current version: $(VERSION)"

# -----------------------------------------------------------------------------
# Bootstrap
# -----------------------------------------------------------------------------

bootstrap: ## Install required tools (sfetch -> goneat)
	@echo "Bootstrapping ipcprims development environment..."
	@echo ""
	@if ! command -v curl >/dev/null 2>&1; then \
		echo "[!!] curl not found (required for bootstrap)"; \
		exit 1; \
	fi
	@echo "[ok] curl found"
	@if ! command -v cargo >/dev/null 2>&1; then \
		echo "[!!] cargo not found (required)"; \
		echo ""; \
		echo "Install Rust toolchain:"; \
		echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"; \
		exit 1; \
	fi
	@echo "[ok] cargo: $$(cargo --version)"
	@echo ""
	@mkdir -p "$(BIN_DIR)"
	@if [ ! -x "$(BIN_DIR)/sfetch" ] && ! command -v sfetch >/dev/null 2>&1; then \
		echo "[..] Installing sfetch (trust anchor)..."; \
		curl -fsSL https://github.com/3leaps/sfetch/releases/download/$(SFETCH_VERSION)/install-sfetch.sh | bash -s -- --dest "$(BIN_DIR)"; \
	else \
		echo "[ok] sfetch already installed"; \
	fi
	@SFETCH_BIN=""; \
	if [ -x "$(BIN_DIR)/sfetch" ]; then SFETCH_BIN="$(BIN_DIR)/sfetch"; \
	elif command -v sfetch >/dev/null 2>&1; then SFETCH_BIN="$$(command -v sfetch)"; fi; \
	if [ -z "$$SFETCH_BIN" ]; then echo "[!!] sfetch installation failed"; exit 1; fi; \
	echo "[ok] sfetch: $$SFETCH_BIN"
	@echo ""
	@SFETCH_BIN=""; \
	if [ -x "$(BIN_DIR)/sfetch" ]; then SFETCH_BIN="$(BIN_DIR)/sfetch"; \
	elif command -v sfetch >/dev/null 2>&1; then SFETCH_BIN="$$(command -v sfetch)"; fi; \
	if [ "$(FORCE)" = "1" ] || ! command -v goneat >/dev/null 2>&1; then \
		echo "[..] Installing goneat $(GONEAT_VERSION) via sfetch (user-space)..."; \
		$$SFETCH_BIN --repo fulmenhq/goneat --tag $(GONEAT_VERSION); \
	else \
		echo "[ok] goneat already installed"; \
	fi
	@if command -v goneat >/dev/null 2>&1; then \
		echo "[ok] goneat: $$(goneat version 2>&1 | head -n1)"; \
	else \
		echo "[!!] goneat installation failed"; exit 1; \
	fi
	@echo ""
	@echo "[..] Checking Rust dev tools..."
	@if ! command -v cargo-deny >/dev/null 2>&1; then \
		echo "[..] Installing cargo-deny..."; \
		cargo install cargo-deny --locked; \
	else \
		echo "[ok] cargo-deny installed"; \
	fi
	@if ! command -v cargo-audit >/dev/null 2>&1; then \
		echo "[..] Installing cargo-audit..."; \
		cargo install cargo-audit --locked; \
	else \
		echo "[ok] cargo-audit installed"; \
	fi
	@if ! cargo set-version -V >/dev/null 2>&1; then \
		echo "[..] Installing cargo-edit..."; \
		cargo install cargo-edit --locked; \
	else \
		echo "[ok] cargo-edit installed"; \
	fi
	@echo ""
	@echo "[ok] Bootstrap complete"

bootstrap-force: ## Force reinstall all tools
	@$(MAKE) bootstrap FORCE=1

# -----------------------------------------------------------------------------
# Quality Gates
# -----------------------------------------------------------------------------

check: fmt-check lint test deny ## Run all quality checks
	@echo "[ok] All quality checks passed"

test: ## Run test suite
	@echo "Running tests..."
	$(CARGO) test --workspace --all-features
	@echo "[ok] Tests passed"

fmt: ## Format code (cargo fmt + goneat format)
	@echo "Formatting Rust..."
	$(CARGO) fmt --all
	@if command -v goneat >/dev/null 2>&1; then \
		echo "Formatting markdown, YAML, JSON..."; \
		goneat format --quiet; \
	else \
		echo "[!!] goneat not found — skipping non-Rust formatting (run 'make bootstrap')"; \
	fi
	@echo "[ok] Formatting complete"

fmt-check: ## Check formatting without modifying
	@echo "Checking Rust formatting..."
	$(CARGO) fmt --all -- --check
	@if command -v goneat >/dev/null 2>&1; then \
		echo "Checking markdown, YAML, JSON formatting..."; \
		goneat format --check --quiet; \
	else \
		echo "[!!] goneat not found — skipping non-Rust format check (run 'make bootstrap')"; \
	fi
	@echo "[ok] Formatting check passed"

lint: ## Run linting (cargo clippy + goneat lint)
	@echo "Linting Rust..."
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings
	@if command -v goneat >/dev/null 2>&1; then \
		echo "Linting YAML, shell, workflows..."; \
		goneat assess --categories lint --fail-on medium --ci-summary --log-level warn --output /dev/null; \
	else \
		echo "[!!] goneat not found — skipping non-Rust linting (run 'make bootstrap')"; \
	fi
	@echo "[ok] Linting passed"

deny: ## Run cargo-deny license and advisory checks
	@echo "Running cargo-deny..."
	@if command -v cargo-deny >/dev/null 2>&1; then \
		cargo-deny check; \
	else \
		echo "[!!] cargo-deny not found (run 'make bootstrap')"; \
		exit 1; \
	fi
	@echo "[ok] cargo-deny passed"

audit: ## Run cargo-audit security scan
	@echo "Running cargo-audit..."
	@if command -v cargo-audit >/dev/null 2>&1; then \
		cargo-audit audit; \
	else \
		echo "[!!] cargo-audit not found (run 'make bootstrap')"; \
		exit 1; \
	fi
	@echo "[ok] cargo-audit passed"

# -----------------------------------------------------------------------------
# Build
# -----------------------------------------------------------------------------

build: ## Build all crates (debug)
	@echo "Building (debug)..."
	$(CARGO) build --workspace
	@echo "[ok] Build complete"

build-release: ## Build all crates (release)
	@echo "Building (release)..."
	$(CARGO) build --workspace --release
	@echo "[ok] Release build complete"

clean: ## Remove build artifacts
	@echo "Cleaning..."
	$(CARGO) clean
	@rm -rf bin/
	@echo "[ok] Clean complete"

# -----------------------------------------------------------------------------
# Install
# -----------------------------------------------------------------------------

INSTALL_BINDIR ?= $(HOME)/.local/bin

install: build-release ## Install ipcprims binary to INSTALL_BINDIR
	@echo "Installing ipcprims to $(INSTALL_BINDIR)..."
	@mkdir -p "$(INSTALL_BINDIR)"
	@cp target/release/ipcprims "$(INSTALL_BINDIR)/ipcprims"
	@chmod 755 "$(INSTALL_BINDIR)/ipcprims"
	@echo "[ok] Installed ipcprims to $(INSTALL_BINDIR)/ipcprims"

dogfood-cli: ## Run end-to-end CLI dogfooding matrix
	@echo "Running CLI dogfooding matrix..."
	@bash scripts/dogfood/cli-matrix.sh
	@echo "[ok] CLI dogfooding matrix passed"

# -----------------------------------------------------------------------------
# Pre-commit / Pre-push Hooks
# -----------------------------------------------------------------------------

precommit: fmt-check lint ## Run pre-commit checks (fast)
	@echo "[ok] Pre-commit checks passed"

prepush: check ## Run pre-push checks (thorough)
	@echo "[ok] Pre-push checks passed"

# -----------------------------------------------------------------------------
# Version Management
# -----------------------------------------------------------------------------

VERSION_FILE := VERSION

version: ## Print current version
	@echo "$(VERSION)"

version-patch: ## Bump patch version (0.1.0 -> 0.1.1)
	@current=$$(cat $(VERSION_FILE)); \
	major=$$(echo $$current | cut -d. -f1); \
	minor=$$(echo $$current | cut -d. -f2); \
	patch=$$(echo $$current | cut -d. -f3); \
	new_patch=$$((patch + 1)); \
	new_version="$$major.$$minor.$$new_patch"; \
	echo "$$new_version" > $(VERSION_FILE); \
	echo "Version bumped: $$current -> $$new_version"

version-minor: ## Bump minor version (0.1.0 -> 0.2.0)
	@current=$$(cat $(VERSION_FILE)); \
	major=$$(echo $$current | cut -d. -f1); \
	minor=$$(echo $$current | cut -d. -f2); \
	new_minor=$$((minor + 1)); \
	new_version="$$major.$$new_minor.0"; \
	echo "$$new_version" > $(VERSION_FILE); \
	echo "Version bumped: $$current -> $$new_version"

version-major: ## Bump major version (0.1.0 -> 1.0.0)
	@current=$$(cat $(VERSION_FILE)); \
	major=$$(echo $$current | cut -d. -f1); \
	new_major=$$((major + 1)); \
	new_version="$$new_major.0.0"; \
	echo "$$new_version" > $(VERSION_FILE); \
	echo "Version bumped: $$current -> $$new_version"

version-set: ## Set explicit version (V=X.Y.Z)
	@if [ -z "$(V)" ]; then \
		echo "Usage: make version-set V=1.2.3"; \
		exit 1; \
	fi
	@echo "$(V)" > $(VERSION_FILE)
	@echo "Version set to $(V)"

version-sync: ## Sync VERSION file to Cargo.toml
	@ver=$$(cat $(VERSION_FILE)); \
	if command -v cargo-set-version >/dev/null 2>&1; then \
		cargo set-version --workspace "$$ver"; \
		echo "[ok] Synced Cargo.toml to $$ver"; \
	else \
		echo "[!!] cargo-edit not installed (cargo install cargo-edit)"; \
		echo "Manual update required: set version = \"$$ver\" in Cargo.toml"; \
	fi

version-check: ## Validate version consistency across files
	@echo "Checking version consistency..."
	@./scripts/check-version.sh

# -----------------------------------------------------------------------------
# CI/CD
# -----------------------------------------------------------------------------

ci: fmt-check lint test deny version-check ## Run exactly what CI runs
	@echo "[ok] CI checks passed"

release-check: version-check ## Version consistency + package check
	@echo "Checking release readiness..."
	@echo ""
	@echo "Packaging all workspace crates..."
	@$(CARGO) package --workspace
	@echo "[ok] All crates package successfully"
	@echo ""
	@echo "Release checklist:"
	@echo "  ✓ Version consistency validated"
	@echo "  ✓ All crates pass package check"
	@echo ""
	@echo "Next steps:"
	@echo "  1. make release-preflight"
	@echo "  2. git tag v$$(cat $(VERSION_FILE))"
	@echo "  3. git push origin v$$(cat $(VERSION_FILE))"
	@echo "  4. Wait for CI + release workflow"
	@echo "  5. make release (sign + upload)"

# -----------------------------------------------------------------------------
# Release Signing
# -----------------------------------------------------------------------------
#
# Workflow:
# 1. Pre-tag: make release-preflight
# 2. Tag and push: git tag vX.Y.Z && git push origin vX.Y.Z
# 3. Wait for GitHub Actions release workflow to create draft release
# 4. Sign locally: make release (or individual steps below)
#
# Environment variables (source ~/devsecops/vars/3leaps-ipcprims-cicd.sh):
#   IPCPRIMS_MINISIGN_KEY  - Path to minisign secret key (required)
#   IPCPRIMS_MINISIGN_PUB  - Path to minisign public key (optional, derived from KEY)
#   IPCPRIMS_PGP_KEY_ID    - PGP key ID for GPG signing (optional)
#   IPCPRIMS_GPG_HOMEDIR   - Custom GPG home directory (optional)
#
# --- Stubs for future bindings ---
# Go bindings (v0.2.0+):
#   Run go-bindings prep workflow before tagging.
#   Merge PR, then tag the merge commit.
#
# TypeScript bindings (v0.2.0+):
#   Run N-API prebuilds workflow after tagging.
#   Run npm publish workflow after signing.

DIST_RELEASE := dist/release
IPCPRIMS_RELEASE_TAG ?= $(shell git describe --tags --abbrev=0 2>/dev/null || echo v$(VERSION))

# Signing keys (set via environment or vars file)
IPCPRIMS_MINISIGN_KEY ?=
IPCPRIMS_MINISIGN_PUB ?=
IPCPRIMS_PGP_KEY_ID ?=
IPCPRIMS_GPG_HOMEDIR ?=

release-preflight: ## Verify all pre-tag requirements (REQUIRED before tagging)
	@echo "Running release preflight checks..."
	@echo ""
	@# Check 1: Working tree must be clean
	@if [ -n "$$(git status --porcelain 2>/dev/null)" ]; then \
		echo "[!!] Working tree not clean - commit or stash changes first"; \
		git status --short; \
		exit 1; \
	fi
	@echo "[ok] Working tree is clean"
	@# Check 2: Prepush quality gates
	@$(MAKE) prepush --silent
	@echo "[ok] Prepush checks passed"
	@# Check 3: Version sync
	@version_file=$$(cat $(VERSION_FILE) 2>/dev/null); \
	if [ -z "$$version_file" ]; then \
		echo "[!!] VERSION file not found"; \
		exit 1; \
	fi; \
	cargo_version=$$(grep '^\[workspace\.package\]' Cargo.toml -A 2 | grep '^version' | head -1 | sed 's/version = "\(.*\)"/\1/' | tr -d '"'); \
	if [ "$$version_file" != "$$cargo_version" ]; then \
		echo "[!!] Version mismatch: VERSION=$$version_file, Cargo.toml=$$cargo_version"; \
		echo "    Run: make version-sync"; \
		exit 1; \
	fi
	@echo "[ok] Version synced"
	@# Check 4: Release notes exist
	@version_file=$$(cat $(VERSION_FILE) 2>/dev/null); \
	release_notes="docs/releases/v$$version_file.md"; \
	if [ ! -f "$$release_notes" ]; then \
		echo "[!!] Release notes not found at $$release_notes"; \
		exit 1; \
	fi
	@echo "[ok] Release notes exist"
	@# Check 5: Local/remote sync
	@echo "[..] Verifying local/remote sync..."; \
	git fetch origin >/dev/null 2>&1; \
	local_only=$$(git log --oneline origin/main..HEAD 2>/dev/null | wc -l | tr -d ' '); \
	remote_only=$$(git log --oneline HEAD..origin/main 2>/dev/null | wc -l | tr -d ' '); \
	if [ "$$local_only" -gt 0 ] || [ "$$remote_only" -gt 0 ]; then \
		echo "[!!] Local and remote are out of sync"; \
		if [ "$$local_only" -gt 0 ]; then \
			echo "    $$local_only local commit(s) not pushed"; \
		fi; \
		if [ "$$remote_only" -gt 0 ]; then \
			echo "    $$remote_only remote commit(s) not pulled"; \
		fi; \
		exit 1; \
	fi
	@echo "[ok] Local and remote are in sync"
	@echo ""
	@echo "[ok] All preflight checks passed - ready to tag"
	@version_file=$$(cat $(VERSION_FILE) 2>/dev/null); \
	echo "    Next: git tag \"v$$version_file\" -m \"Release $$version_file\""

release-guard-tag-version: ## Verify tag matches VERSION file
	./scripts/release-guard-tag-version.sh

release-clean: ## Remove dist/release contents
	@echo "Cleaning release directory..."
	rm -rf $(DIST_RELEASE)
	@echo "[ok] Release directory cleaned"

release-download: ## Download release assets from GitHub
	@if [ -z "$(IPCPRIMS_RELEASE_TAG)" ] || [ "$(IPCPRIMS_RELEASE_TAG)" = "v" ]; then \
		echo "Error: No release tag found. Set IPCPRIMS_RELEASE_TAG=vX.Y.Z"; \
		exit 1; \
	fi
	./scripts/download-release-assets.sh $(IPCPRIMS_RELEASE_TAG) $(DIST_RELEASE)

release-checksums: ## Generate SHA256SUMS and SHA512SUMS
	./scripts/generate-checksums.sh $(DIST_RELEASE)

release-sign: ## Sign checksum manifests (requires IPCPRIMS_MINISIGN_KEY)
	@if [ -z "$(IPCPRIMS_MINISIGN_KEY)" ]; then \
		echo "Error: IPCPRIMS_MINISIGN_KEY not set"; \
		echo ""; \
		echo "Source the vars file:"; \
		echo "  source ~/devsecops/vars/3leaps-ipcprims-cicd.sh"; \
		exit 1; \
	fi
	IPCPRIMS_MINISIGN_KEY=$(IPCPRIMS_MINISIGN_KEY) \
	IPCPRIMS_PGP_KEY_ID=$(IPCPRIMS_PGP_KEY_ID) \
	IPCPRIMS_GPG_HOMEDIR=$(IPCPRIMS_GPG_HOMEDIR) \
	./scripts/sign-release-assets.sh $(IPCPRIMS_RELEASE_TAG) $(DIST_RELEASE)

release-export-keys: ## Export public signing keys
	IPCPRIMS_MINISIGN_KEY=$(IPCPRIMS_MINISIGN_KEY) \
	IPCPRIMS_MINISIGN_PUB=$(IPCPRIMS_MINISIGN_PUB) \
	IPCPRIMS_PGP_KEY_ID=$(IPCPRIMS_PGP_KEY_ID) \
	IPCPRIMS_GPG_HOMEDIR=$(IPCPRIMS_GPG_HOMEDIR) \
	./scripts/export-release-keys.sh $(DIST_RELEASE)

release-verify-checksums: ## Verify checksums match artifacts
	@echo "Verifying checksums..."
	cd $(DIST_RELEASE) && shasum -a 256 -c SHA256SUMS
	@echo "[ok] Checksums verified"

release-verify-signatures: ## Verify minisign/PGP signatures
	./scripts/verify-signatures.sh $(DIST_RELEASE)

release-verify-keys: ## Verify exported keys are public-only
	./scripts/verify-public-keys.sh $(DIST_RELEASE)

release-verify: release-verify-checksums release-verify-signatures release-verify-keys ## Run all release verification
	@echo "[ok] All release verifications passed"

release-notes: ## Copy release notes to dist
	@src="docs/releases/$(IPCPRIMS_RELEASE_TAG).md"; \
	if [ -f "$$src" ]; then \
		cp "$$src" "$(DIST_RELEASE)/release-notes-$(IPCPRIMS_RELEASE_TAG).md"; \
		echo "[ok] Copied release notes"; \
	else \
		echo "[--] No release notes found at $$src"; \
	fi

release-upload: release-verify release-notes ## Upload signed artifacts to GitHub release
	./scripts/upload-release-assets.sh $(IPCPRIMS_RELEASE_TAG) $(DIST_RELEASE)

release: release-guard-tag-version release-clean release-download release-checksums release-sign release-export-keys release-upload ## Full signing workflow (after CI build)
	@echo "[ok] Release $(IPCPRIMS_RELEASE_TAG) complete"
