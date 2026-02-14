// Package ipcprims provides Go bindings for ipcprims peer-level IPC.
//
// The bindings are synchronous and mirror the v0.1.x Rust peer API.
package ipcprims

const (
	CONTROL   = ffiChannelControl
	COMMAND   = ffiChannelCommand
	DATA      = ffiChannelData
	TELEMETRY = ffiChannelTelemetry
	ERROR     = ffiChannelError
)

// Init initializes thread-local error state in the underlying FFI library.
func Init() error {
	code, msg := ffiInit()
	return checkResult(code, msg)
}

// Cleanup clears thread-local error state in the underlying FFI library.
func Cleanup() {
	ffiCleanup()
}

// LastError returns the last FFI error string for the current OS thread.
func LastError() string {
	return ffiLastError()
}
