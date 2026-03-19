#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

BIN="target/debug/ipcprims"
FIXTURE_SCHEMA_DIR="$ROOT_DIR/tests/fixtures/schemas"

# Platform detection: Windows named pipes vs Unix domain sockets.
if [[ "${OS:-}" == "Windows_NT" ]] || [[ "$(uname -s 2>/dev/null)" == MINGW* ]] || [[ "$(uname -s 2>/dev/null)" == MSYS* ]]; then
	IS_WINDOWS=true
	BIN="target/debug/ipcprims.exe"
	UNIQUE="$$-$(date +%s)"
	# Use short names — normalize_pipe_name() prepends \\.\pipe\ automatically.
	SOCK_ECHO="ipcprims-dogfood-echo-${UNIQUE}"
	SOCK_LISTEN="ipcprims-dogfood-listen-${UNIQUE}"
	TMP_DIR="${TEMP:-/tmp}/ipcprims-dogfood-$$"
	ARTIFACT_DIR="$TMP_DIR/artifacts"
	mkdir -p "$ARTIFACT_DIR"
else
	IS_WINDOWS=false
	TMP_DIR="/tmp/ipcprims-dogfood-$$"
	SOCK_ECHO="$TMP_DIR/echo.sock"
	SOCK_LISTEN="$TMP_DIR/listen.sock"
	ARTIFACT_DIR="$TMP_DIR/artifacts"
	mkdir -p "$ARTIFACT_DIR"
fi

# Track background server PIDs for cleanup.
BG_PIDS=()

cleanup() {
	for pid in "${BG_PIDS[@]}"; do
		kill "$pid" >/dev/null 2>&1 || true
	done
	rm -rf "$TMP_DIR"
}
trap cleanup EXIT

run_with_timeout() {
	local seconds="$1"
	shift
	if command -v timeout >/dev/null 2>&1; then
		timeout "$seconds" "$@"
	elif command -v gtimeout >/dev/null 2>&1; then
		gtimeout "$seconds" "$@"
	elif command -v sysprims >/dev/null 2>&1; then
		sysprims timeout "$seconds" -- "$@"
	else
		"$@"
	fi
}

assert_contains() {
	local haystack="$1"
	local needle="$2"
	local context="$3"
	if [[ "$haystack" != *"$needle"* ]]; then
		echo "FAIL: expected '$needle' in $context"
		exit 1
	fi
}

wait_for_ready() {
	local sock="$1"
	local max_attempts=120
	if [[ "$IS_WINDOWS" == true ]]; then
		# Windows named pipes may take longer to become ready.
		max_attempts=200
	fi
	for _ in $(seq 1 $max_attempts); do
		if "$BIN" info "$sock" --timeout 1s >/dev/null 2>&1; then
			return 0
		fi
		sleep 0.1
	done
	echo "FAIL: service at $sock did not become ready"
	exit 1
}

# retry_cmd — retry a command that connects to a named pipe server.
# On Windows, the server must re-create the pipe instance between connections,
# so the next client may need to wait briefly for the pipe to reappear.
retry_cmd() {
	local max_attempts=1
	if [[ "$IS_WINDOWS" == true ]]; then
		max_attempts=30
	fi
	for attempt in $(seq 1 $max_attempts); do
		if output="$("$@" 2>&1)"; then
			echo "$output"
			return 0
		fi
		if [[ $attempt -eq $max_attempts ]]; then
			echo "$output" >&2
			return 1
		fi
		sleep 0.2
	done
}

echo
echo "== Build CLI =="
cargo build -p ipcprims --features cli >/dev/null

echo
echo "== Version =="
"$BIN" version
"$BIN" version --extended

echo
echo "== Doctor / Envinfo =="
doctor_json="$("$BIN" --format json doctor)"
assert_contains "$doctor_json" '"schema_id"' "doctor json"
envinfo_json="$("$BIN" --format json envinfo)"
assert_contains "$envinfo_json" '"target":"' "envinfo json"

echo
echo "== Info timeout (expect 124) =="
if [[ "$IS_WINDOWS" == true ]]; then
	MISSING_ADDR="ipcprims-dogfood-missing-$$"
else
	MISSING_ADDR="$TMP_DIR/missing.sock"
fi
set +e
"$BIN" info "$MISSING_ADDR" --timeout 1s >/dev/null 2>&1
code=$?
set -e
if [[ "$code" -ne 124 ]]; then
	echo "FAIL: expected info timeout exit 124, got $code"
	exit 1
fi

echo
echo "== Echo + Info + Send(wait) =="
"$BIN" --log-level error echo "$SOCK_ECHO" --validate "$FIXTURE_SCHEMA_DIR" \
	>"$ARTIFACT_DIR/echo.stdout.log" 2>"$ARTIFACT_DIR/echo.stderr.log" &
BG_PIDS+=($!)
wait_for_ready "$SOCK_ECHO"

info_json="$(retry_cmd "$BIN" --format json info "$SOCK_ECHO")"
assert_contains "$info_json" '"schema_id":"https://schemas.3leaps.dev/ipcprims/cli/v1/connection-info.schema.json"' "info json"
assert_contains "$info_json" '"connected":true' "info json"

send_ok="$(retry_cmd "$BIN" --format json send "$SOCK_ECHO" --channel 1 --json '{"action":"ping"}' --wait --wait-timeout 2s)"
assert_contains "$send_ok" '"channel":1' "send --wait response"

echo
echo "== Echo validate invalid payload (expect ERROR payload and process alive) =="
invalid_output="$(retry_cmd run_with_timeout 10s "$BIN" --format raw send "$SOCK_ECHO" --channel 1 --json '{"bad":"shape"}' --wait --wait-timeout 2s || true)"
assert_contains "$invalid_output" 'schema validation error' "invalid payload response"

still_alive="$(retry_cmd "$BIN" --format json send "$SOCK_ECHO" --channel 1 --json '{"action":"still-alive"}' --wait --wait-timeout 2s)"
assert_contains "$still_alive" '"channel":1' "post-invalid echo response"

echo
echo "== Listen + Send + Count =="
"$BIN" --log-level error listen "$SOCK_LISTEN" --count 2 --format json \
	>"$ARTIFACT_DIR/listen.stdout.log" 2>"$ARTIFACT_DIR/listen.stderr.log" &
LISTEN_PID=$!
BG_PIDS+=($LISTEN_PID)
wait_for_ready "$SOCK_LISTEN"
retry_cmd "$BIN" send "$SOCK_LISTEN" --channel 1 --data 'one' >/dev/null
retry_cmd "$BIN" send "$SOCK_LISTEN" --channel 2 --data 'two' >/dev/null
wait "$LISTEN_PID"

echo
echo "== Output format spot-check =="
"$BIN" --format table envinfo >/dev/null
"$BIN" --format pretty doctor >/dev/null
"$BIN" --format raw version >/dev/null

echo
echo "Dogfood matrix complete."
echo "Artifacts saved under: $ARTIFACT_DIR"
