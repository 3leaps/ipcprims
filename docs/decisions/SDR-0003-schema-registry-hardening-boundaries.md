# SDR-0003: Schema Registry Hardening Boundaries

**Status**: Accepted
**Date**: 2026-02-08
**Deciders**: Architecture Council

## Context

Phase 2 security review (D2-03 punchlist) identified follow-on schema-boundary
concerns after D2-01:

- strict mode could miss object-like schemas that omit explicit `type: object`
- directory schema loading had no explicit count/size bounds
- schema loading behavior needed clearer fail-closed handling for symlinked
  schema files

The schema registry may be used at trust boundaries where schemas and payloads
are externally influenced. Defensive limits should be explicit and default-on.

## Decision

1. Strict mode object detection is expanded.

`strict_mode` treats schemas as object-like when object keywords are present,
even if `type: object` is omitted.

Object-like keywords include:

- `properties`, `patternProperties`, `additionalProperties`,
  `unevaluatedProperties`, `required`, `dependentRequired`,
  `dependentSchemas`, `propertyNames`

2. Directory loading is bounded by default.

`RegistryConfig` includes:

- `max_schemas_from_directory` (default: 256)
- `max_schema_file_size` (default: 256 KiB)

Loading fails when either bound is exceeded.

3. Symlinked schema files are rejected.

For `*.schema.json` paths, symbolic links are treated as load errors rather
than silently followed or skipped.

4. File identity checks are required during schema load.

The loader verifies that path metadata and opened-handle metadata refer to the
same file identity before reading schema content:

- Unix: `(dev, ino)`
- Windows: `(volume serial, file index)`

## Consequences

**Positive:**

- Closes strict-mode bypass class for object-keyword schemas.
- Reduces risk of schema-loading memory/CPU abuse through oversized or massive
  schema sets.
- Makes schema load behavior more deterministic and fail-closed.

**Trade-offs:**

- Existing deployments with very large schema files or very large schema sets
  may need to increase limits explicitly.
- Symlink-based schema indirection is no longer accepted for schema filenames.

## References

- D2-03 punchlist: P2-B, SEC-006
- `crates/ipcprims-schema/src/config.rs`
- `crates/ipcprims-schema/src/registry.rs`
