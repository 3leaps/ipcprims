# Windows ARM64 Dev Setup (GNU Toolchain)

This guide covers setting up the ipcprims development environment on **Windows ARM64**
using the GNU/LLVM toolchain. No Visual Studio or MSVC is required.

> **Status:** Validated on Windows 11 ARM (Parallels VM, aarch64).
> The MSVC toolchain also works and is simpler if you already have VS Build Tools installed.
> See [MSVC alternative](#msvc-alternative) at the bottom.

---

## Prerequisites

Install [Scoop](https://scoop.sh) if not already present:

```powershell
Set-ExecutionPolicy RemoteSigned -Scope CurrentUser
irm get.scoop.sh | iex
```

---

## Step 1 — Core tools

```sh
scoop install git gh make 7zip
```

Configure GitHub auth (PAT or device flow):

```sh
gh auth login
```

---

## Step 2 — Rust via rustup (GNU toolchain)

Install rustup via scoop. Scoop sets `RUSTUP_HOME` and `CARGO_HOME` to its persist
directory and adds `cargo/bin` to your PATH automatically — **these take effect in new
shell sessions**. In your current session, export them manually:

```sh
scoop install rustup
```

Then in any shell (or add to `~/.bashrc`/`~/.profile`):

```sh
export RUSTUP_HOME="$USERPROFILE/scoop/persist/rustup/.rustup"
export CARGO_HOME="$USERPROFILE/scoop/persist/rustup/.cargo"
export PATH="$CARGO_HOME/bin:$PATH"
```

Install the ARM64 GNU/LLVM toolchain and set it as default:

```sh
rustup toolchain install stable-aarch64-pc-windows-gnullvm
rustup default stable-aarch64-pc-windows-gnullvm
```

> **Why gnullvm?** The MSVC toolchain requires Visual Studio Build Tools. The `gnullvm`
> target uses Rust's bundled `rust-lld` linker and LLVM's `clang` — no Microsoft tooling
> needed.

---

## Step 3 — MSYS2 (clangarm64 packages)

The gnullvm toolchain needs two things from MSYS2:

1. **Windows import libraries** (`.a` files for `kernel32`, `advapi32`, etc.)
2. **`clang.exe`** at build time (required by crates like `ring` that compile C/asm)

```sh
scoop install msys2
```

After scoop completes, run the initial MSYS2 setup (required once):

```sh
msys2
```

Then install the clangarm64 packages — you can do this from Git Bash:

```sh
MSYS2_ROOT="$USERPROFILE/scoop/apps/msys2/current"
"$MSYS2_ROOT/usr/bin/bash" -l -c "pacman -S --noconfirm mingw-w64-clang-aarch64-headers mingw-w64-clang-aarch64-crt mingw-w64-clang-aarch64-clang"
```

---

## Step 4 — Cargo config

Create `~/.cargo/config.toml` (i.e. `$CARGO_HOME/config.toml`) with the following.
This tells cargo to use rust-lld as the linker and points the `cc` crate at clang from MSYS2:

```toml
# ~/.cargo/config.toml
[target.aarch64-pc-windows-gnullvm]
linker = "rust-lld"
rustflags = [
    "-L", "C:/Users/<YOU>/scoop/apps/msys2/current/clangarm64/lib",
]

[env]
CC_aarch64_pc_windows_gnullvm  = "C:/Users/<YOU>/scoop/apps/msys2/current/clangarm64/bin/clang.exe"
CXX_aarch64_pc_windows_gnullvm = "C:/Users/<YOU>/scoop/apps/msys2/current/clangarm64/bin/clang++.exe"
AR_aarch64_pc_windows_gnullvm  = "C:/Users/<YOU>/scoop/apps/msys2/current/clangarm64/bin/llvm-ar.exe"
```

Replace `<YOU>` with your Windows username.

---

## Step 5 — Node.js via fnm

**Do not use nvm-windows** — as of v1.2.2 it silently installs the x64 Node.js binary
on ARM64 machines (see [rough-edges item 4](windows-arm64-rough-edges.md#4-nvm-install-arm64-silently-downloads-x64)).
Use [fnm](https://github.com/Schniz/fnm) instead.

```sh
scoop install fnm
```

> **Note:** As of fnm 1.39.0, scoop ships an x64 fnm binary (no ARM64 build yet).
> It runs fine under emulation but defaults `FNM_ARCH` to `x64`. Set `FNM_ARCH=arm64`
> in your shell environment (see Step 6) so that `fnm install` downloads native ARM64
> Node.js binaries.

Install the current LTS with the ARM64 architecture:

```sh
export FNM_ARCH=arm64
fnm install --lts
fnm default lts-latest
```

**Important:** fnm requires `fnm env` to be evaluated in every shell session before
`fnm use` or `fnm default` will work. Add the appropriate line to your shell profile
(see Step 6 for the `.bashrc` block, or the
[fnm shell setup guide](https://github.com/Schniz/fnm#shell-setup) for other shells):

- **Git Bash:** `eval "$(fnm env --use-on-cd --shell bash)"`
- **PowerShell:** `fnm env --use-on-cd --shell powershell | Out-String | Invoke-Expression`

See also [rough-edges item 5](windows-arm64-rough-edges.md#5-fnm-requires-fnm-env-evaluation-in-every-shell)
if you hit the "can't find the necessary environment variables" error.

Verify:

```sh
eval "$(fnm env --use-on-cd --shell bash)"
node -e "console.log(process.arch)"
# arm64
```

---

## Step 6 — Shell environment (env vars)

Scoop sets `RUSTUP_HOME`, `CARGO_HOME`, and the cargo `bin` PATH entry as **Windows
user environment variables** — these are visible in new PowerShell/cmd sessions
automatically. However, Git Bash sessions that were opened before the scoop install (or
that don't source the Windows environment fully) won't see them.

The safest approach is to add an explicit block to `~/.bashrc` (Git Bash reads this on
every new session):

```sh
# ~/.bashrc — ipcprims / 3 Leaps dev environment

# ── Rust ─────────────────────────────────────────────────────────────────────
# scoop sets these as Windows user env vars; repeat here for Git Bash safety
export RUSTUP_HOME="$USERPROFILE/scoop/persist/rustup/.rustup"
export CARGO_HOME="$USERPROFILE/scoop/persist/rustup/.cargo"

# ── Go ───────────────────────────────────────────────────────────────────────
# Go extracted manually to scoop apps dir (native ARM64 binary)
export GOPATH="$USERPROFILE/go"

# ── Node.js (fnm) ───────────────────────────────────────────────────────────
# fnm ships as x64 via scoop — force ARM64 Node.js downloads
export FNM_ARCH="arm64"
eval "$(fnm env --shell bash)"

# ── PATH ─────────────────────────────────────────────────────────────────────
export PATH="\
$CARGO_HOME/bin:\
$USERPROFILE/scoop/apps/go/go/bin:\
$USERPROFILE/scoop/apps/msys2/current/clangarm64/bin:\
$PATH"

# ── RUSTFLAGS ─────────────────────────────────────────────────────────────────
# Required for make ts-build: napi build spawns cargo via Node.js/npm and does
# not inherit CARGO_HOME, so config.toml rustflags are not applied. RUSTFLAGS
# is a plain env var and always flows through. See windows-arm64-rough-edges.md.
export RUSTFLAGS="-L $USERPROFILE/scoop/apps/msys2/current/clangarm64/lib"
```

**Why `clangarm64/bin` on PATH?** Some Rust crates invoke `clang.exe` directly at build
time (notably `ring`, used by TLS-dependent tools like `cargo-deny`). Having it on PATH
means `make bootstrap` works without any extra flags.

> **PowerShell / cmd users:** the Windows user env vars set by scoop are sufficient —
> no `.bashrc` equivalent needed. Add `%USERPROFILE%\scoop\apps\go\go\bin` and
> `%USERPROFILE%\scoop\apps\msys2\current\clangarm64\bin` to your user PATH via
> System Properties → Advanced → Environment Variables. For fnm, add
> `fnm env --use-on-cd | Out-String | Invoke-Expression` to your PowerShell profile
> and set `FNM_ARCH=arm64` as a user environment variable.

---

## Step 7 — Go (native ARM64)

Go has shipped a native **windows/arm64** binary since Go 1.17. Do **not** use the
`windows-amd64` build on this machine — the native binary is available and preferred.

Download and extract manually (scoop's main bucket does not carry Go):

```sh
VER="go1.26.1"
curl -fL "https://go.dev/dl/${VER}.windows-arm64.zip" -o /tmp/go-arm64.zip
7z x /tmp/go-arm64.zip -o"$USERPROFILE/scoop/apps/go" -y
```

Add to PATH (add to `~/.bashrc`):

```sh
export PATH="$USERPROFILE/scoop/apps/go/go/bin:$PATH"
```

Verify:

```sh
go version
# go version go1.26.1 windows/arm64
```

> **Why not scoop?** Scoop's main bucket does not include Go at the time of writing.
> The manual extraction into the scoop apps directory keeps things tidy alongside
> other scoop-managed tools.

---

## Step 8 — Bootstrap

With the above in place, `make bootstrap` runs cleanly:

```sh
make bootstrap
```

Expected output:

```
[ok] curl found
[ok] cargo: cargo X.Y.Z
[ok] sfetch already installed
[ok] goneat already installed
[..] Checking Rust dev tools...
[ok] cargo-deny installed
[ok] cargo-audit installed
[ok] cargo-edit installed
[ok] Bootstrap complete
```

For Go bindings, also install `cbindgen` (generates the C header from the FFI crate):

```sh
cargo install cbindgen --locked
```

**sfetch and goneat** can be installed via the 3leaps scoop bucket:

```sh
scoop bucket add 3leaps https://github.com/3leaps/scoop-bucket
scoop install sfetch goneat
```

If the scoop bucket doesn't carry goneat yet, install goneat via sfetch after sfetch is available:

```sh
sfetch --repo fulmenhq/goneat --tag v0.5.1
```

---

## Step 9 — TypeScript N-API bindings

The `ipcprims-napi` crate requires a GNU-format Node.js import library (`libnode.dll.a`)
that neither fnm nor nvm ships. A workaround for `CARGO_HOME` not being inherited by
`napi build` when invoked via npm is also needed.

Both issues are documented in detail with copy-paste commands in
**[windows-arm64-rough-edges.md](windows-arm64-rough-edges.md)** (items 1 and 2).

The short version: follow rough-edge item 2 to generate `~/node-arm64-lib/` from fnm's
ARM64 node.exe, set `LIBNODE_PATH` in `config.toml`, and add `RUSTFLAGS` to your
`.bashrc` (see Step 6 for the full `.bashrc` block including this). After that, both
`cargo build -p ipcprims-napi --release` and `make ts-build` work.

---

## Known limitations on Windows

| Feature | Status | Notes |
|---|---|---|
| Core Rust build (`cargo build`) | ✅ | All crates except N-API |
| Test suite (`cargo test`) | ✅ | Full pass |
| cargo-deny / cargo-audit | ✅ | Requires MSYS2 clangarm64 |
| TypeScript N-API bindings (`make ts-build`) | ✅ | Requires Step 9 + `RUSTFLAGS` in `.bashrc`; see rough-edges annex |
| Go bindings (`make go-build`) | ✅ | Go installed via Step 7; `cbindgen` installed via bootstrap |
| `make check-windows-msvc` | ❌ | Requires MSVC; skip on GNU setup |

---

## MSVC alternative

If you have or install Visual Studio Build Tools:

```sh
winget install Microsoft.VisualStudio.2022.BuildTools
# Select: MSVC aarch64 build tools + Windows SDK
```

Then use the MSVC toolchain instead:

```sh
rustup toolchain install stable-aarch64-pc-windows-msvc
rustup default stable-aarch64-pc-windows-msvc
```

No `config.toml` linker config needed. `make bootstrap` runs without MSYS2.
