# Hearthline Documentation

The documentation follows the same hierarchy as the Svelte application. Each
folder represents a documented application view and contains a local
`README.md`, a physical `screenshot.png`, and a logical
`logical-screenshot.png`.

![Hearthline regional architecture](screenshot.png)

![Hearthline regional logical architecture](logical-screenshot.png)

## Current Coverage

The documentation currently covers all 22 documented application views with
matching physical and logical captures. It documents the rendered Svelte model,
the parsed appliance and connection YAML baseline, the bootstrap process
contract, and target engineering requirements. Captures were revalidated
against release `0.2.0` on 2026-07-29. It does not document passing network,
control, process, or failover tests because those complete configured engines
and scenarios do not yet exist.

The documented architecture is also a provisional working model. Screenshots
record the current application faithfully, but they do not freeze topology,
equipment selection, placement, addressing, or policy as final design
decisions. Those details remain planned refinement after the behavioral
contracts can test them.

```text
project/docs
|-- customer-network
|   |-- customer-lan
|   |-- customer-edge
|   `-- public-web-path
|-- central-office
|   |-- it-dmz
|   |-- business-it
|   `-- operations-intelligence
`-- factory
    |-- ot-dmz
    `-- process
        |-- material
        |-- thermal
        `-- finishing
```

## Sites

| Site | Scope |
| --- | --- |
| [Customer Network](customer-network/README.md) | Private LAN, customer edge, and public web access |
| [Central Office](central-office/README.md) | Public services, enterprise IT, governance, security operations, and analytics |
| [Factory](factory/README.md) | Factory OT DMZ and the segmented ceramics process |

## Project-Wide Decisions

- [Implementation direction](reference/project-direction.md)
- [Deployment conformance review](reference/deployment-conformance.md)
- [Rust simulation engine](reference/simulation-engine.md)
- [Svelte architecture application](reference/svelte-application.md)
- [Configuration model](../config/README.md)
- [Continuous integration policy](../standards/CI_POLICY.md)
- [Changelog](../../CHANGELOG.md)
- [Versioning and releases](reference/versioning.md)

Network and process details belong at their lowest owning level. Parent pages
describe scope, authority, and relationships without duplicating child
inventories.

## Status Language

- **Implemented** describes behavior that can be exercised in the current
  Svelte application or build pipeline.
- **Bootstrap** describes representative data used to establish a view or
  interface contract before authoritative generation exists.
- **Provisional baseline** describes structurally valid configuration or
  architecture placeholders that are intentionally unfinished and expected to
  change as executable requirements mature.
- **Planned** describes work for which no executable implementation currently
  exists.
- **Validation target** describes an expected result that the future Rust
  engine must prove; it is not a passing test today.

Paired devices express a redundancy requirement or logical role. They do not,
by themselves, prove independent failure domains, synchronized state, or tested
failover.

Future documentation will be generated or cross-checked against canonical YAML,
Rust diagnostics, control-source references, and reproducible scenario
results. Screenshots and architecture text must be updated together whenever a
rendered route changes.
