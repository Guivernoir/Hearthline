# Hearthline Implementation Direction

## Decision

**Status:** Accepted  
**Date:** 2026-07-26

Hearthline separates declarative configuration, industrial control sources,
validated behavior, and human-readable presentation.

| Component | Responsibility | Authoritative for |
| --- | --- | --- |
| YAML | Declarative source of truth | Inventory, interfaces, addressing, routes, services, policies, scenarios, program assignment, tags, and I/O mapping |
| IEC 61131-3 sources | Industrial control source of truth | Structured Text and Ladder Diagram programs |
| Rust | Parsing, validation, and behavior engine | Network checks, graph construction, policy evaluation, control cross-references, process behavior, and scenario results |
| Virtual PLC runtime | Control execution | Scan cycles, task scheduling, timers, function blocks, and selected language semantics |
| Svelte | Static architecture application | Navigation, topology, inspection, and visualization of validated data |

## Engineering Rationale

The initial tool evaluation did not identify a single simulator that supported
Hearthline's combined requirements for hierarchical physical and logical
architecture, vendor-neutral desired state, explained network and security
decisions, IEC 61131-3 source mapping, virtual controller execution, and a
ceramics process model.

The project therefore uses a purpose-specific integration architecture. It does
not attempt to reproduce every packet, proprietary controller feature, or
vendor engineering workflow. Existing network emulators, vendor tools, virtual
PLC runtimes, and hardware-in-the-loop laboratories remain external validation
targets where their fidelity is required.

This decision increases the verification burden. A feature is not considered
implemented merely because the Svelte application can draw it or YAML can
describe it. Claims require parsed input, deterministic evaluation, diagnostics,
repeatable scenarios, and tests at the appropriate abstraction level.

## Source-of-Truth Rules

1. YAML defines intended network and controller configuration.
2. IEC 61131-3 files define control-program logic.
3. Rust owns parsing, interpretation, validation, and behavioral evaluation.
4. The virtual PLC runtime owns execution semantics for the selected control
   dialect.
5. Svelte consumes generated data and owns presentation coordinates and
   interaction state only.
6. Markdown explains architecture, decisions, and operational intent without
   duplicating every configuration field.
7. Generated JSON is derivative and cannot override YAML or control sources.

YAML represents production-shaped desired state. It is not deployable
vendor-specific configuration until a renderer, target schema, secret
injection, and output tests exist for that platform.

## Site Authority

The Central Office is the primary governance and analysis site. Operations
Intelligence owns architecture standards, identity and policy services, NOC and
SOC workflows, production-data analysis, and approved change staging.

The Factory is the execution and enforcement site. It contains the OT DMZ,
independent IT-side and OT-side policy boundaries, Level 3 operations, local
engineering resources, the redundant vPLC compute cluster, distributed I/O,
and the process areas.

The inter-site conduit carries explicitly approved administrative and data
workflows. It does not create direct Central Office routes to controllers.
Loss of the conduit may delay analytics and remote administration, but it must
not stop safe local process operation.

## Data Flow

```text
Canonical YAML ------------+
                           |
Structured Text -----------+--> Rust parsers and validated models
                           |          |
Ladder / PLCopen XML ------+          +--> topology and addressing validation
                                      +--> routing, NAT, and policy evaluation
                                      +--> program, task, tag, and I/O analysis
                                      +--> network and process scenarios
                                                |
                                                v
                                         Generated JSON
                                                |
                                                v
                                  Static Svelte architecture application

Structured Text / Ladder --> virtual PLC runtime --> simulated I/O
                                                       |
                                                       v
                                              Rust plant model
```

The Svelte application may filter, select, highlight paths, and inspect
scenario results. It must not become a second routing, policy, PLC, or process
engine.

## Current Bootstrap Contract

The initial process view model is
[`web/src/generated/process-view.json`](../web/src/generated/process-view.json).
It defines:

- Ten ordered process areas with stable routes.
- Individual component records and roles.
- Upstream presentation relationships.
- Future canonical configuration references.
- A versioned schema marker.

This bootstrap establishes the frontend consumer contract. It is not evidence
that YAML generation or Rust validation exists. Inventory, connectivity,
control-source references, I/O bindings, simulation state, and scenario
outcomes will move to Rust-generated data.

## YAML Scope

The canonical model will cover:

- Sites, zones, conduits, devices, and roles.
- Physical interfaces, links, VLANs, trunks, and routed interfaces.
- IPv4 and future IPv6 addressing.
- Routing and NAT intent.
- Services, service groups, and stateful policy.
- Management-plane access.
- OT requirements by source, destination, protocol, direction, purpose, owner,
  and availability.
- Identity, device assurance, policy decision, and session attributes.
- Expected-success and expected-denial scenarios.
- Controllers, programs, tasks, tags, and simulated I/O mappings.
- Physical control-compute hosts, logical vPLC instances, cell network
  assignments, and distributed-I/O stations.

Credentials, private keys, and reusable shared secrets are referenced by
identifier and remain outside version control.

## Industrial Control Scope

- Structured Text is stored as reviewable `.st` source.
- Ladder Diagram uses one declared machine-readable interchange format;
  PLCopen XML is the initial vendor-neutral candidate.
- YAML assigns programs and tasks to controllers and maps symbolic variables to
  simulated or physical I/O.
- Rust preserves source locations, resolves symbols and program organization
  units, and reports unsupported extensions explicitly.
- Executable scenarios declare language edition, dialect, task timing, data
  types, scan behavior, and I/O update order.

Parsing does not prove that a program compiles or behaves identically on a real
controller. Functional-safety and burner-management claims require separate,
qualified engineering and validation.

## Rust Scope

The first engine milestones are:

1. Parse and validate YAML schemas.
2. Reject duplicate addresses, invalid prefixes, undefined references, VLAN
   mismatches, and missing interfaces.
3. Build a directed graph of interfaces, links, zones, and policy boundaries.
4. Evaluate routing, NAT, and stateful policy for declared flows.
5. Explain the selected path or exact denial reason.
6. Parse supported control sources and resolve symbols, tasks, tags, and I/O.
7. Emit stable JSON for the Svelte application.
8. Execute positive, negative, process, and fault scenarios in CI.

The reference deployment keeps physical vPLC hosts in factory-local Level 3
control compute. Area-specific network separation continues from each cell to
its assigned runtime interface. Distributed I/O remains inside the automation
cell, and sensors and actuators terminate there rather than directly on a
software workload.

The initial engine is an intent evaluator and process model, not a bit-level
packet emulator or substitute for qualified hardware testing.

## Repository Direction

```text
.
|-- config
|   |-- inventory.yaml
|   |-- topology.yaml
|   |-- devices
|   |-- policies
|   `-- scenarios
|-- logic
|   |-- structured-text
|   `-- ladder
|-- crates
|   |-- hearthline-model
|   |-- hearthline-engine
|   |-- hearthline-plc-parser
|   `-- hearthline-cli
|-- web
|   `-- Svelte static application
|-- docs
|   |-- customer-network
|   |-- central-office
|   |-- factory
|   `-- project-direction.md
`-- README.md
```

## Delivery Sequence

The initial navigable Svelte and documentation baseline is complete. The next
engineering milestone is the canonical YAML schema; later steps remain
unimplemented unless stated otherwise in the repository-level README.

1. Maintain the navigable Svelte architecture and synchronized documentation.
2. Define stable YAML identifiers, schemas, and reference rules.
3. Translate remaining frontend inventory into canonical YAML.
4. Implement Rust structural and reference validation.
5. Add routing, NAT, stateful-policy, and conduit evaluation.
6. Generate versioned JSON view models.
7. Add positive and negative network scenarios.
8. Select supported control-language editions and interchange formats.
9. Implement control parsing and I/O cross-reference validation.
10. Integrate virtual PLC execution with the Rust plant model.
11. Add process state, accelerated time, material tracking, and fault injection.
12. Add vendor-specific rendering only as a separate tested capability.
