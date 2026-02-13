# CLI Dogfooding Guide

This guide shows how to run a practical end-to-end CLI exercise for `ipcprims`.

It is intended for:

- developers validating local changes
- QA verifying expected command behavior
- operators learning command semantics

## One-command run

From repository root:

```bash
make dogfood-cli
```

Equivalent direct command:

```bash
bash scripts/dogfood/cli-matrix.sh
```

## What it validates

The matrix exercises:

1. `version` and `version --extended`
2. `doctor` and `envinfo` JSON output
3. `info` timeout behavior (`124` expected on missing socket)
4. `echo` + `send --wait` roundtrip on `COMMAND`
5. `echo --validate` handling of invalid payloads (schema error response, process stays alive)
6. `listen --count` bounded receive behavior
7. output format spot-check (`json`, `table`, `pretty`, `raw`)

## Fixture inputs

The dogfood run uses a committed schema fixture:

- `tests/fixtures/schemas/command.schema.json`

This ensures deterministic schema-validation behavior.

## Artifacts

The script writes temporary artifacts under `/tmp/ipcprims-dogfood-<pid>/artifacts` while running.

Key logs:

- `echo.stdout.log`
- `echo.stderr.log`
- `listen.stdout.log`
- `listen.stderr.log`

The directory is cleaned up automatically on script exit.

## Exit behavior

- Exit `0`: matrix passed
- Non-zero: at least one behavior check failed (script prints the failing expectation)

## Troubleshooting

- If timeout helper binaries differ by platform, the script auto-detects in this order:
  1. `timeout`
  2. `gtimeout`
  3. `sysprims timeout`
- On macOS, if `timeout` is unavailable, install coreutils (`gtimeout`) or use `sysprims`.
