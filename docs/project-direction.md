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

At the current stage, both YAML content and rendered architecture are
provisional placeholders. Their purpose is to establish stable contracts,
representative boundaries, and enough topology to develop the simulation
engine. Detailed configuration and architecture completion is deliberately
deferred while communication, network behavior, control integration, and
scenario execution are built.

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

## Current Frontend Contracts

The initial process view model is
[`web/src/generated/process-view.json`](../web/src/generated/process-view.json).
It defines:

- Ten ordered process areas with stable routes.
- Individual component records and roles.
- Upstream presentation relationships.
- Future canonical configuration references.
- A versioned schema marker.

This bootstrap establishes the process presentation contract. It is not
evidence that area connectivity, control-source references, I/O bindings,
simulation state, or scenario outcomes have been validated.

The appliance configuration catalog is
[`web/src/generated/appliance-configs.json`](../web/src/generated/appliance-configs.json).
Rust generates it from 160 parsed per-appliance and 205 per-connection YAML
files. It provides stable IDs, typed kind and behavior-family metadata,
resolved connection endpoints, lifecycle state, source revisions, full source
text, and environment-scoped render bindings. Svelte uses this derivative
catalog for appliance and connection inspection. A localhost Rust API accepts
revision-checked edits and regenerates the catalog only after whole-project
validation succeeds.

## Current Rust Foundation

The initial Rust workspace now contains:

- `hearthline-model` for stable identifiers, appliance kinds, network data, and
  process events.
- `hearthline-engine` for deterministic appliance behavior, process-component
  behavior, typed appliance and connection parsing, configuration validation,
  frontend projection, event scheduling, trace output, and drop reasons.
- `hearthline-api` for localhost-only validated and atomic configuration
  editing.
- `hearthline-cli` for behavior-catalog inspection, rendered-role coverage,
  configuration validation and generation, and a small forwarding
  demonstration.

The engine has unit-tested switching, static routing, PAT, static NAT,
stateful-policy, connector, DNS, service, web-gateway, controller-scan,
remote-I/O, field-device, and safety primitives. It is not yet constructed
from YAML and does not yet execute the complete rendered topology. The YAML
pipeline now cross-validates connection endpoints, appliance port hardware,
port state and settings, physical-media compatibility and capacity, and
point-to-point port ownership; simulator construction and end-to-end flow
evaluation remain pending.

## YAML Scope

Appliance schema `0.3.0` currently covers:

- One stable file and ID per appliance.
- Appliance kind, typed behavior family, placement, role, summary, lifecycle,
  and tags.
- Physical or logical render bindings.
- Ports with Rust-defined hardware capabilities, administrative and initial
  operational state, speed, duplex, MTU, logical mode, addresses, and VLAN
  lists.
- Family-specific baselines for links, switching, routing, NAT, firewalls,
  application gateways, service endpoints, wireless, monitoring, control
  compute, vPLCs, HMIs, remote I/O, field devices, and safety interfaces.

Connection schema `0.2.0` covers:

- One stable file and ID per connection.
- Two appliance/interface endpoints.
- Ethernet, wireless LAN, wide-area, field-I/O, virtual, mirror, and encrypted
  IP transports, plus an available analog-telephone transport type.
- Copper, fiber, radio, carrier, virtual, field-wiring, and telephone media.
- Capacity, fixed latency, deterministic loss, direction, connection
  operational state, lifecycle, and tags.
- Endpoint existence, appliance and port capability, medium compatibility,
  endpoint speed, medium capacity, duplicate-pair detection, and exclusive
  point-to-point physical ports.

Port hardware and physical media behavior are Rust types. Appliance YAML
configures a port; connection YAML selects a supported bearer and describes a
specific attachment. Rust combines both sides to determine initial link state,
effective MTU, negotiated duplex, serialization delay, and propagation delay.

The canonical model still needs complete typed coverage and cross-reference
validation for:

- Sites, zones, conduits, devices, and roles.
- Cross-connection VLAN, trunk, and routed-interface consistency.
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

The engine milestones are:

1. Maintain stable component, event, effect, and trace contracts.
2. Parse and validate YAML schemas.
3. Reject duplicate addresses, invalid prefixes, undefined references, VLAN
   mismatches, and missing interfaces.
4. Build the complete graph of interfaces, links, zones, and policy boundaries.
5. Evaluate complete declared flows using the existing component primitives.
6. Explain the selected path or exact denial reason.
7. Parse supported control sources and resolve symbols, tasks, tags, and I/O.
8. Emit stable JSON for the Svelte application.
9. Execute positive, negative, process, and fault scenarios in CI.

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
|   |-- appliances
|   |   |-- customer
|   |   |-- internet
|   |   |-- central-office
|   |   |-- factory
|   |   `-- shared
|   |-- connections
|   `-- ot
|       `-- process
|-- logic
|   |-- structured-text
|   `-- ladder
|-- crates
|   |-- hearthline-model
|   |-- hearthline-engine
|   |-- hearthline-plc-parser
|   |-- hearthline-api
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

The navigable Svelte and documentation baseline, initial Rust behavior
foundation, typed appliance and connection repositories, frontend projection,
and local validated editor are complete. The next engineering milestone is a
formal device-to-device communication contract executed through configured
ports and typed media. Complete topology construction and deeper network
cross-validation follow that contract; later steps remain unimplemented unless
stated otherwise in the repository-level README.

1. Maintain the navigable Svelte architecture and synchronized documentation.
2. Carry typed messages between configured device ports through the Rust media
   and connector layer with deterministic traces.
3. Validate the Customer LAN and Customer Edge as the first executable path.
4. Extend structural validation with address, VLAN, route, policy, and HA
   reference rules.
5. Translate remaining site and environment presentation data into canonical
   inputs.
6. Construct simulator components and connectors from parsed configuration.
7. Assemble routing, NAT, stateful-policy, and conduit primitives into complete
   configured topologies.
8. Extend versioned JSON generation to topology and scenario data.
9. Add positive and negative network scenarios.
10. Replace provisional configuration and architecture content with
    scenario-derived, cross-validated engineering definitions.
11. Select supported control-language editions and interchange formats.
12. Implement control parsing and I/O cross-reference validation.
13. Integrate virtual PLC execution with the Rust plant model.
14. Add process state, accelerated time, material tracking, and fault injection.
15. Add vendor-specific rendering only as a separate tested capability.
