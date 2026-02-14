//go:build cgo && linux && amd64

package ipcprims

/*
#cgo CFLAGS: -I${SRCDIR}/include
#cgo LDFLAGS: -L${SRCDIR}/lib/local/linux-amd64 -L${SRCDIR}/lib/linux-amd64 -lipcprims_ffi -lm -lpthread -ldl
#include "ipcprims.h"
*/
import "C"
