# @3leaps/ipcprims

TypeScript bindings for ipcprims using Node-API (NAPI-RS).

## Async Peer

Use `AsyncPeer` and `AsyncListener` when the process must keep its event loop live while waiting for IPC frames. The sync `Peer.recv()` and `Peer.recvOn()` APIs are still available for short scripts, but they block the JavaScript thread while waiting.

```ts
import { AsyncListener, AsyncPeer, COMMAND } from "@3leaps/ipcprims";

const listener = AsyncListener.bind("/tmp/ipcprims.sock", {
	channels: [COMMAND],
});

const accepted = listener.accept();
const client = await AsyncPeer.connect("/tmp/ipcprims.sock", [COMMAND]);
const server = await accepted;

const controller = new AbortController();
const pending = client.recvAsync({ signal: controller.signal });

await server.send(COMMAND, Buffer.from("hello"));
const frame = await pending;
console.log(frame.channel, frame.payload.toString());

client.close();
server.close();
await listener.close();
```

`recvAsync()` receives frames in arrival order. `recvOnAsync(channel)` and `openChannel(channel)` receive FIFO frames for that channel; they do not claim global ordering across channels.

The TypeScript async wrapper owns a small dispatcher above Rust `AsyncPeer` so concurrent JavaScript receive calls can wait independently. That dispatcher buffers at most 256 frames or 16 MiB while routing frames to waiters; Rust `AsyncPeer` transport, channel negotiation, CONTROL handling, schema validation, and bounded receive behavior still remain the underlying safety boundary.

```ts
const receiver = await client.openChannel(COMMAND);

for await (const frame of receiver) {
	console.log(frame.payload.toString());
}
```

Passing an already-aborted or later-aborted `AbortSignal` rejects the pending receive without closing the peer, so a later `recvAsync()` or `recvOnAsync()` can still consume future frames.
