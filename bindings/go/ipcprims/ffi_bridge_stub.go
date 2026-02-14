//go:build !cgo || !((linux && (amd64 || arm64)) || (darwin && (amd64 || arm64)))

package ipcprims

import "unsafe"

const (
	ffiChannelControl   uint16 = 0
	ffiChannelCommand   uint16 = 1
	ffiChannelData      uint16 = 2
	ffiChannelTelemetry uint16 = 3
	ffiChannelError     uint16 = 4
)

func ffiUnsupported() (int32, string) {
	return 99, "ipcprims Go bindings currently support linux and darwin targets"
}

func ffiInit() (int32, string) { return ffiUnsupported() }

func ffiCleanup() {}

func ffiLastError() string {
	_, msg := ffiUnsupported()
	return msg
}

func ffiListenerBind(_ string) (unsafe.Pointer, int32, string) {
	code, msg := ffiUnsupported()
	return nil, code, msg
}

func ffiListenerAccept(_ unsafe.Pointer) (unsafe.Pointer, int32, string) {
	code, msg := ffiUnsupported()
	return nil, code, msg
}

func ffiListenerFree(_ unsafe.Pointer) {}

func ffiConnect(_ string, _ []uint16) (unsafe.Pointer, int32, string) {
	code, msg := ffiUnsupported()
	return nil, code, msg
}

func ffiPeerSend(_ unsafe.Pointer, _ uint16, _ []byte) (int32, string) { return ffiUnsupported() }

func ffiPeerRecv(_ unsafe.Pointer) (int32, string, uint16, []byte) {
	code, msg := ffiUnsupported()
	return code, msg, 0, nil
}

func ffiPeerRecvOn(_ unsafe.Pointer, _ uint16) (int32, string, uint16, []byte) {
	code, msg := ffiUnsupported()
	return code, msg, 0, nil
}

func ffiPeerPing(_ unsafe.Pointer) (int32, string, uint64) {
	code, msg := ffiUnsupported()
	return code, msg, 0
}

func ffiPeerShutdown(_ unsafe.Pointer) (int32, string) { return ffiUnsupported() }

func ffiPeerFree(_ unsafe.Pointer) {}

func ffiSchemaRegistryFromDirectory(_ string) (unsafe.Pointer, int32, string) {
	code, msg := ffiUnsupported()
	return nil, code, msg
}

func ffiSchemaRegistryValidate(_ unsafe.Pointer, _ uint16, _ []byte) (int32, string) {
	return ffiUnsupported()
}

func ffiSchemaRegistryFree(_ unsafe.Pointer) {}
