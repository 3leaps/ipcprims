package ipcprims

import "fmt"

// ErrorCode maps to ipcprims FFI result codes.
type ErrorCode int32

const (
	ErrOK                 ErrorCode = 0
	ErrInvalidArgument    ErrorCode = 1
	ErrTransport          ErrorCode = 2
	ErrFrame              ErrorCode = 3
	ErrHandshakeFailed    ErrorCode = 4
	ErrDisconnected       ErrorCode = 5
	ErrUnsupportedChannel ErrorCode = 6
	ErrBufferFull         ErrorCode = 7
	ErrTimeout            ErrorCode = 8
	ErrShutdownFailed     ErrorCode = 9
	ErrSchema             ErrorCode = 10
	ErrInternal           ErrorCode = 99
)

func (c ErrorCode) String() string {
	switch c {
	case ErrOK:
		return "OK"
	case ErrInvalidArgument:
		return "InvalidArgument"
	case ErrTransport:
		return "Transport"
	case ErrFrame:
		return "Frame"
	case ErrHandshakeFailed:
		return "HandshakeFailed"
	case ErrDisconnected:
		return "Disconnected"
	case ErrUnsupportedChannel:
		return "UnsupportedChannel"
	case ErrBufferFull:
		return "BufferFull"
	case ErrTimeout:
		return "Timeout"
	case ErrShutdownFailed:
		return "ShutdownFailed"
	case ErrSchema:
		return "Schema"
	case ErrInternal:
		return "Internal"
	default:
		return "Unknown"
	}
}

// Error wraps an ipcprims FFI failure code and message.
type Error struct {
	Code    ErrorCode
	Message string
}

func (e *Error) Error() string {
	if e.Message == "" {
		return e.Code.String()
	}
	return fmt.Sprintf("%s: %s", e.Code, e.Message)
}

func checkResult(code int32, msg string) error {
	if ErrorCode(code) == ErrOK {
		return nil
	}
	return &Error{Code: ErrorCode(code), Message: msg}
}
