package ipcprims

// Frame is a received message with channel metadata.
type Frame struct {
	Channel uint16
	Payload []byte
}
