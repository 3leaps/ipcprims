import { AsyncListener, AsyncPeer, COMMAND, type JsFrame } from "../index";

async function readmeControlLoopShape(socket: string): Promise<JsFrame> {
	const listener = AsyncListener.bind(socket, { channels: [COMMAND] });
	const accepted = listener.accept();
	const client = await AsyncPeer.connect(socket, [COMMAND]);
	const server = await accepted;
	const receiver = await client.openChannel(COMMAND);

	for await (const frame of receiver) {
		client.close();
		server.close();
		await listener.close();
		return frame;
	}

	throw new Error("receiver ended unexpectedly");
}

void readmeControlLoopShape;
