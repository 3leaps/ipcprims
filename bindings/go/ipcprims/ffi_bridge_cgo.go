//go:build cgo && ((linux && (amd64 || arm64)) || (darwin && (amd64 || arm64)))

package ipcprims

/*
#include "ipcprims.h"
#include <stdlib.h>
*/
import "C"

import (
	"runtime"
	"unsafe"
)

const (
	ffiChannelControl   = uint16(C.IPC_CHANNEL_CONTROL)
	ffiChannelCommand   = uint16(C.IPC_CHANNEL_COMMAND)
	ffiChannelData      = uint16(C.IPC_CHANNEL_DATA)
	ffiChannelTelemetry = uint16(C.IPC_CHANNEL_TELEMETRY)
	ffiChannelError     = uint16(C.IPC_CHANNEL_ERROR)
)

func ffiCallResult(call func() C.IpcResult) (int32, string) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	code := call()
	if code == C.IPC_RESULT_OK {
		return int32(code), ""
	}
	return int32(code), C.GoString(C.ipc_last_error())
}

func ffiInit() (int32, string) {
	return ffiCallResult(func() C.IpcResult { return C.ipc_init() })
}

func ffiCleanup() {
	C.ipc_cleanup()
}

func ffiLastError() string {
	return C.GoString(C.ipc_last_error())
}

func ffiListenerBind(path string) (unsafe.Pointer, int32, string) {
	cPath := C.CString(path)
	defer C.free(unsafe.Pointer(cPath))

	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	handle := C.ipc_listener_bind(cPath)
	if handle == nil {
		return nil, int32(C.IPC_RESULT_INTERNAL), C.GoString(C.ipc_last_error())
	}
	return unsafe.Pointer(handle), int32(C.IPC_RESULT_OK), ""
}

func ffiListenerAccept(listener unsafe.Pointer) (unsafe.Pointer, int32, string) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	handle := C.ipc_listener_accept((C.IpcListenerHandle)(listener))
	if handle == nil {
		return nil, int32(C.IPC_RESULT_INTERNAL), C.GoString(C.ipc_last_error())
	}
	return unsafe.Pointer(handle), int32(C.IPC_RESULT_OK), ""
}

func ffiListenerFree(listener unsafe.Pointer) {
	C.ipc_listener_free((C.IpcListenerHandle)(listener))
}

func ffiConnect(path string, channels []uint16) (unsafe.Pointer, int32, string) {
	cPath := C.CString(path)
	defer C.free(unsafe.Pointer(cPath))

	var cChannels *C.uint16_t
	if len(channels) > 0 {
		cChannels = (*C.uint16_t)(unsafe.Pointer(&channels[0]))
	}

	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	handle := C.ipc_connect(cPath, cChannels, C.uintptr_t(len(channels)))
	if handle == nil {
		return nil, int32(C.IPC_RESULT_INTERNAL), C.GoString(C.ipc_last_error())
	}
	return unsafe.Pointer(handle), int32(C.IPC_RESULT_OK), ""
}

func ffiPeerSend(peer unsafe.Pointer, channel uint16, payload []byte) (int32, string) {
	var ptr *C.uint8_t
	if len(payload) > 0 {
		ptr = (*C.uint8_t)(unsafe.Pointer(&payload[0]))
	}
	return ffiCallResult(func() C.IpcResult {
		return C.ipc_peer_send(
			(C.IpcPeerHandle)(peer),
			C.uint16_t(channel),
			ptr,
			C.uintptr_t(len(payload)),
		)
	})
}

func ffiPeerRecv(peer unsafe.Pointer) (int32, string, uint16, []byte) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var frame C.struct_IpcFrame
	code := C.ipc_peer_recv((C.IpcPeerHandle)(peer), &frame)
	if code != C.IPC_RESULT_OK {
		return int32(code), C.GoString(C.ipc_last_error()), 0, nil
	}
	payload := ffiFramePayload(frame)
	return int32(code), "", uint16(frame.channel), payload
}

func ffiPeerRecvOn(peer unsafe.Pointer, channel uint16) (int32, string, uint16, []byte) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var frame C.struct_IpcFrame
	code := C.ipc_peer_recv_on((C.IpcPeerHandle)(peer), C.uint16_t(channel), &frame)
	if code != C.IPC_RESULT_OK {
		return int32(code), C.GoString(C.ipc_last_error()), 0, nil
	}
	payload := ffiFramePayload(frame)
	return int32(code), "", uint16(frame.channel), payload
}

func ffiFramePayload(frame C.struct_IpcFrame) []byte {
	defer C.ipc_frame_free(&frame)

	if frame.data == nil || frame.len == 0 {
		return nil
	}
	return C.GoBytes(unsafe.Pointer(frame.data), C.int(frame.len))
}

func ffiPeerPing(peer unsafe.Pointer) (int32, string, uint64) {
	var rtt uint64
	code, msg := ffiCallResult(func() C.IpcResult {
		return C.ipc_peer_ping((C.IpcPeerHandle)(peer), (*C.uint64_t)(unsafe.Pointer(&rtt)))
	})
	return code, msg, rtt
}

func ffiPeerShutdown(peer unsafe.Pointer) (int32, string) {
	return ffiCallResult(func() C.IpcResult {
		return C.ipc_peer_shutdown((C.IpcPeerHandle)(peer))
	})
}

func ffiPeerFree(peer unsafe.Pointer) {
	C.ipc_peer_free((C.IpcPeerHandle)(peer))
}

func ffiSchemaRegistryFromDirectory(path string) (unsafe.Pointer, int32, string) {
	cPath := C.CString(path)
	defer C.free(unsafe.Pointer(cPath))

	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	handle := C.ipc_schema_registry_from_directory(cPath)
	if handle == nil {
		return nil, int32(C.IPC_RESULT_INTERNAL), C.GoString(C.ipc_last_error())
	}
	return unsafe.Pointer(handle), int32(C.IPC_RESULT_OK), ""
}

func ffiSchemaRegistryValidate(registry unsafe.Pointer, channel uint16, payload []byte) (int32, string) {
	var ptr *C.uint8_t
	if len(payload) > 0 {
		ptr = (*C.uint8_t)(unsafe.Pointer(&payload[0]))
	}
	return ffiCallResult(func() C.IpcResult {
		return C.ipc_schema_registry_validate(
			(C.IpcSchemaRegistryHandle)(registry),
			C.uint16_t(channel),
			ptr,
			C.uintptr_t(len(payload)),
		)
	})
}

func ffiSchemaRegistryFree(registry unsafe.Pointer) {
	C.ipc_schema_registry_free((C.IpcSchemaRegistryHandle)(registry))
}
