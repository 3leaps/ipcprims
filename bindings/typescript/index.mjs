// ESM façade over the CommonJS binding wrapper.
//
// The wrapper (index.js) owns the single native-addon instance and
// loadBinding()/platform-selection logic. This file imports it exactly once
// and re-exports its values as explicit bindings, so CJS and ESM consumers in
// one process share one native instance and identical class/prototype
// identities. It must not call loadBinding() or load a native addon itself.
import cjs from "./index.js";

export const {
	AsyncChannelReceiver,
	AsyncListener,
	AsyncPeer,
	Listener,
	Peer,
	SchemaRegistry,
	control,
	command,
	data,
	telemetry,
	error,
	CONTROL,
	COMMAND,
	DATA,
	TELEMETRY,
	ERROR,
} = cjs;
