//go:build ipcprims_schema

package ipcprims

import (
	"errors"
	"runtime"
	"sync"
	"unsafe"
)

var errSchemaRegistryClosed = errors.New("ipcprims: schema registry is closed")

// SchemaRegistry wraps an FFI schema registry handle.
type SchemaRegistry struct {
	mu     sync.Mutex
	handle unsafe.Pointer
	closed bool
}

// NewSchemaRegistryFromDir loads channel schema files from directory.
func NewSchemaRegistryFromDir(path string) (*SchemaRegistry, error) {
	handle, code, msg := ffiSchemaRegistryFromDirectory(path)
	if err := checkResult(code, msg); err != nil {
		return nil, err
	}
	registry := &SchemaRegistry{handle: handle}
	runtime.SetFinalizer(registry, func(r *SchemaRegistry) {
		_ = r.Close()
	})
	return registry, nil
}

// Validate validates payload against schema for channel.
func (r *SchemaRegistry) Validate(channel uint16, data []byte) error {
	handle, err := r.getHandle()
	if err != nil {
		return err
	}
	code, msg := ffiSchemaRegistryValidate(handle, channel, data)
	return checkResult(code, msg)
}

// Close releases schema registry handle.
func (r *SchemaRegistry) Close() error {
	if r == nil {
		return nil
	}

	r.mu.Lock()
	defer r.mu.Unlock()
	if r.closed {
		return nil
	}
	if r.handle != nil {
		ffiSchemaRegistryFree(r.handle)
		r.handle = nil
	}
	r.closed = true
	runtime.SetFinalizer(r, nil)
	return nil
}

func (r *SchemaRegistry) getHandle() (unsafe.Pointer, error) {
	if r == nil {
		return nil, errSchemaRegistryClosed
	}

	r.mu.Lock()
	defer r.mu.Unlock()
	if r.closed || r.handle == nil {
		return nil, errSchemaRegistryClosed
	}
	return r.handle, nil
}
