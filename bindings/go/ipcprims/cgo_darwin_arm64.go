//go:build cgo && darwin && arm64

package ipcprims

/*
#cgo CFLAGS: -I${SRCDIR}/include
#cgo LDFLAGS: -L${SRCDIR}/lib/local/darwin-arm64 -L${SRCDIR}/lib/darwin-arm64 -lipcprims_ffi -lm -lpthread
#include "ipcprims.h"
*/
import "C"
