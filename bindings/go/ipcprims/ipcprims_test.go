package ipcprims_test

import (
	"crypto/subtle"
	"errors"
	"fmt"
	"os"
	"runtime"
	"testing"
	"time"

	"github.com/3leaps/ipcprims/bindings/go/ipcprims"
)

func testSocketPath(t *testing.T, name string) string {
	t.Helper()
	if runtime.GOOS == "windows" {
		return `\\.\pipe\ipcprims-go-` + name
	}
	return fmt.Sprintf("/tmp/ipcp-go-%d-%d-%s.sock", os.Getpid(), time.Now().UnixNano(), name)
}

func TestInitAndCleanup(t *testing.T) {
	if err := ipcprims.Init(); err != nil {
		t.Fatalf("Init failed: %v", err)
	}
	ipcprims.Cleanup()
}

func TestConnectSendRecv(t *testing.T) {
	sock := testSocketPath(t, "connect-send-recv")
	listener, err := ipcprims.Listen(sock)
	if err != nil {
		t.Fatalf("Listen failed: %v", err)
	}
	defer func() {
		_ = listener.Close()
	}()

	errCh := make(chan error, 1)
	go func() {
		peer, acceptErr := listener.Accept()
		if acceptErr != nil {
			errCh <- fmt.Errorf("accept failed: %w", acceptErr)
			return
		}
		defer func() {
			_ = peer.Close()
		}()

		frame, recvErr := peer.RecvOn(ipcprims.COMMAND)
		if recvErr != nil {
			errCh <- fmt.Errorf("recv failed: %w", recvErr)
			return
		}
		if frame.Channel != ipcprims.COMMAND {
			errCh <- fmt.Errorf("unexpected channel %d", frame.Channel)
			return
		}
		if sendErr := peer.Send(ipcprims.COMMAND, frame.Payload); sendErr != nil {
			errCh <- fmt.Errorf("send failed: %w", sendErr)
			return
		}
		errCh <- nil
	}()

	client, err := ipcprims.Connect(sock, []uint16{ipcprims.COMMAND})
	if err != nil {
		t.Fatalf("Connect failed: %v", err)
	}
	defer func() {
		_ = client.Close()
	}()

	payload := []byte(`{"action":"ping"}`)
	if err := client.Send(ipcprims.COMMAND, payload); err != nil {
		t.Fatalf("Send failed: %v", err)
	}

	frame, err := client.RecvOn(ipcprims.COMMAND)
	if err != nil {
		t.Fatalf("RecvOn failed: %v", err)
	}
	if string(frame.Payload) != string(payload) {
		t.Fatalf("payload mismatch: got %q want %q", string(frame.Payload), string(payload))
	}

	select {
	case serverErr := <-errCh:
		if serverErr != nil {
			t.Fatal(serverErr)
		}
	case <-time.After(3 * time.Second):
		t.Fatal("server goroutine timed out")
	}
}

func TestConnectWithAuthTokenRetrieve(t *testing.T) {
	sock := testSocketPath(t, "auth-token")
	listener, err := ipcprims.Listen(sock)
	if err != nil {
		t.Fatalf("Listen failed: %v", err)
	}
	defer func() {
		_ = listener.Close()
	}()

	token := []byte{'t', 'o', 'k', 'e', 'n', 0, 0xff}
	errCh := make(chan error, 1)
	go func() {
		peer, acceptErr := listener.Accept()
		if acceptErr != nil {
			errCh <- fmt.Errorf("accept failed: %w", acceptErr)
			return
		}
		defer func() {
			_ = peer.Close()
		}()

		got, present, takeErr := peer.TakeClientAuthToken()
		if takeErr != nil {
			errCh <- fmt.Errorf("take auth token failed: %w", takeErr)
			return
		}
		if !present {
			errCh <- fmt.Errorf("expected auth token to be present")
			return
		}
		if len(got) != len(token) || subtle.ConstantTimeCompare(got, token) != 1 {
			errCh <- fmt.Errorf("auth token mismatch: got %v want %v", got, token)
			return
		}

		got, present, takeErr = peer.TakeClientAuthToken()
		if takeErr != nil {
			errCh <- fmt.Errorf("second take auth token failed: %w", takeErr)
			return
		}
		if present || got != nil {
			errCh <- fmt.Errorf("second take should report no token, got present=%v token=%v", present, got)
			return
		}
		errCh <- nil
	}()

	client, err := ipcprims.ConnectWithAuth(sock, []uint16{ipcprims.COMMAND}, token)
	if err != nil {
		t.Fatalf("ConnectWithAuth failed: %v", err)
	}
	_ = client.Close()

	select {
	case serverErr := <-errCh:
		if serverErr != nil {
			t.Fatal(serverErr)
		}
	case <-time.After(3 * time.Second):
		t.Fatal("server goroutine timed out")
	}
}

func TestConnectWithEmptyAuthTokenRejected(t *testing.T) {
	sock := testSocketPath(t, "empty-auth-token")
	_, err := ipcprims.ConnectWithAuth(sock, []uint16{ipcprims.COMMAND}, []byte{})
	if err == nil {
		t.Fatal("ConnectWithAuth unexpectedly accepted an empty auth token")
	}
	var ipcErr *ipcprims.Error
	if !errors.As(err, &ipcErr) || ipcErr.Code != ipcprims.ErrInvalidArgument {
		t.Fatalf("ConnectWithAuth error = %v, want ErrInvalidArgument", err)
	}
}

func ExamplePeer_TakeClientAuthToken() {
	var peer *ipcprims.Peer
	expected := []byte("expected-token-from-config")

	presented, present, err := peer.TakeClientAuthToken()
	if err != nil || !present || len(presented) == 0 || len(presented) != len(expected) ||
		subtle.ConstantTimeCompare(presented, expected) != 1 {
		_ = peer.Close()
		return
	}

	// Treat peer as authenticated only after the constant-time check succeeds.
}

func TestPing(t *testing.T) {
	sock := testSocketPath(t, "ping")
	listener, err := ipcprims.Listen(sock)
	if err != nil {
		t.Fatalf("Listen failed: %v", err)
	}
	defer func() {
		_ = listener.Close()
	}()

	go func() {
		peer, acceptErr := listener.Accept()
		if acceptErr != nil {
			return
		}
		defer func() {
			_ = peer.Close()
		}()
		_, _ = peer.Recv()
	}()

	client, err := ipcprims.Connect(sock, []uint16{ipcprims.COMMAND})
	if err != nil {
		t.Fatalf("Connect failed: %v", err)
	}
	defer func() {
		_ = client.Close()
	}()

	rtt, err := client.Ping()
	if err != nil {
		t.Fatalf("Ping failed: %v", err)
	}
	if rtt < 0 {
		t.Fatalf("unexpected negative RTT: %v", rtt)
	}
}

func TestPeerCloseIdempotent(t *testing.T) {
	var peer *ipcprims.Peer
	if err := peer.Close(); err != nil {
		t.Fatalf("first close failed: %v", err)
	}
	if err := peer.Close(); err != nil {
		t.Fatalf("second close failed: %v", err)
	}
}

func TestListenerCloseIdempotent(t *testing.T) {
	var listener *ipcprims.Listener
	if err := listener.Close(); err != nil {
		t.Fatalf("first close failed: %v", err)
	}
	if err := listener.Close(); err != nil {
		t.Fatalf("second close failed: %v", err)
	}
}

func TestLastErrorIsString(t *testing.T) {
	_ = ipcprims.LastError()
}
