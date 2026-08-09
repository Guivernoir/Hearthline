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
[`packages/web/src/generated/process-view.json`](../../../packages/web/src/generated/process-view.json).
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
[`packages/web/src/generated/appliance-configs.json`](../../../packages/web/src/generated/appliance-configs.json).
Rust generates it from 185 parsed per-appliance and 231 per-connection YAML
files. It provides stable IDs, typed kind and behavior-family metadata,
resolved connection endpoints, lifecycle state, source revisions, full source
text, and environment-scoped render bindings. Svelte uses this derivative
catalog for appliance and connection inspection. A localhost Rust API accepts
revision-checked edits and regenerates the catalog only after whole-project
validation succeeds.

## Current Rust Foundation

The initial Rust workspace now contains:

- `hearthline-model` for stable identifiers, appliance kinds, network data, and
  process events without `std` or heap allocation.
- `hearthline-engine` for allocator-free deterministic appliance behavior,
  process-component behavior, event scheduling, trace output, and drop reasons.
- `hearthline-config` for host-side appliance and connection parsing,
  filesystem repositories, cross-file validation, and frontend projection.
- `hearthline-api` for localhost-only validated and atomic configuration
  editing plus configured scenario catalog and execution.
- `hearthline-cli` for behavior-catalog inspection, rendered-role coverage,
  configuration validation and generation, a hand-built forwarding
  demonstration, and versioned YAML-built scenarios.

External integration suites cover switching, static routing, PAT, stateful
policy, connectors, DNS, services, web gateways, controller scans, media
compatibility, and safety behavior. Selected endpoint, switch, router, NAT
router, stateful firewall, DNS, web-server, web-gateway, HMI, virtual-PLC,
remote-I/O, field-device, safety-interface, and link components are now
constructed from YAML. Thirty versioned end-to-end scenarios cover
independent customer public paths, Business IT PC-01 through PC-04 internal DNS
and HTTPS, approved or denied factory operations-data transfer, Forming
controller-to-Level-3 collection, Level-3-to-DMZ replication, three
WAF-prevented path-traversal, disallowed-method, and SQL-injection request-body
exercises, a customer access-circuit outage with a declared restored-state
DNS-delivery expectation, a Business IT core recovery that transfers three
VRRP groups and Rust-computed Rapid-PVST forwarding roles to Core-02, and a
converged northbound-firewall recovery that transfers active ownership to the
validated standby, plus a protocol-timed variant that carries one session and
heartbeats over the HA medium before validating a reverse ACK. Rust also
executes four bounded fault variants: an HA-medium outage after successful
state replication, standby session-state loss that must fail closed after
promotion, retained state that expires after the modeled TCP idle timer, and
sync-path isolation that fences the standby while peer failure is unconfirmed.
A composite scenario drops both factory-facing conduit handoffs while an
independent Body Preparation HMI, vPLC, remote-I/O, safety, and pump path
remains operational; passing requires the expected historian-path failure and
the configured local pump command. Rust exposes each trace through the local API and
Svelte simulation workspace; security evidence can also
enter a bounded Central SOC session. The engine does not yet execute the
complete rendered topology. The YAML pipeline cross-validates connection
endpoints, appliance port hardware, port state and settings, physical-media
compatibility and capacity, point-to-point port ownership, and each selected
scenario path; broader policy, service, controller-program, and process-state
scenarios remain pending.

## YAML Scope

Appliance schema `0.10.0` currently covers:

- One stable file and ID per appliance.
- Appliance kind, typed behavior family, placement, role, summary, lifecycle,
  and tags.
- Physical or logical render bindings.
- Ports with Rust-defined hardware capabilities, administrative and initial
  operational state, speed, duplex, MTU, logical mode, addresses, and VLAN
  lists.
- Routed-interface first-hop groups with virtual IP and MAC identity, member
  priority, preemption intent, and an initial active or standby role.
- Optional Rapid-PVST bridge protocol, standard bridge priority, and unique
  bridge MAC identity for Layer 2 and Layer 3 switches.
- Optional LACP system identity, local groups, shared logical bundle IDs,
  active or passive mode, minimum links, and member interfaces.
- Optional reciprocal multi-chassis domain, primary or secondary role, and
  peer-link interface.
- Optional reciprocal stateful-firewall domain, active or standby role,
  monitored virtual interfaces, session-sync intent, dedicated sync port, and
  validated heartbeat and failure-hold timers.
- Family-specific baselines for links, switching, routing, NAT, firewalls,
  application gateways, service endpoints, wireless, monitoring, control
  compute, vPLCs, HMIs/SCADA with optional signal scope, remote I/O, field
  devices, and safety interfaces.

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
|-- packages
|   |-- crates
|   |   |-- hearthline-model
|   |   |-- hearthline-engine
|   |   |-- hearthline-config
|   |   |-- hearthline-api
|   |   `-- hearthline-cli
|   |-- fuzz
|   `-- web
|-- project
|   |-- control
|   |-- config
|   |-- docs
|   |-- scripts
|   `-- standards
`-- README.md
```

## Delivery Sequence

The navigable Svelte and documentation baseline, initial Rust behavior
foundation, typed appliance and connection repositories, frontend projection,
local validated editor, media-transit contract, Customer DNS path, customer
public-service delivery/denial pair, Business IT internal-service paths, and
operations-data delivery/denial pair are complete. The bounded factory
conduit-outage/local-command autonomy proof is also complete. Customer PC-01/02 and
Business IT PC-01/02/03/04 are enterable and invoke independent Rust-backed
paths from terminals and browsers. Their browsers render configured content
returned through the selected public or internal path. Customer endpoints can
also resolve a target and run bounded repeated ICMP probes through a compatible
configured route template, with YAML-controlled echo response and full media
traces. A deterministic API-session cache now retains successful DNS answers
for 60 seconds per workstation across browser, `curl`, `ping`, and SSH actions;
`nslookup` remains an explicit DNS-server query. This is the first retained
endpoint resolver state. A compatible scenario-session runner now follows it:
each workstation owns a union baseline topology with monotonic simulator time,
retained endpoint ARP and customer PAT state, and action-relative reports.
Controlled resilience contracts remain isolated by design. Broader inspection,
mutation, and formal contracts for switch, router, firewall, and connector
session state remain planned. Complete topology
construction and deeper network cross-validation follow; later steps remain
unimplemented unless stated otherwise in the repository-level README.

All ten process areas have configured operator sessions. Forming includes one
cell-wide SCADA scope and four module-local HMI scopes. Rust validates operator
permission and sends each accepted command through the operator interface,
vPLC, remote I/O, and actuator primitives while preserving safety, alarm,
actuator, and audit state. Forming additionally gives its five operator
interfaces one shared deterministic process. A bounded Structured Text source
owns normal sequence steps and requested outputs through an explicit YAML I/O
map; Rust advances plant measurements and independently raises modeled trips
across 20-millisecond scans.
The authorized Forming SCADA scope can capture that shared state into a typed,
bounded telemetry packet and execute the existing brokered historian-replica
path to Central Office analytics. The returned scenario report keeps the live
process sequence, payload, network policy result, media timing, and delivery
trace in one operator workflow.
This is not a general IEC 61131-3 runtime and does not provide a
production-fidelity material, pressure, drying, or robot model.

1. Maintain the navigable Svelte architecture and synchronized documentation.
2. Refine the implemented Forming I/O and control-program contract with
   reviewed process conditions and parameters.
3. Add further exact Phase 1 failover, isolation, and outage scenarios.
4. Extend structural validation with address, VLAN, route, policy, and HA
   reference rules.
5. Translate remaining site and environment presentation data into canonical
   inputs.
6. Extend parsed component construction to behavior families not yet supported.
7. Assemble routing, NAT, stateful-policy, and conduit primitives into broader
   configured topologies.
8. Extend versioned JSON generation to topology and scenario data.
9. Add positive and negative network scenarios.
10. Replace provisional configuration and architecture content with
    scenario-derived, cross-validated engineering definitions.
11. Select formal control-language compatibility targets and Ladder
    interchange formats before broadening the implemented Forming subset.
12. Extend control parsing and I/O cross-reference validation to selected
    constructs and additional process areas.
13. Extend source-driven virtual PLC execution beyond Forming.
14. Extend process state, accelerated time, material tracking, and fault
    injection beyond the first Forming implementation.
15. Add vendor-specific rendering only as a separate tested capability.
