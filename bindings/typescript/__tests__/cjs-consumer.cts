// CJS package-specifier consumer fixture. Verifies that TypeScript Node16
// resolution against the package `exports` map selects the CommonJS declaration
// (index.d.cts) for require/import-from-CJS consumers.
import {
	Peer,
	Listener,
	SchemaRegistry,
	control,
	CONTROL,
	COMMAND,
	DATA,
	TELEMETRY,
	ERROR,
	type JsFrame,
	type AuthTokenResult,
} from "@3leaps/ipcprims";

export function surface(): number {
	const peer: Peer = Peer.connect("/tmp/ipcprims-cjs.sock", [
		CONTROL,
		COMMAND,
		DATA,
		TELEMETRY,
		ERROR,
	]);
	const listener: Listener = Listener.bind("/tmp/ipcprims-cjs.sock");
	const reg: SchemaRegistry = SchemaRegistry.fromDirectory("/tmp");
	const token: AuthTokenResult = peer.takeClientAuthToken();
	const frame: JsFrame = { channel: control(), payload: Buffer.from("x") };
	void frame;
	void listener;
	void reg;
	void token;
	return CONTROL + COMMAND + DATA + TELEMETRY + ERROR;
}
