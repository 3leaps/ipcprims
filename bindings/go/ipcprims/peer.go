package ipcprims

import (
	"errors"
	"runtime"
	"sync"
	"time"
	"unsafe"
)

var errPeerClosed = errors.New("ipcprims: peer is closed")

// Peer wraps an FFI peer handle.
type Peer struct {
	mu     sync.Mutex
	handle unsafe.Pointer
	closed bool
}

// Connect dials a listener path and negotiates channels.
func Connect(path string, channels []uint16) (*Peer, error) {
	handle, code, msg := ffiConnect(path, channels)
	if err := checkResult(code, msg); err != nil {
		return nil, err
	}

	peer := &Peer{handle: handle}
	runtime.SetFinalizer(peer, func(p *Peer) {
		_ = p.Close()
	})
	return peer, nil
}

// Send transmits payload bytes on a channel.
func (p *Peer) Send(channel uint16, data []byte) error {
	handle, err := p.getHandle()
	if err != nil {
		return err
	}

	code, msg := ffiPeerSend(handle, channel, data)
	return checkResult(code, msg)
}

// Recv blocks until a non-control frame is received.
func (p *Peer) Recv() (*Frame, error) {
	handle, err := p.getHandle()
	if err != nil {
		return nil, err
	}

	code, msg, channel, payload := ffiPeerRecv(handle)
	if err := checkResult(code, msg); err != nil {
		return nil, err
	}
	return &Frame{Channel: channel, Payload: payload}, nil
}

// RecvOn blocks until a frame for channel is received.
func (p *Peer) RecvOn(channel uint16) (*Frame, error) {
	handle, err := p.getHandle()
	if err != nil {
		return nil, err
	}

	code, msg, recvChannel, payload := ffiPeerRecvOn(handle, channel)
	if err := checkResult(code, msg); err != nil {
		return nil, err
	}
	return &Frame{Channel: recvChannel, Payload: payload}, nil
}

// Ping sends ping and waits for pong, returning RTT.
func (p *Peer) Ping() (time.Duration, error) {
	handle, err := p.getHandle()
	if err != nil {
		return 0, err
	}

	code, msg, rttNs := ffiPeerPing(handle)
	if err := checkResult(code, msg); err != nil {
		return 0, err
	}
	return time.Duration(rttNs), nil
}

// Shutdown performs graceful peer shutdown.
func (p *Peer) Shutdown() error {
	p.mu.Lock()
	if p.closed || p.handle == nil {
		p.mu.Unlock()
		return errPeerClosed
	}
	handle := p.handle
	p.handle = nil
	p.closed = true
	p.mu.Unlock()

	defer runtime.SetFinalizer(p, nil)
	code, msg := ffiPeerShutdown(handle)
	return checkResult(code, msg)
}

// Close releases the peer handle. Close is idempotent.
func (p *Peer) Close() error {
	if p == nil {
		return nil
	}

	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil
	}
	if p.handle != nil {
		ffiPeerFree(p.handle)
		p.handle = nil
	}
	p.closed = true
	runtime.SetFinalizer(p, nil)
	return nil
}

func (p *Peer) getHandle() (unsafe.Pointer, error) {
	if p == nil {
		return nil, errPeerClosed
	}

	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed || p.handle == nil {
		return nil, errPeerClosed
	}
	return p.handle, nil
}
