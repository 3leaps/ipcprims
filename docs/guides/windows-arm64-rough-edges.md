# Windows ARM64 — Rough Edges (GNU Toolchain)

> **This document is a temporary annex** to [windows-dev-setup.md](windows-dev-setup.md).
> It captures workarounds for issues that are specific to the early state of Windows ARM64
> native toolchain support. As the ecosystem matures these should become unnecessary.

---

## 1. CARGO_HOME not inherited by napi build

**Symptom:** `make ts-build` fails with `lld: error: unable to find library -ladvapi32`
even though a direct `cargo build -p ipcprims-napi --release` works fine.

**Root cause:** `napi build` spawns cargo as a child of Node.js/npm. `CARGO_HOME` is not
reliably inherited into that subprocess (a napi-rs + Node.js version manager interaction),
so cargo does not find the `[target.aarch64-pc-windows-gnullvm]` rustflags in
`$CARGO_HOME/config.toml`.

**Workaround:** Export `RUSTFLAGS` directly in your shell — it is a plain env var and
always flows through to child processes:

```sh
export RUSTFLAGS="-L C:/Users/<YOU>/scoop/apps/msys2/current/clangarm64/lib"
```

Add this to the `~/.bashrc` block alongside the other PATH exports (see
[windows-dev-setup.md Step 6](windows-dev-setup.md#step-6--shell-environment-env-vars)).
Replace `<YOU>` with your Windows username.

**Will fix when:** napi-rs properly propagates `CARGO_HOME` to cargo subprocesses, or if
`CARGO_HOME` is set as a persistent Windows user env var (which scoop does — it just
doesn't reach the current bash session).

---

## 2. N-API TypeScript bindings: libnode.dll import library missing

**Symptom:** `cargo build -p ipcprims-napi` fails with `libnode.dll not found in any
search path`.

**Root cause:** The `napi-build` crate needs a GNU-format import library
(`libnode.dll.a`) for Node.js so the linker can resolve N-API symbols. Neither fnm nor
Node.js ships this for the GNU toolchain — only an MSVC-format `node.lib` is provided
in official distributions, and even that is not included in fnm's installation.

**Workaround:** Generate the import library from fnm's ARM64 node.exe (requires
[fnm with `FNM_ARCH=arm64`](windows-dev-setup.md#step-5--nodejs-via-fnm)):

```sh
# fnm's ARM64 node.exe lives here (adjust version as needed):
NODE_EXE="$APPDATA/fnm/node-versions/v24.14.1/installation/node.exe"

LLVM="$USERPROFILE/scoop/apps/msys2/current/clangarm64/bin"
LIB_DIR="$HOME/node-arm64-lib"
mkdir -p "$LIB_DIR"

# 1. Generate .def file from node.exe export table
echo "EXPORTS" > "$LIB_DIR/node.def"
"$LLVM/llvm-objdump.exe" -p "$NODE_EXE" | awk '/^ *[0-9]+ 0x[0-9a-f]+ / {print $3}' >> "$LIB_DIR/node.def"

# 2. Create GNU import library
"$LLVM/llvm-dlltool.exe" -m arm64 -D node.exe -d "$LIB_DIR/node.def" -l "$LIB_DIR/libnode.dll.a"

# 3. Copy node.exe as libnode.dll (napi-build existence check)
cp "$NODE_EXE" "$LIB_DIR/libnode.dll"
```

Then set `LIBNODE_PATH` in `$CARGO_HOME/config.toml`:

```toml
[env]
LIBNODE_PATH = "C:/Users/<YOU>/node-arm64-lib"
```

> **Note:** When you update Node.js via `fnm install`, re-run the steps above to
> regenerate `libnode.dll.a` from the new node.exe.

**Will fix when:** napi-rs ships prebuilt import libraries for Windows ARM64.

---

## 3. go env GOOS / GOARCH errors in make

**Symptom:** Every `make` invocation prints:

```
process_begin: CreateProcess(NULL, go env GOOS, ...) failed.
Makefile:43: pipe: Bad file descriptor
```

**Root cause:** The Makefile evaluates `$(shell go env GOOS)` and `$(shell go env GOARCH)`
at parse time for all targets — including ones that don't use Go. If `go` is not on PATH
when make starts, this fails noisily. It is benign (make continues, Go targets just get
empty variables).

**Workaround:** Ensure Go is on PATH before running any make target (see
[windows-dev-setup.md Step 6](windows-dev-setup.md#step-6--shell-environment-env-vars)).

**Will fix when:** The Makefile is updated to lazy-evaluate or guard these shell calls.

---

## 4. nvm-windows does not support ARM64 — use fnm instead

**Symptom:** `nvm install 24 arm64` reports success and reports "64-bit" but
`node -e "console.log(process.arch)"` prints `x64`.

**Root cause:** nvm-windows v1.2.2 has partial ARM64 support but does not consistently
use the ARM64 distribution archive from nodejs.org. It silently downloads the x64 binary
even when `arm64` is explicitly requested.

**Resolution:** Use [fnm](https://github.com/Schniz/fnm) as the Node.js version manager
on Windows ARM64. fnm correctly downloads native ARM64 Node.js binaries when
`FNM_ARCH=arm64` is set:

```sh
scoop install fnm
export FNM_ARCH=arm64
fnm install --lts
```

> **Note:** As of fnm 1.39.0, scoop ships an x64 fnm binary (no ARM64 build available
> yet). fnm itself runs under emulation, which is fine — the important thing is that
> `FNM_ARCH=arm64` causes it to download and manage native ARM64 Node.js binaries.
> Add `export FNM_ARCH="arm64"` to your `~/.bashrc` so all installs default to ARM64.

See [windows-dev-setup.md Step 5](windows-dev-setup.md#step-5--nodejs-via-fnm) for the
full setup.

**Status:** Resolved. nvm-windows is no longer recommended for ARM64 development.

---

## 5. fnm requires `fnm env` evaluation in every shell

**Symptom:** `fnm use 24` fails with:

```
error: We can't find the necessary environment variables to replace the Node version.
You should setup your shell profile to evaluate `fnm env`
```

**Root cause:** fnm uses per-shell multishell directories to manage the active Node
version. Unlike nvm-windows which sets a global symlink, fnm requires its environment
variables (`FNM_MULTISHELL_PATH`, `PATH` entries) to be initialized in each shell
session via `fnm env`. Without this, `fnm use` and `fnm default` cannot swap the
active Node.

**Resolution:** Evaluate `fnm env` in your shell profile. The exact invocation differs
per shell — see the [fnm shell setup guide](https://github.com/Schniz/fnm#shell-setup)
for the full list.

**Git Bash** (`~/.bashrc`):

```sh
export FNM_ARCH="arm64"
eval "$(fnm env --use-on-cd --shell bash)"
```

**PowerShell** (`$PROFILE`):

```powershell
$env:FNM_ARCH = "arm64"
fnm env --use-on-cd --shell powershell | Out-String | Invoke-Expression
```

The `--use-on-cd` flag is optional but recommended — it auto-switches Node versions
when you `cd` into a directory containing a `.node-version` or `.nvmrc` file.

> **Caution:** Replacing the active Node binary (e.g. `fnm install` + `fnm use` for a
> new version) will kill any running Node process in that shell, including Claude Code.
> Install new versions, then restart your shell/IDE.

**Will fix when:** fnm adds a persistent default that works without per-shell env setup
(tracked upstream).

---

## 6. MSVC-linked npm packages need the VC++ runtime

**Symptom:** A globally installed npm tool (e.g. `biome`) crashes immediately with
`error while loading shared libraries` (Git Bash) or a silent non-zero exit
(PowerShell).

**Root cause:** Some npm packages ship native MSVC-linked binaries (e.g.
`@biomejs/cli-win32-arm64`). These require `vcruntime140.dll`, which is part of
the Microsoft Visual C++ Redistributable. A pure gnullvm toolchain setup does
not include this DLL.

**Resolution:** Install the ARM64 VC++ Redistributable manually (interactive —
requires accepting license terms and has GUI confirmation dialogs):

```powershell
winget install Microsoft.VCRedist.2015+.arm64
```

> **Note:** This must be run interactively in PowerShell or cmd — it prompts for
> agreement to source terms and pops up an installer GUI. It cannot be scripted
> silently.

**Will fix when:** Not applicable — this is a permanent dependency for
MSVC-linked native binaries. Document as a prerequisite for the gnullvm setup.
