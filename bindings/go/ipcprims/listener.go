package ipcprims

import (
	"errors"
	"runtime"
	"sync"
	"unsafe"
)

var errListenerClosed = errors.New("ipcprims: listener is closed")

// ListenerOption is reserved for future listener config.
type ListenerOption func(*listenerConfig)

type listenerConfig struct{}

// Listener wraps an FFI listener handle.
type Listener struct {
	mu     sync.Mutex
	handle unsafe.Pointer
	closed bool
}

// Listen binds a listener socket path.
func Listen(path string, opts ...ListenerOption) (*Listener, error) {
	cfg := &listenerConfig{}
	for _, opt := range opts {
		if opt != nil {
			opt(cfg)
		}
	}

	handle, code, msg := ffiListenerBind(path)
	if err := checkResult(code, msg); err != nil {
		return nil, err
	}
	listener := &Listener{handle: handle}
	runtime.SetFinalizer(listener, func(l *Listener) {
		_ = l.Close()
	})
	return listener, nil
}

// Accept waits for and accepts a peer connection.
func (l *Listener) Accept() (*Peer, error) {
	if l == nil {
		return nil, errListenerClosed
	}

	l.mu.Lock()
	defer l.mu.Unlock()
	if l.closed || l.handle == nil {
		return nil, errListenerClosed
	}

	handle, code, msg := ffiListenerAccept(l.handle)
	if err := checkResult(code, msg); err != nil {
		return nil, err
	}
	peer := &Peer{handle: handle}
	runtime.SetFinalizer(peer, func(p *Peer) {
		_ = p.Close()
	})
	return peer, nil
}

// Close releases the listener handle. Close is idempotent.
func (l *Listener) Close() error {
	if l == nil {
		return nil
	}

	l.mu.Lock()
	defer l.mu.Unlock()
	if l.closed {
		return nil
	}
	if l.handle != nil {
		ffiListenerFree(l.handle)
		l.handle = nil
	}
	l.closed = true
	runtime.SetFinalizer(l, nil)
	return nil
}
