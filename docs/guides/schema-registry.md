# Schema Registry Guide

`ipcprims-schema` validates JSON payloads by channel at the IPC boundary. It
uses JSON Schema 2020-12 and is optional: applications that do not attach a
registry perform no schema validation.

## Start With The Default

`RegistryConfig::default()` is deliberately permissive:

| Setting                      | Default   | Effect                                                                  |
| ---------------------------- | --------- | ----------------------------------------------------------------------- |
| `strict_mode`                | `false`   | Schemas are compiled without the strict transform.                      |
| `fail_on_missing_schema`     | `false`   | An unregistered channel succeeds without validation.                    |
| `max_schemas_from_directory` | `256`     | Directory loading fails when this many recognized schemas are exceeded. |
| `max_schema_file_size`       | `256 KiB` | Each recognized schema has this maximum size.                           |

With the default configuration, a channel without a registered schema receives
**no validation at all**. `SchemaRegistry::validate` returns success for that
channel without parsing the payload as JSON. This fail-open default is a
compatibility contract. Use both `strict_mode` and `fail_on_missing_schema`
when an application requires the stricter path.

## Setup And Registration

Add the schema crate directly when validation is needed:

```toml
[dependencies]
ipcprims-schema = "0.2.4"
```

Create a registry, then register JSON Schema text or a parsed
`serde_json::Value`. A successful registration for an existing channel replaces
that channel's validator.

```rust
use ipcprims_schema::{RegistryConfig, SchemaError, SchemaRegistry};

let mut registry = SchemaRegistry::with_config(RegistryConfig {
    strict_mode: true,
    fail_on_missing_schema: true,
    ..RegistryConfig::default()
});

registry.register(
    1,
    r#"{
        "type": "object",
        "properties": { "id": { "type": "integer" } },
        "required": ["id"]
    }"#,
)?;

registry.validate(1, br#"{"id": 7}"#)?;
assert!(registry.validate(1, br#"{"id": 7, "extra": true}"#).is_err());

// Missing schemas fail before this non-JSON payload is parsed.
assert!(matches!(
    registry.validate(2, b"not json"),
    Err(SchemaError::NoSchema(2))
));
# Ok::<(), SchemaError>(())
```

Use `SchemaRegistry::from_embedded` for a fixed list of schema strings. It
always uses the default configuration. Use `with_config` followed by `register`,
or `from_directory_with_config`, when embedded or loaded schemas need the
stricter path.

`validate_frame` validates an `ipcprims_frame::Frame`; `has_schema`,
`channels`, and `config` expose the current registry state.

## Strict Mode And Missing Schemas

`strict_mode` is a registration-time schema transform, not a universal
deny-unknown guarantee. For recognized object-like schema locations, it inserts
`"additionalProperties": false` only when that keyword is absent. An explicit
`additionalProperties` value, including `true` or a schema, is preserved.

An object-like schema is recognized by `type: "object"`, a type array containing
`"object"`, or, when `type` is absent, an object keyword such as `properties`,
`patternProperties`, `additionalProperties`, `unevaluatedProperties`,
`required`, `dependentRequired`, `dependentSchemas`, or `propertyNames`. An
explicit non-object `type` takes precedence over the object-keyword fallback.
The transform recurses through the supported structural keywords: `properties`,
`patternProperties`, `dependentSchemas`, `$defs`, `definitions`,
`propertyNames`, `additionalProperties`, `unevaluatedProperties`, `items`,
`contains`, `additionalItems`, `unevaluatedItems`, `not`, `if`, `then`, `else`,
`prefixItems`, `allOf`, `anyOf`, and `oneOf`.

This is best-effort hardening of those recognized locations. It does not make
every JSON Schema construct deny unknown properties. Pair it with
`fail_on_missing_schema: true` when unregistered channels must be rejected.

For a registered channel, invalid JSON returns `SchemaError::InvalidJson`; a
payload that fails its schema returns `SchemaError::ValidationFailed`. Invalid
schema JSON returns `InvalidJson`, while schema compilation failures return
`CompileFailed`.

## Loading A Directory

`SchemaRegistry::from_directory` uses defaults.
`SchemaRegistry::from_directory_with_config` applies an explicit configuration.
Use canonical lowercase filenames:

| Filename                  |                                   Channel |
| ------------------------- | ----------------------------------------: |
| `control.schema.json`     |                                         0 |
| `command.schema.json`     |                                         1 |
| `data.schema.json`        |                                         2 |
| `telemetry.schema.json`   |                                         3 |
| `error.schema.json`       |                                         4 |
| `channel_<N>.schema.json` | Numeric channel `N` (`0` through `65535`) |

The loader ignores ordinary non-schema files and non-file entries. A canonical
lowercase schema-file symlink is rejected. A filename ending in `.schema.json`
that does not map to a supported channel is a load error rather than an ignored
file.

Use the canonical lowercase names exactly. Channel resolution normalizes ASCII
case, but the current symlink rejection checks only the literal lowercase
`.schema.json` suffix. A symlink with a differently cased suffix, such as
`command.SCHEMA.JSON`, is skipped rather than rejected; with the default
fail-open configuration, the missing channel then receives no validation.
Treat non-canonical filename case as unsupported for security-sensitive schema
directories.

Recognized schema files are subject to the configured count limit and size
limit. The loader checks the opened file's reported length and also reads at
most one byte beyond the configured size, so a file that grows while being read
cannot bypass the bound.

Avoid duplicate aliases for a channel. For example, `command.schema.json` and
`channel_1.schema.json` map to the same channel. The loader replaces an earlier
validator with a later successful registration, and directory enumeration order
is unspecified. A permissive schema can therefore nondeterministically replace
a stricter schema. Keep exactly one schema filename per channel.

### File Identity By Platform

| Platform | Protection during schema loading                                                                                        | Residual limitation                                                                                                                                                                    |
| -------- | ----------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Unix     | Canonical lowercase schema symlinks are rejected and path metadata is compared with the opened file using `(dev, ino)`. | A path-to-open replacement is detected before the file is read. Symlinks with a differently cased suffix are skipped rather than rejected.                                             |
| Windows  | Canonical lowercase schema symlinks are rejected and the count/size-bounded read protections apply.                     | Opened-file identity is not yet compared. A local file swap between path metadata and open is not detected; symlinks with a differently cased suffix are skipped rather than rejected. |

The accepted Windows file-identity design is not yet implemented. Do not rely
on directory loading on Windows to provide the Unix opened-file identity check.

## Attach To Rust Peers

Enable the peer crate's `schema` feature to attach a shared
`Arc<SchemaRegistry>` to synchronous or asynchronous listeners. Peers accepted
from that listener validate public sends and delivered receives with the shared
registry. Outbound connectors accept the same registry through their explicit
configuration functions.

```toml
[dependencies]
ipcprims-peer = { version = "0.2.4", features = ["schema"] }
ipcprims-schema = "0.2.4"
```

Add the `async` feature when using `AsyncPeerListener` or
`async_connect_with_config`:

```toml
ipcprims-peer = { version = "0.2.4", features = ["schema", "async"] }
```

Create the registry with `Arc::new`, then pass a clone to
`PeerListener::with_schema_registry` or
`AsyncPeerListener::with_schema_registry`. For outbound connections, pass
`Some(registry)` to `connect_with_config` or `async_connect_with_config` after
the handshake configuration argument. The registry is an optional peer feature;
without one, peer traffic has no schema-validation overhead.

## CLI Validation

`ipcprims echo --validate <directory>` loads the directory with
`strict_mode: true` and `fail_on_missing_schema: false`. It applies the strict
transform to loaded schemas, but it is **not** the full deny-on-missing path:
unregistered channels still pass without validation. Schema failures received by
the echo server trigger a best-effort `ERROR`-channel response while the server
continues to run; it logs a warning if that response cannot be sent.

## TypeScript Binding

The TypeScript binding supports standalone directory loading and validation:

```ts
import { COMMAND, SchemaRegistry } from "@3leaps/ipcprims";

const registry = SchemaRegistry.fromDirectory("/opt/example/schemas");
registry.validate(COMMAND, Buffer.from('{"id":7}'));
registry.close();
```

`close()` is safe to call more than once. After it closes the registry,
`validate()` fails because the registry is no longer available.

`Listener.bind` and `AsyncListener.bind` also accept
`ListenerOptions.schemaDir`, which loads and attaches a separate registry for
that listener. The standalone registry cannot be attached to a TypeScript peer
or listener. All three TypeScript surfaces use `RegistryConfig::default()`:
TypeScript currently cannot enable `strict_mode` or `fail_on_missing_schema`,
and it cannot register schemas programmatically. Do not assume TypeScript
directory validation rejects unregistered channels.

See the [TypeScript binding README](https://github.com/3leaps/ipcprims/blob/main/bindings/typescript/README.md)
for listener examples.

## Decisions

[SDR-0001](https://github.com/3leaps/ipcprims/blob/main/docs/decisions/SDR-0001-schema-validation-scope.md)
describes the validation boundary, and
[SDR-0003](https://github.com/3leaps/ipcprims/blob/main/docs/decisions/SDR-0003-schema-registry-hardening-boundaries.md)
describes the hardening direction. The implementation and this guide are the
current contract where historical decision text differs from the shipped API or
platform support.
