//go:build cgo && linux && arm64

package ipcprims

/*
#cgo CFLAGS: -I${SRCDIR}/include
#cgo LDFLAGS: -L${SRCDIR}/lib/local/linux-arm64 -L${SRCDIR}/lib/linux-arm64 -lipcprims_ffi -lm -lpthread -ldl
#include "ipcprims.h"
*/
import "C"
