// ESM package-specifier consumer fixture. Verifies that TypeScript Node16
// resolution against the package `exports` map selects the ESM declaration
// (index.d.mts) and that the full named-export surface type-checks, including
// the four type-only interfaces.
import {
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
	type JsFrame,
	type RecvAsyncOptions,
	type ListenerOptions,
	type AuthTokenResult,
} from "@3leaps/ipcprims";

const controller = new AbortController();
const signal: RecvAsyncOptions = { signal: controller.signal };

export async function surface(): Promise<number> {
	const listener: AsyncListener = AsyncListener.bind("/tmp/ipcprims-esm.sock", {
		channels: [COMMAND],
	} satisfies ListenerOptions);
	const accepted: Promise<AsyncPeer> = listener.accept();
	const client: AsyncPeer = await AsyncPeer.connect("/tmp/ipcprims-esm.sock", [
		CONTROL,
		COMMAND,
		DATA,
		TELEMETRY,
		ERROR,
	]);
	const receiver: AsyncChannelReceiver = await client.openChannel(COMMAND);
	const frame: JsFrame = { channel: control(), payload: Buffer.from("x") };
	await client.send(frame.channel, frame.payload);
	const up: number = control() + command() + data() + telemetry() + error();
	const syncPeer: Peer = Peer.connect("/tmp/ipcprims-esm.sock", [CONTROL]);
	const syncListener: Listener = Listener.bind("/tmp/ipcprims-esm.sock");
	const reg: SchemaRegistry = SchemaRegistry.fromDirectory("/tmp");
	const token: AuthTokenResult = syncPeer.takeClientAuthToken();
	for await (const received of receiver) {
		void received;
	}
	void frame;
	void up;
	void syncListener;
	void receiver;
	void signal;
	void reg;
	void token;
	await client.shutdown();
	return 0;
}
