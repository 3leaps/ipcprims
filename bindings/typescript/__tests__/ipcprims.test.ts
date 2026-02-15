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

// eslint-disable-next-line @typescript-eslint/no-var-requires
const ipcprims = require("../index.js") as {
	Listener: {
		bind(path: string, options?: { channels?: number[] }): NativeListener;
	};
	Peer: { connect(path: string, channels: number[]): NativePeer };
	COMMAND: number;
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
