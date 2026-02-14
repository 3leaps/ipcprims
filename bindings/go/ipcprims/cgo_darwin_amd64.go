//go:build cgo && darwin && amd64

package ipcprims

/*
#cgo CFLAGS: -I${SRCDIR}/include
#cgo LDFLAGS: -L${SRCDIR}/lib/local/darwin-amd64 -L${SRCDIR}/lib/darwin-amd64 -lipcprims_ffi -lm -lpthread
#include "ipcprims.h"
*/
import "C"
