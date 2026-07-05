import test from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { Worker } from "node:worker_threads";

interface NativePeer {
	send(channel: number, payload: Buffer): void;
	recv(): { channel: number; payload: Buffer };
	recvOn(channel: number): { channel: number; payload: Buffer };
	ping(): number;
	close(): void;
}

interface NativeListener {
	accept(): NativePeer;
	close(): void;
}

interface NativeAsyncPeer {
	send(channel: number, payload: Buffer): Promise<void>;
	recvAsync(options?: { signal?: AbortSignal }): Promise<{ channel: number; payload: Buffer }>;
	recvOnAsync(
		channel: number,
		options?: { signal?: AbortSignal },
	): Promise<{ channel: number; payload: Buffer }>;
	openChannel(channel: number): Promise<AsyncIterable<{ channel: number; payload: Buffer }> & {
		recvAsync(options?: { signal?: AbortSignal }): Promise<{ channel: number; payload: Buffer }>;
	}>;
	close(): void;
}

interface NativeAsyncListener {
	accept(): Promise<NativeAsyncPeer>;
	close(): Promise<void>;
}

// eslint-disable-next-line @typescript-eslint/no-var-requires
const ipcprims = require("../index.js") as {
	Listener: {
		bind(path: string, options?: { channels?: number[] }): NativeListener;
	};
	AsyncListener: {
		bind(path: string, options?: { channels?: number[] }): NativeAsyncListener;
	};
	Peer: { connect(path: string, channels: number[]): NativePeer };
	AsyncPeer: {
		connect(path: string, channels: number[]): Promise<NativeAsyncPeer>;
	};
	COMMAND: number;
	DATA: number;
};

function socketPath(tag: string): string {
	return path.join("/tmp", `ipcp-ts-${process.pid}-${Date.now()}-${tag}.sock`);
}

function startServer(socket: string, mode: "echo" | "ping") {
	let readyResolver: (() => void) | undefined;
	let doneResolver: (() => void) | undefined;
	let doneRejecter: ((error: Error) => void) | undefined;

	const ready = new Promise<void>((resolve) => {
		readyResolver = resolve;
	});
	const done = new Promise<void>((resolve, reject) => {
		doneResolver = resolve;
		doneRejecter = reject;
	});

	const worker = new Worker(
		`
      const { parentPort, workerData } = require('node:worker_threads')
      const ipcprims = require(workerData.modulePath)

      try {
        const listener = ipcprims.Listener.bind(workerData.socket, { channels: [ipcprims.COMMAND] })
        parentPort.postMessage({ type: 'ready' })

        const serverPeer = listener.accept()
        if (workerData.mode === 'echo') {
          const frame = serverPeer.recvOn(ipcprims.COMMAND)
          serverPeer.send(ipcprims.COMMAND, frame.payload)
        } else if (workerData.mode === 'ping') {
          try {
            serverPeer.recv()
          } catch (_) {
          }
        }

        serverPeer.close()
        listener.close()
        parentPort.postMessage({ type: 'done' })
      } catch (error) {
        parentPort.postMessage({ type: 'error', message: error instanceof Error ? error.message : String(error) })
      }
    `,
		{
			eval: true,
			workerData: {
				modulePath: path.resolve(__dirname, "..", "index.js"),
				socket,
				mode,
			},
		},
	);

	worker.on("message", (message: { type: string; message?: string }) => {
		if (message.type === "ready") {
			readyResolver?.();
			return;
		}

		if (message.type === "done") {
			doneResolver?.();
			return;
		}

		if (message.type === "error") {
			doneRejecter?.(new Error(message.message ?? "server worker failed"));
		}
	});
	worker.on("error", (error) => doneRejecter?.(error));
	worker.on("exit", (code) => {
		if (code !== 0) {
			doneRejecter?.(new Error(`server worker exited with code ${code}`));
		}
	});

	return { ready, done };
}

function startAsyncServer(
	socket: string,
	mode: "echo" | "echo-two" | "delayed" | "two" | "two-channels" | "idle",
) {
	let readyResolver: (() => void) | undefined;
	let doneResolver: (() => void) | undefined;
	let doneRejecter: ((error: Error) => void) | undefined;

	const ready = new Promise<void>((resolve) => {
		readyResolver = resolve;
	});
	const done = new Promise<void>((resolve, reject) => {
		doneResolver = resolve;
		doneRejecter = reject;
	});

	const worker = new Worker(
		`
      const { parentPort, workerData } = require('node:worker_threads')
      const ipcprims = require(workerData.modulePath)

      const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

      ;(async () => {
        const listener = ipcprims.AsyncListener.bind(workerData.socket, { channels: [ipcprims.COMMAND, ipcprims.DATA] })
        parentPort.postMessage({ type: 'ready' })
        const serverPeer = await listener.accept()

        if (workerData.mode === 'echo') {
          const frame = await serverPeer.recvOnAsync(ipcprims.COMMAND)
          await serverPeer.send(ipcprims.COMMAND, frame.payload)
        } else if (workerData.mode === 'echo-two') {
          for (let i = 0; i < 2; i += 1) {
            const frame = await serverPeer.recvOnAsync(ipcprims.COMMAND)
            await serverPeer.send(ipcprims.COMMAND, frame.payload)
          }
        } else if (workerData.mode === 'delayed') {
          await sleep(250)
          await serverPeer.send(ipcprims.COMMAND, Buffer.from('delayed'))
        } else if (workerData.mode === 'two') {
          await serverPeer.send(ipcprims.COMMAND, Buffer.from('one'))
          await serverPeer.send(ipcprims.COMMAND, Buffer.from('two'))
        } else if (workerData.mode === 'two-channels') {
          await sleep(100)
          await serverPeer.send(ipcprims.DATA, Buffer.from('data-first'))
          await serverPeer.send(ipcprims.COMMAND, Buffer.from('command-second'))
        } else if (workerData.mode === 'idle') {
          await sleep(350)
        }

        serverPeer.close()
        await listener.close()
        parentPort.postMessage({ type: 'done' })
      })().catch((error) => {
        parentPort.postMessage({ type: 'error', message: error instanceof Error ? error.message : String(error) })
      })
    `,
		{
			eval: true,
			workerData: {
				modulePath: path.resolve(__dirname, "..", "index.js"),
				socket,
				mode,
			},
		},
	);

	worker.on("message", (message: { type: string; message?: string }) => {
		if (message.type === "ready") {
			readyResolver?.();
			return;
		}

		if (message.type === "done") {
			doneResolver?.();
			return;
		}

		if (message.type === "error") {
			doneRejecter?.(new Error(message.message ?? "server worker failed"));
		}
	});
	worker.on("error", (error) => doneRejecter?.(error));
	worker.on("exit", (code) => {
		if (code !== 0) {
			doneRejecter?.(new Error(`server worker exited with code ${code}`));
		}
	});

	return { ready, done };
}

test("connect/send/recv roundtrip", async () => {
	const socket = socketPath("roundtrip");
	const server = startServer(socket, "echo");
	await server.ready;
	const client = ipcprims.Peer.connect(socket, [ipcprims.COMMAND]);
	const payload = Buffer.from('{"action":"ping"}');
	client.send(ipcprims.COMMAND, payload);
	const reply = client.recvOn(ipcprims.COMMAND);
	assert.equal(reply.channel, ipcprims.COMMAND);
	assert.equal(Buffer.compare(reply.payload, payload), 0);
	client.close();
	await server.done;
});

test("ping succeeds", async () => {
	const socket = socketPath("ping");
	const server = startServer(socket, "ping");
	await server.ready;
	const client = ipcprims.Peer.connect(socket, [ipcprims.COMMAND]);
	const rttMs = client.ping();
	assert.equal(typeof rttMs, "number");
	assert.ok(rttMs >= 0);
	client.close();
	await server.done;
});

test("async peer roundtrip preserves channels", async () => {
	const socket = socketPath("async-roundtrip");
	const server = startAsyncServer(socket, "echo");
	await server.ready;
	const client = await ipcprims.AsyncPeer.connect(socket, [ipcprims.COMMAND]);
	const payload = Buffer.from('{"action":"ping"}');
	await client.send(ipcprims.COMMAND, payload);
	const reply = await client.recvOnAsync(ipcprims.COMMAND);
	assert.equal(reply.channel, ipcprims.COMMAND);
	assert.equal(Buffer.compare(reply.payload, payload), 0);
	client.close();
	await server.done;
});

test("recvAsync does not block the event loop", async () => {
	const socket = socketPath("async-liveness");
	const server = startAsyncServer(socket, "delayed");
	await server.ready;
	const client = await ipcprims.AsyncPeer.connect(socket, [ipcprims.COMMAND]);

	let timerFired = false;
	const timer = new Promise<void>((resolve) => {
		setTimeout(() => {
			timerFired = true;
			resolve();
		}, 100);
	});
	const pending = client.recvAsync();
	await timer;
	assert.equal(timerFired, true);
	const reply = await pending;
	assert.equal(reply.payload.toString(), "delayed");
	client.close();
	await server.done;
});

test("recvAsync observes AbortSignal without closing future receives", async () => {
	const socket = socketPath("async-abort");
	const server = startAsyncServer(socket, "delayed");
	await server.ready;
	const client = await ipcprims.AsyncPeer.connect(socket, [ipcprims.COMMAND]);

	const alreadyAborted = new AbortController();
	alreadyAborted.abort();
	await assert.rejects(() => client.recvAsync({ signal: alreadyAborted.signal }), /receive aborted|Abort/);

	const controller = new AbortController();
	const pending = client.recvAsync({ signal: controller.signal });
	controller.abort();
	await assert.rejects(() => pending, /receive aborted|Abort/);

	const reply = await client.recvAsync();
	assert.equal(reply.payload.toString(), "delayed");
	client.close();
	await server.done;
});

test("AbortSignal keeps existing onabort handler", async () => {
	const socket = socketPath("async-abort-handler");
	const server = startAsyncServer(socket, "idle");
	await server.ready;
	const client = await ipcprims.AsyncPeer.connect(socket, [ipcprims.COMMAND]);

	const controller = new AbortController();
	let handlerCalled = false;
	const handler = () => {
		handlerCalled = true;
	};
	controller.signal.onabort = handler;

	const pending = client.recvAsync({ signal: controller.signal });
	assert.equal(controller.signal.onabort, handler);
	controller.abort();
	await assert.rejects(() => pending, /receive aborted|Abort/);
	assert.equal(handlerCalled, true);
	client.close();
	await server.done;
});

test("AbortSignal listener is removed after successful receive", async () => {
	const socket = socketPath("async-abort-cleanup");
	const server = startAsyncServer(socket, "echo-two");
	await server.ready;
	const client = await ipcprims.AsyncPeer.connect(socket, [ipcprims.COMMAND]);

	const controller = new AbortController();
	const signal = controller.signal;
	const addEventListener = signal.addEventListener.bind(signal);
	const removeEventListener = signal.removeEventListener.bind(signal);
	let activeAbortListeners = 0;
	signal.addEventListener = ((
		type: Parameters<AbortSignal["addEventListener"]>[0],
		listener: Parameters<AbortSignal["addEventListener"]>[1],
		options?: Parameters<AbortSignal["addEventListener"]>[2],
	) => {
		if (type === "abort") {
			activeAbortListeners += 1;
		}
		return addEventListener(type, listener, options);
	}) as AbortSignal["addEventListener"];
	signal.removeEventListener = ((
		type: Parameters<AbortSignal["removeEventListener"]>[0],
		listener: Parameters<AbortSignal["removeEventListener"]>[1],
		options?: Parameters<AbortSignal["removeEventListener"]>[2],
	) => {
		if (type === "abort") {
			activeAbortListeners -= 1;
		}
		return removeEventListener(type, listener, options);
	}) as AbortSignal["removeEventListener"];

	await client.send(ipcprims.COMMAND, Buffer.from("one"));
	const first = await client.recvAsync({ signal });
	assert.equal(first.payload.toString(), "one");
	assert.equal(activeAbortListeners, 0);

	await client.send(ipcprims.COMMAND, Buffer.from("two"));
	const second = await client.recvAsync({ signal });
	assert.equal(second.payload.toString(), "two");
	assert.equal(activeAbortListeners, 0);

	controller.abort();
	assert.equal(activeAbortListeners, 0);
	client.close();
	await server.done;
});

test("async channel receiver iterates FIFO for a channel", async () => {
	const socket = socketPath("async-iterator");
	const server = startAsyncServer(socket, "two");
	await server.ready;
	const client = await ipcprims.AsyncPeer.connect(socket, [ipcprims.COMMAND]);
	const receiver = await client.openChannel(ipcprims.COMMAND);
	const iterator = receiver[Symbol.asyncIterator]();

	const first = await iterator.next();
	const second = await iterator.next();
	assert.equal(first.value.payload.toString(), "one");
	assert.equal(second.value.payload.toString(), "two");
	client.close();
	await server.done;
});

test("async send can run while recvAsync is pending", async () => {
	const socket = socketPath("async-concurrent");
	const server = startAsyncServer(socket, "echo");
	await server.ready;
	const client = await ipcprims.AsyncPeer.connect(socket, [ipcprims.COMMAND]);
	const pending = client.recvOnAsync(ipcprims.COMMAND);
	await client.send(ipcprims.COMMAND, Buffer.from("concurrent"));
	const reply = await pending;
	assert.equal(reply.payload.toString(), "concurrent");
	client.close();
	await server.done;
});

test("concurrent recvOnAsync calls do not starve other channels", async () => {
	const socket = socketPath("async-concurrent-channels");
	const server = startAsyncServer(socket, "two-channels");
	await server.ready;
	const client = await ipcprims.AsyncPeer.connect(socket, [ipcprims.COMMAND, ipcprims.DATA]);

	const command = client.recvOnAsync(ipcprims.COMMAND);
	const data = client.recvOnAsync(ipcprims.DATA);

	const dataReply = await data;
	const commandReply = await command;
	assert.equal(dataReply.channel, ipcprims.DATA);
	assert.equal(dataReply.payload.toString(), "data-first");
	assert.equal(commandReply.channel, ipcprims.COMMAND);
	assert.equal(commandReply.payload.toString(), "command-second");
	client.close();
	await server.done;
});

test("close cancels a pending recvAsync without throwing", async () => {
	const socket = socketPath("async-close-pending");
	const server = startAsyncServer(socket, "idle");
	await server.ready;
	const client = await ipcprims.AsyncPeer.connect(socket, [ipcprims.COMMAND]);

	const pending = client.recvAsync();
	assert.doesNotThrow(() => client.close());
	await assert.rejects(() => pending, /cancelled|closed|receive|async/);
	await server.done;
});

test("close disconnects the remote async peer", async () => {
	const socket = socketPath("async-close-remote");
	const listener = ipcprims.AsyncListener.bind(socket, { channels: [ipcprims.COMMAND] });
	const accepted = listener.accept();
	const client = await ipcprims.AsyncPeer.connect(socket, [ipcprims.COMMAND]);
	const server = await accepted;

	const pending = server.recvAsync();
	client.close();
	await assert.rejects(() => pending, /closed|cancelled|receive|async|connection/i);
	server.close();
	await listener.close();
});

test("listener close cancels a pending accept", async () => {
	const socket = socketPath("async-listener-close");
	const listener = ipcprims.AsyncListener.bind(socket, { channels: [ipcprims.COMMAND] });

	const pending = assert.rejects(listener.accept(), /listener is closed|closed/);
	await listener.close();
	await pending;
});
