# Role Catalog (ipcprims)

Agentic role prompts for AI agent sessions in this repository.

Roles extend [crucible baseline roles](https://crucible.3leaps.dev/catalog/roles/) with
ipcprims-specific scope, responsibilities, and validation requirements.

## Available Roles

| Role                                   | Slug           | Category   | Purpose                                             |
| -------------------------------------- | -------------- | ---------- | --------------------------------------------------- |
| [Development Lead](devlead.yaml)       | `devlead`      | agentic    | Core implementation, wire format, transport         |
| [Delivery Lead](deliverylead.yaml)     | `deliverylead` | governance | Readiness assessments, delivery coordination        |
| [Security Review](secrev.yaml)         | `secrev`       | review     | Security analysis, input validation, FFI safety     |
| [Quality Assurance](qa.yaml)           | `qa`           | review     | Testing, cross-platform coverage, stress tests      |
| [Release Engineering](releng.yaml)     | `releng`       | automation | Release coordination with CI/CD platform validation |
| [CI/CD Automation](cicd.yaml)          | `cicd`         | automation | Pipelines, runners, platform matrix                 |
| [Information Architect](infoarch.yaml) | `infoarch`     | agentic    | Documentation, wire format spec, standards          |

## Key Customizations for ipcprims

All roles include ipcprims-specific extensions:

### Wire Format Safety

Every role that touches frame codec or transport code references:

- Defensive parsing of untrusted peer input
- Length validation before buffer allocation
- Adversarial input testing (malformed frames, truncated data)

### Platform Matrix

Roles that involve builds or releases reference the supported platform set:

- Linux x64/arm64 (glibc + musl)
- macOS arm64
- Windows x64

## Usage

Reference roles in session prompts or AGENTS.md:

```yaml
roles:
  - slug: devlead
    source: config/agentic/roles/devlead.yaml
```

Or load directly in a session:

```
Role: devlead (config/agentic/roles/devlead.yaml)
```

## Role Selection Guide

| Task                   | Primary Role | May Escalate To                                      |
| ---------------------- | ------------ | ---------------------------------------------------- |
| Feature implementation | devlead      | secrev (security), qa (testing)                      |
| Bug fixes              | devlead      | qa (regression tests)                                |
| Security review        | secrev       | human maintainers (critical)                         |
| Test design            | qa           | devlead (implementation questions)                   |
| CI/CD changes          | cicd         | releng (release workflows), secrev (secrets)         |
| Release preparation    | releng       | cicd (workflow issues), human maintainers (approval) |
| Documentation          | infoarch     | devlead (technical accuracy)                         |

## Schema

Role files conform to the [role-prompt schema](https://schemas.3leaps.dev/agentic/v0/role-prompt.schema.json).
