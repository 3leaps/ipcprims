# Development Setup

This guide covers getting ipcprims building and tested locally. Pick your platform below.

The core toolchain requirements are:

- **Rust** stable (1.85+ for core crates; 1.88+ for `ipcprims-napi`)
- **Go** 1.21+ (for Go bindings)
- **Node.js** 18+ (for TypeScript bindings)
- **goneat** (formatting and lint orchestration)
- **sfetch** (secure downloader, used by `make bootstrap`)

Once prerequisites are in place, bootstrap installs the remaining Rust dev tools:

```sh
make bootstrap
```

After bootstrap, verify your environment is complete:

```sh
make doctor-env
```

This reports your shell, OS, platform, toolchain versions, and tool availability.
If any tools show `[!!] MISSING`, check the platform-specific sections below or
run `make bootstrap` again.

---

## Node.js version manager

We recommend [fnm](https://github.com/Schniz/fnm) as the Node.js version manager
across all platforms. fnm is fast, cross-platform, and correctly handles ARM64
architectures — which is critical on Windows ARM64 where
[nvm-windows silently installs x64 binaries](windows-arm64-rough-edges.md#4-nvm-windows-does-not-support-arm64--use-fnm-instead).

[Volta](https://volta.sh) is also a good cross-platform option, particularly for
teams that want pinned toolchain versions per project. Either fnm or Volta works;
we document fnm below because it is what we test against.

> **nvm (Unix)** works fine on Linux/macOS x64 and arm64. We still recommend fnm
> for consistency across platforms, but nvm is not a problem on Unix.
>
> **nvm-windows** should be avoided on ARM64 machines. See
> [rough-edges item 4](windows-arm64-rough-edges.md#4-nvm-windows-does-not-support-arm64--use-fnm-instead).

**Important:** fnm requires `fnm env` to be evaluated in each shell session.
Add the appropriate line to your shell profile — see
[rough-edges item 5](windows-arm64-rough-edges.md#5-fnm-requires-fnm-env-evaluation-in-every-shell)
or the [fnm shell setup guide](https://github.com/Schniz/fnm#shell-setup).

---

## Linux / macOS

### Rust

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### Go

```sh
# macOS (Homebrew)
brew install go

# Linux — download from https://go.dev/dl/ and extract to /usr/local
curl -fsSL https://go.dev/dl/go1.26.1.linux-amd64.tar.gz | sudo tar -C /usr/local -xz
echo 'export PATH="$PATH:/usr/local/go/bin"' >> ~/.profile
```

For Linux ARM64, replace `linux-amd64` with `linux-arm64` in the URL above.

### Node.js

```sh
# Via fnm (recommended)
curl -fsSL https://fnm.vercel.app/install | bash
fnm install --lts

# macOS alternative (Homebrew)
brew install node
```

### sfetch and goneat

```sh
# sfetch (trust anchor — installs to ~/.local/bin by default)
curl -fsSL https://github.com/3leaps/sfetch/releases/latest/download/install-sfetch.sh | bash

# goneat (via sfetch)
sfetch --repo fulmenhq/goneat --tag v0.5.1
```

### Bootstrap

```sh
git clone https://github.com/3leaps/ipcprims && cd ipcprims
make bootstrap
make doctor-env   # verify everything is in place
make check
```

---

## Windows ARM64

Windows ARM64 setup requires a few extra steps to work around the lack of a native Rust
GNU toolchain shipping all Windows import libraries out of the box.

See the dedicated guide: **[Windows ARM64 Dev Setup](windows-dev-setup.md)**

It covers:

- Scoop + rustup with the `aarch64-pc-windows-gnullvm` toolchain
- MSYS2 clangarm64 for import libs and `clang.exe`
- VC++ Redistributable (required for MSVC-linked npm tools like biome)
- `~/.cargo/config.toml` and shell profile wiring (`.bashrc` + PowerShell)
- fnm with `FNM_ARCH=arm64` for native ARM64 Node.js
- Go installation (native ARM64 binary)
- N-API TypeScript bindings (`libnode.dll.a` generation via llvm-dlltool)
- MSVC alternative

Known rough edges (ecosystem immaturity, not design issues) are documented separately in
[windows-arm64-rough-edges.md](windows-arm64-rough-edges.md).

---

## After setup — daily workflow

```sh
make doctor-env   # check environment (useful after shell/tool updates)
make fmt          # format all code
make check        # fmt-check + lint + test + deny (full gate)
make build        # debug build
make test         # tests only
```

Pre-commit and pre-push hooks are configured via goneat. Run `make precommit` before
committing and `make prepush` before pushing.

---

## Bindings

| Binding            | Target                 | Extra requirement         |
| ------------------ | ---------------------- | ------------------------- |
| Go                 | `bindings/go/ipcprims` | Go 1.21+                  |
| TypeScript (N-API) | `bindings/typescript`  | Node 18+, `make ts-build` |

Sync the FFI artifacts before building bindings:

```sh
make go-build     # builds FFI, syncs libs, builds Go module
make ts-build     # builds N-API native addon + TypeScript
```
