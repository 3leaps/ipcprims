# ipcprims

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust: 1.81+](https://img.shields.io/badge/rust-1.81%2B-orange.svg)](https://www.rust-lang.org/)

**Reliable inter-process communication with permissive licensing.**

ipcprims provides permissively licensed, cross-platform IPC primitives — named pipes, Unix sockets, message framing, schema validation, and channel multiplexing — that can be statically or dynamically linked into your applications. When your services need structured, validated communication over local transports and you want a lightweight alternative to full RPC frameworks, ipcprims offers a focused solution.

**Lifecycle Phase**: `alpha` | See [RELEASE_NOTES.md](RELEASE_NOTES.md) for current version

## The Problem

You're building software where multiple processes need to communicate locally — an agent talks to adapters, a service talks to sidecars, a plugin host talks to plugins. Your options:

1. **Pull in gRPC/tonic** — Heavy dependency tree, designed for network RPC not local IPC, brings protobuf compilation into your build
2. **Raw Unix sockets** — No message framing, no validation, every project reinvents length-prefixed reads
3. **Shell out to socat/netcat** — No structured messaging, not embeddable as a library
4. **Message brokers (ZeroMQ, Redis)** — External service dependency for what should be in-process communication
5. **Roll your own** — Every team writes the same buggy length-prefix parser, the same reconnection logic, the same "how do I multiplex channels" code

## What ipcprims Offers

- **Permissively licensed**: MIT/Apache-2.0 dual licensed. Link statically or dynamically with no additional obligations.
- **Framed-by-default**: Every message is length-prefixed with type tags. No partial reads, no buffer management in user code.
- **Schema-validated (opt-in)**: Validate messages against JSON Schema 2020-12 at the transport boundary. Catch contract violations before they become bugs.
- **Multiplexed channels**: Separate command and data streams over a single transport. No need for multiple sockets per peer.
- **Cross-platform**: Unix domain sockets on Linux/macOS, named pipes on Windows, with a unified API.
- **Sync-first, async-planned**: Blocking sync API in v0.1.0. Tokio-native async API planned for v0.2.0 behind `async` feature flag.
- **Library-first**: Embed directly in Rust, Go, Python, or TypeScript. CLI is a diagnostic/demo tool.

### Framed-by-Default: The Core Difference

The fundamental improvement over raw sockets and pipes isn't just convenience — it's _correctness_.

**The problem with raw IPC:**

```
Process A writes 4096 bytes to socket
Process B reads... 2048 bytes (partial read)
Process B parses incomplete JSON -> crash or silent corruption
Process A writes another message
Process B reads remainder of first + start of second -> garbled
```

**ipcprims behavior:**

```
Process A frames message: [4-byte length][2-byte channel][payload]
Process B reads frame header -> knows exact payload size
Process B reads exactly that many bytes -> complete message guaranteed
Channel tag routes to correct handler (command vs. data)
Optional: schema validation rejects malformed payloads at the boundary
```

## Who Should Use This

**Platform Engineers**: You need IPC primitives in your service mesh with a clean, permissive dependency tree. ipcprims gives you framed, multiplexed communication that links into anything.

**Library Authors**: You're building something where processes need to talk to each other. Depend on ipcprims instead of rolling your own length-prefix parser.

**Enterprise Teams**: Your supply chain policy requires clear licensing. ipcprims is MIT/Apache-2.0 throughout — straightforward for compliance review.

**OSS Projects**: You want unambiguous licensing that works for all contributors. MIT/Apache-2.0 dual license keeps things simple.

## Quick Start

### As a Rust Library

```toml
[dependencies]
ipcprims = "0.1"
```

```rust
use ipcprims::transport::UnixDomainSocket;
use ipcprims::frame::{encode_frame, decode_frame, Frame, COMMAND, DEFAULT_MAX_PAYLOAD};
use bytes::BytesMut;
use std::io::{Read, Write};

// Server side
let listener = UnixDomainSocket::bind("/tmp/my-service.sock")?;
let mut peer = listener.accept()?;

// Client side (in another process)
let mut client = UnixDomainSocket::connect("/tmp/my-service.sock")?;

// Send a framed message on the COMMAND channel
let mut buf = BytesMut::new();
encode_frame(COMMAND, b"{\"action\":\"ping\"}", &mut buf);
client.write_all(&buf)?;
```

### As a CLI

```bash
# Start a listener (useful for testing peer services)
ipcprims listen /tmp/my-service.sock

# Connect and send (useful for debugging)
ipcprims send /tmp/my-service.sock --channel 1 --json '{"action":"ping"}'

# Echo server (useful for integration testing)
ipcprims echo /tmp/test.sock

# Version and build info
ipcprims version --extended
```

### CLI Dogfooding (End-to-End)

```bash
# Run a full local CLI behavior matrix
make dogfood-cli
```

This exercises `echo`, `send`, `listen`, `info`, `doctor`, and `envinfo` with schema-validation and timeout scenarios. See `docs/guides/cli-dogfooding.md`.

### Exit Codes

| Condition                | Exit Code |
| ------------------------ | --------- |
| Success                  | 0         |
| Connection refused       | 1         |
| Transport error          | 3         |
| Permission denied        | 50        |
| Schema validation failed | 60        |
| Invalid arguments        | 64        |
| Timeout                  | 124       |
| ipcprims itself failed   | 125       |

## Modules

### ipcprims-transport

Cross-platform transport abstraction. Unix domain sockets on Linux/macOS, named pipes on Windows.

### ipcprims-frame

The core value-add. Length-prefixed message framing with channel multiplexing.

**Wire format:**

```
+----------+---------+---------+-----------------+
| Magic 2B | Len 4B  | Chan 2B | Payload         |
| "IP"     | (LE)    | (LE)    | (Len bytes)     |
+----------+---------+---------+-----------------+
```

**Built-in channels:**

| ID   | Name      | Purpose                                                |
| ---- | --------- | ------------------------------------------------------ |
| 0    | CONTROL   | Connection management (handshake, ping/pong, shutdown) |
| 1    | COMMAND   | Structured commands (request/response)                 |
| 2    | DATA      | Bulk data transfer                                     |
| 3    | TELEMETRY | Metrics, logs, health signals                          |
| 4    | ERROR     | Error notifications                                    |
| 256+ | User      | Application-defined channels                           |

### ipcprims-schema

Optional JSON Schema 2020-12 validation at the transport boundary. Behind the `schema` feature flag.

### ipcprims-peer

High-level peer connection management with handshake, health tracking, and request/response patterns.

### ipcprims-ffi

C-ABI bindings scaffold for peer-level APIs, enabling Go/TypeScript/Python bindings to link against
`cdylib`/`staticlib` artifacts.

### Go bindings

Go bindings are provided in-module at `bindings/go/ipcprims` with cgo linkage to `ipcprims-ffi`.
The module follows sibling-repo layout conventions with `include/` (generated header) and
`lib/` (static archives; `lib/local/<platform>/` for local development sync).

### TypeScript bindings

TypeScript bindings scaffold is provided at `bindings/typescript` using Node-API (`ipcprims-napi`).

## Platform Support

| Platform            | Target                       | Transport        | Status    |
| ------------------- | ---------------------------- | ---------------- | --------- |
| Linux x64 (glibc)   | `x86_64-unknown-linux-gnu`   | UDS (abstract)   | Primary   |
| Linux x64 (musl)    | `x86_64-unknown-linux-musl`  | UDS (abstract)   | Primary   |
| Linux arm64 (glibc) | `aarch64-unknown-linux-gnu`  | UDS (abstract)   | Primary   |
| Linux arm64 (musl)  | `aarch64-unknown-linux-musl` | UDS (abstract)   | Primary   |
| macOS arm64         | `aarch64-apple-darwin`       | UDS (filesystem) | Supported |
| Windows x64         | `x86_64-pc-windows-msvc`     | Named pipes      | Supported |

## Development

```bash
# Build
cargo build

# Test
cargo test

# Full quality check
make check
```

### Quality Gates

- `cargo fmt --check` — zero diff
- `cargo clippy -- -Dwarnings` — zero warnings
- `cargo test` — all tests pass
- `cargo deny check` — all dependencies permissively licensed

## Supply Chain

ipcprims is designed for environments where dependency hygiene matters:

- **License-clean**: All dependencies use MIT, Apache-2.0, or compatible licenses
- **Auditable**: Run `cargo tree` to inspect the full dependency graph
- **SBOM-ready**: Compatible with `cargo sbom`
- **No runtime network calls**: All functionality is local

```bash
# Check dependencies
cargo deny check licenses

# Audit for vulnerabilities
cargo audit
```

## Prior Art

ipcprims builds on ideas from others in this space:

- **[interprocess](https://crates.io/crates/interprocess)** — Good cross-platform IPC crate. We build our own thinner wrappers for full control over framing behavior.
- **[tonic](https://crates.io/crates/tonic)** — Excellent gRPC framework. Overkill for local IPC where you don't need HTTP/2 or protobuf.
- **[tarpc](https://crates.io/crates/tarpc)** — Clean RPC framework. Too opinionated for a primitives library.

We're not claiming to replace these projects. ipcprims fills a specific niche: embeddable IPC primitives with framing built in, permissive licensing, and first-class bindings.

## Ecosystem

ipcprims is the third member of the 3leaps prims family:

| Library                                        | Scope                       | Tagline                                                          |
| ---------------------------------------------- | --------------------------- | ---------------------------------------------------------------- |
| [sysprims](https://github.com/3leaps/sysprims) | System/OS operations        | "Reliable process control with permissive licensing"             |
| [docprims](https://github.com/3leaps/docprims) | Document format handling    | "Reliable document extraction with permissive licensing"         |
| **ipcprims**                                   | Inter-process communication | "Reliable inter-process communication with permissive licensing" |

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Apache-2.0 provides explicit patent grants, which may be valuable for enterprise adoption.

Subject to [3 Leaps OSS policies](https://github.com/3leaps/oss-policies).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines and [MAINTAINERS.md](MAINTAINERS.md) for governance.

---

<div align="center">

**Built by the [3 Leaps](https://3leaps.net) team**

Part of the [Fulmen Ecosystem](https://github.com/fulmenhq)

</div>
