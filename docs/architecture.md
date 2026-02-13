# ipcprims Architecture

## Context

ipcprims provides framed IPC primitives for applications where multiple processes need reliable, structured communication over local transports. It is the IPC substrate for the Lanyte autonomous AI agent platform and a general-purpose library for any multi-process system.

```
┌─────────────────────────────────────────────────┐
│              Application Layer                   │
│   (Lanyte Gateway, your service, CLI tools)      │
├─────────────────────────────────────────────────┤
│              ipcprims-peer                       │
│   Peer · PeerListener · connect() · handshake    │
│   CONTROL protocol: ping/pong, shutdown          │
├─────────────────────────────────────────────────┤
│   ipcprims-schema          ipcprims-frame        │
│   SchemaRegistry           FrameReader<T>        │
│   Validation               FrameWriter<T>        │
│   (opt-in)                 Wire codec            │
├─────────────────────────────────────────────────┤
│              ipcprims-transport                   │
│   IpcStream · UnixDomainSocket · (Named Pipes)   │
│   (future: TcpTransport behind feature flag)     │
├─────────────────────────────────────────────────┤
│              Operating System                     │
│   Unix domain sockets · Windows named pipes      │
└─────────────────────────────────────────────────┘
```

## Crate Map

| Crate                | Purpose                                                  | Transport-dependent?                  |
| -------------------- | -------------------------------------------------------- | ------------------------------------- |
| `ipcprims-transport` | Stream abstraction over OS IPC mechanisms                | Yes — this IS the transport           |
| `ipcprims-frame`     | Wire format codec, FrameReader/Writer, channel constants | No — generic over `Read + Write`      |
| `ipcprims-schema`    | JSON Schema 2020-12 validation keyed by channel ID       | No                                    |
| `ipcprims-peer`      | Peer connection handle, handshake, CONTROL protocol      | No — generic over frame reader/writer |
| `ipcprims`           | Umbrella re-exports + CLI binary                         | Depends on all above                  |

## Wire Format

Frozen for v0.1.0:

```
┌──────────────┬───────────┬──────────┬─────────────────┐
│ Magic (2B)   │ Length    │ Channel  │ Payload          │
│ 0x49 0x50    │ (4B LE)  │ (2B LE)  │ (Length bytes)   │
│ "IP"         │          │          │                  │
└──────────────┴───────────┴──────────┴─────────────────┘
Header: 8 bytes. Max payload: 16 MiB (configurable).
```

## Channel Model

| Range       | Purpose                                                 |
| ----------- | ------------------------------------------------------- |
| 0 (CONTROL) | Handshake, ping/pong, shutdown — protocol-level         |
| 1–4         | Built-in: COMMAND, DATA, TELEMETRY, ERROR               |
| 5–255       | Reserved for future ipcprims use                        |
| 256+        | Application-defined (e.g., Lanyte: 256=MAIL, 257=PROXY) |

## Transport Extensibility

The framing layer (`FrameReader<T: Read>`, `FrameWriter<T: Write>`) works with any byte stream. The transport crate provides IPC-specific bindings. Additional transports (TCP, TCP+TLS) are planned for v0.2.0 behind feature flags. See [DDR-0001](decisions/DDR-0001-transport-scope.md).

## Security Considerations

ipcprims provides **mechanisms**, not **policy**:

- **Schema validation**: Configurable strictness. Consumers like Lanyte enable strict mode (`deny_unknown_fields`, fail on missing schema).
- **Peer identity**: Exposes OS-level credentials (SO_PEERCRED) and optional auth token in handshake. Policy enforcement is the consumer's responsibility.
- **Observability**: Emits `tracing` events on send/recv. Consumers attach subscribers for audit logging.

See [SDR-0001](decisions/SDR-0001-schema-validation-scope.md).

## Decision Records

See [docs/decisions/](decisions/README.md) for architecture, design, and security decisions.
