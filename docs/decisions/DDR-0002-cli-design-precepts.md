# DDR-0002: CLI Design Precepts

**Status**: Accepted
**Date**: 2026-02-06
**Deciders**: Architecture Council

## Context

ipcprims ships a CLI binary that serves as both a diagnostic/debugging tool and a genuinely useful operational tool for IPC workflows. The 3leaps ecosystem maintains consistent CLI conventions across sysprims, docprims, and ipcprims. These precepts align with patterns established in sysprims and the rsfulmen foundry library.

## Decision

The ipcprims CLI follows these precepts:

### P1: Stdout Purity

- **stdout**: Only machine-readable programmatic output (command results, frame data)
- **stderr**: All logging, diagnostics, warnings, progress indicators
- JSON-formatted logs go to stderr when `--log-format json` is used
- This enables clean piping: `ipcprims listen /tmp/test.sock --format json | jq .`

### P2: Consistent `--format` Option

All commands that produce output support a `--format` flag:

| Value    | Behavior                                                                                           |
| -------- | -------------------------------------------------------------------------------------------------- |
| `json`   | Structured JSON to stdout, one object per line for streaming commands. Includes `schema_id` field. |
| `table`  | Human-readable aligned columns                                                                     |
| `pretty` | Decorated/colorized output (alias or extended table)                                               |
| `raw`    | Unformatted payload bytes (for piping to hexdump, etc.)                                            |

Default varies by context: `json` when stdout is not a TTY (pipe-friendly), `table` when stdout is a TTY (human-friendly).

### P3: `version` with `--extended`

- `ipcprims version` — prints semver version
- `ipcprims version --extended` — prints version, git hash, build timestamp, Rust toolchain, target triple, rsfulmen/crucible version. Useful for bug reports and provenance.

### P4: `doctor` Command

`ipcprims doctor` performs context-specific health checks:

- Verify runtime dependencies (socket paths accessible, schema directories readable)
- Check platform capabilities (UDS support, named pipe support)
- Validate configuration files if present
- Report rsfulmen foundry version alignment

Output is a checklist of pass/fail items. Exit code 0 if all pass, non-zero if any fail.

### P5: `envinfo` Command

`ipcprims envinfo` reports diagnostic information:

- Platform (OS, arch, kernel version)
- Rust version used to compile
- ipcprims version and build provenance
- Relevant environment variables (IPC paths, config overrides)
- Feature flags compiled in

Useful for support tickets and CI diagnostics.

### P6: Exit Codes and Signals (rsfulmen-aligned)

For v0.1.0 scaffold, ipcprims defines local exit code constants aligned to
rsfulmen semantics and uses `ctrlc`-based signal handling. This keeps startup
friction low while preserving ecosystem-compatible codes.

**Exit code mapping:**

| Condition                    | Code  | rsfulmen constant          |
| ---------------------------- | ----- | -------------------------- |
| Success                      | 0     | `EXIT_SUCCESS`             |
| General failure              | 1     | `EXIT_FAILURE`             |
| Schema validation failed     | 60    | `EXIT_DATA_INVALID`        |
| Connection refused / timeout | 124   | `EXIT_TIMEOUT`             |
| Transport/IO error           | 3     | (ipcprims-specific)        |
| Permission denied            | 50    | `EXIT_PERMISSION_DENIED`   |
| Invalid arguments            | 64    | `EXIT_USAGE`               |
| ipcprims internal error      | 125   | `EXIT_TIMEOUT_INTERNAL`    |
| Signal-induced exit          | 128+N | Standard signal convention |

Signal handling is currently `ctrlc + AtomicBool` in long-running commands.

### P7: Schema-Identified JSON Output

All JSON output includes a `schema_id` field referencing a versioned schema URI:

```json
{
  "schema_id": "https://schemas.3leaps.dev/ipcprims/frame/v1.0.0/frame-received.schema.json",
  "channel": 1,
  "channel_name": "COMMAND",
  "payload_size": 42,
  "payload": "{\"action\":\"ping\"}"
}
```

Schema constants are defined in the core crate and imported by the CLI.

### P8: Logging via tracing

- Global options: `--log-format` (text|json) and `--log-level` (error|warn|info|debug|trace)
- Subscriber initialized once in `main()`, writes to stderr
- Library crates emit `tracing` events; only the CLI binary initializes the subscriber
- Structured fields in log events for machine parsing

## Consequences

**Positive:**

- Ecosystem consistency with sysprims and other 3leaps tools
- Pipe-friendly by default; human-friendly when interactive
- rsfulmen dependency provides cross-language exit code parity (same codes in Go/Python/TypeScript tools)
- `doctor` and `envinfo` reduce support burden

**Negative:**

- rsfulmen integration is deferred, so constants are mirrored locally until a
  later dependency consolidation pass
- Exit code 60 for schema validation differs from the D2 brief's proposed exit code 2 — but aligns with ecosystem standard

**Neutral:**

- TTY detection for default format requires `atty` or `std::io::IsTerminal` (stable since Rust 1.70)

## References

- sysprims CLI: ADR-0008 (error handling), ADR-0009 (logging strategy), ADR-0010 (schema management)
- rsfulmen foundry: exit_codes, signals modules
- Crucible exit code standard: `config/crucible-rs/library/foundry/exit-codes.yaml`
