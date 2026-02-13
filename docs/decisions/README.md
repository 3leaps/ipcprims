# Decision Records — ipcprims

Architecture (ADR), Design (DDR), and Security (SDR) decision records for ipcprims.

## Index

| ID       | Type     | Title                                                                                       | Status   | Date       |
| -------- | -------- | ------------------------------------------------------------------------------------------- | -------- | ---------- |
| DDR-0001 | Design   | [Transport Scope: IPC-First, Extensible](DDR-0001-transport-scope.md)                       | Accepted | 2026-02-06 |
| DDR-0002 | Design   | [CLI Design Precepts](DDR-0002-cli-design-precepts.md)                                      | Accepted | 2026-02-06 |
| SDR-0001 | Security | [Schema Validation at IPC Boundary](SDR-0001-schema-validation-scope.md)                    | Accepted | 2026-02-06 |
| SDR-0002 | Security | [Peer and Transport Hardening Defaults](SDR-0002-peer-transport-hardening-defaults.md)      | Accepted | 2026-02-08 |
| SDR-0003 | Security | [Schema Registry Hardening Boundaries](SDR-0003-schema-registry-hardening-boundaries.md)    | Accepted | 2026-02-08 |
| SDR-0004 | Security | [auth_token and Peer Credentials Boundary](SDR-0004-auth-token-and-credentials-boundary.md) | Accepted | 2026-02-09 |
| SDR-0005 | Security | [Ordering and Replay Boundary](SDR-0005-ordering-and-replay-boundary.md)                    | Accepted | 2026-02-09 |

## Record Types

- **ADR** (Architecture Decision Record): Structural choices affecting system boundaries, crate organization, or integration patterns
- **DDR** (Design Decision Record): API design, conventions, and implementation approach choices
- **SDR** (Security Decision Record): Security-relevant design choices, threat mitigations, trust boundaries

## Conventions

- Records are numbered sequentially within each type
- Status values: `Proposed`, `Accepted`, `Superseded`, `Deprecated`
- Once accepted, records are not modified — supersede with a new record instead
- Context section describes the forces at play; Decision section states the choice; Consequences section captures trade-offs
