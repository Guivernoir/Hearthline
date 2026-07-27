# Hearthline

Hearthline is an industrial architecture and simulation project intended to
connect a public customer journey, enterprise services, governed IT/OT
exchange, and a segmented ceramics process. Its current implementation is a
navigable Svelte architecture application. Declarative YAML configuration,
Rust validation and simulation, and IEC 61131-3 control execution are planned
engineering layers, not current capabilities.

![Hearthline regional architecture](docs/screenshot.png)

![Hearthline regional logical architecture](docs/logical-screenshot.png)

## Project Goals

Hearthline is designed to:

- Progressively model the architecture through corresponding physical and
  logical views.
- Keep network inventory, addressing, interfaces, policy, and scenarios in
  reviewable YAML.
- Validate topology, routing, NAT, segmentation, and permitted conduits in
  Rust.
- Associate controllers with Structured Text or Ladder Diagram programs,
  tasks, tags, and simulated I/O.
- Run virtual PLC logic against a Rust process model.
- Explain successful and denied communication paths in the Svelte interface.
- Model safe local factory operation without depending on Central Office
  availability.

## Why a Custom Codebase

The project began with existing network and industrial simulation tools, but
the initial evaluation did not identify one simulator that covered the combined
requirements: hierarchical physical and logical navigation, vendor-neutral
configuration, explainable network and security decisions, virtual PLC
execution, IEC 61131-3 source integration, process simulation, fault scenarios,
and generated documentation.

Hearthline is therefore being developed as a purpose-specific codebase instead
of treating several disconnected tools as one authoritative model. This is a
scope decision, not a claim that the project already replaces network
emulators, vendor engineering environments, virtual PLC products, or hardware
integration laboratories. Those tools remain necessary for implementation and
acceptance testing.

Building the missing integration also creates substantial engineering
obligations. Parser correctness, timing behavior, protocol fidelity, failure
modes, safety boundaries, and generated results must be tested before they can
be trusted. Hearthline only claims behavior that has an implemented and
repeatable validation path.

## Current State

The following capabilities are implemented in the Svelte application:

- A regional map containing the Customer Network, Central Office, and Factory.
- Physical and logical representations at every documented level.
- Drill-down navigation from sites to environments and from the Factory process
  to ten individual production areas.
- Selectable architecture nodes, device inspectors, zoom, pan, fit, reset,
  grid, and minimap controls.
- Customer LAN, customer edge, public-service, enterprise, DMZ, operations,
  analytics, and factory security views.
- A ten-stage ceramics process with individual controllers, HMIs, sensors,
  distributed I/O, actuators, and safety or permissive interfaces.
- A bootstrap process view model loaded from
  [`process-view.json`](web/src/generated/process-view.json).

Current maturity is:

| Layer | Status |
| --- | --- |
| Svelte architecture application | Implemented and buildable |
| Physical and logical documentation captures | Implemented for every documented route |
| Process view-model contract | Bootstrap JSON, schema `0.2.0` |
| Canonical YAML configuration | Planned; no authoritative YAML records exist yet |
| Rust validation and simulation | Planned; no Rust workspace exists yet |
| IEC 61131-3 sources and vPLC execution | Planned; no control sources or runtime integration exist yet |
| Deployment or standards conformance | Not claimed |

Until generated data replaces the remaining frontend datasets, those datasets
describe architecture and presentation intent rather than validated
configuration or simulated behavior.

## Target Architecture

| Layer | Responsibility |
| --- | --- |
| YAML | Canonical inventory, interfaces, addressing, routes, services, policies, I/O assignments, and scenarios |
| IEC 61131-3 | Structured Text and machine-readable Ladder Diagram control sources |
| Rust | Schema validation, graph construction, connectivity and policy evaluation, process behavior, fault injection, and generated view data |
| Virtual PLC runtime | PLC scan cycles, task scheduling, timers, function blocks, and execution of the selected control dialect |
| Svelte | Static architecture presentation, navigation, inspection, filtering, and visualization of validated results |

```text
YAML configuration --------+
                           |
Structured Text -----------+--> Rust models and validation
                           |          |
Ladder / PLCopen XML ------+          +--> connectivity and policy results
                                      +--> control and I/O cross-references
                                      +--> process and fault scenarios
                                                |
                                                v
                                         Generated JSON
                                                |
                                                v
                                      Svelte application

Control sources --> virtual PLC runtime --> simulated I/O --> Rust plant model
```

Svelte owns presentation and interaction. It does not make routing, policy,
identity, control, or process decisions.

## Sites

| Site | Scope | Documentation |
| --- | --- | --- |
| Customer Network | Residential LAN, customer edge, and end-to-end public web access | [Customer Network](docs/customer-network/README.md) |
| Central Office | Public IT DMZ, Business IT, governance, monitoring, analytics, and approved change workflows | [Central Office](docs/central-office/README.md) |
| Factory | Factory-local OT DMZ, Level 3 handoff, and segmented ceramics process | [Factory](docs/factory/README.md) |

The Central Office is the principal governance and analysis site. The Factory
retains local execution, enforcement, engineering authority, and safe process
operation. Central services do not receive direct routes to controllers.

## Target Security Model

Hearthline applies the following architecture rules:

- Default-deny communication between security zones.
- Explicit conduits defined by source, destination, protocol, direction,
  purpose, owner, and availability requirement.
- Separate IT-side and OT-side enforcement at the factory OT DMZ.
- Controlled administrative access through jump services.
- Brokered or replicated production data for enterprise analytics.
- Passive monitoring paths that are not required for process forwarding.
- Identity, device assurance, session authorization, and least privilege in
  addition to network segmentation.
- Independent local process operation when inter-site services are unavailable.
- Explicit safety and burner-management boundaries that are not replaced by
  general-purpose control logic.

## Ceramics Process

```text
Body Preparation
  -> Forming
  -> Controlled Drying
  -> Industrial Dryer
  -> Color and Glaze
  -> Kiln 1
  -> Intermediate Inspection
  -> Kiln 2
  -> Final Inspection
  -> Logistics
```

Each process area is independently enterable and contains a representative cell
network, logical vPLC workload, local HMI, distributed I/O, sensors, actuators,
and safety or permissive interface. The target deployment assigns physical vPLC
execution to a factory-local redundant control-compute cluster; no runtime is
integrated yet. Detailed process documentation starts at the
[Ceramics Process](docs/factory/process/README.md).

## Repository Structure

```text
.
|-- config
|   `-- ot
|       `-- process
|-- docs
|   |-- customer-network
|   |-- central-office
|   |-- factory
|   `-- project-direction.md
|-- web
|   |-- src
|   |-- package.json
|   `-- README.md
|-- LICENSE
`-- README.md
```

The documentation tree follows the Svelte navigation tree. Every documentation
folder contains its own README plus physical and logical screenshots of the
corresponding current view.

## Roadmap

1. Define stable YAML identifiers, schemas, and reference rules.
2. Move the remaining site, environment, device, and connection data out of
   Svelte components.
3. Implement Rust schema and cross-reference validation.
4. Add route, NAT, stateful-policy, and conduit evaluation.
5. Emit versioned JSON view models for the Svelte application.
6. Add positive and negative network scenarios.
7. Select the supported IEC 61131-3 edition, Structured Text dialect, and
   Ladder interchange format.
8. Implement program, symbol, task, tag, and I/O cross-references.
9. Integrate virtual PLC execution with the Rust plant model.
10. Add process scenarios, accelerated time, material tracking, and fault
    injection.

## Running the Application

```bash
cd web
npm install
npm run dev
```

Quality checks:

```bash
npm run check
npm run build
```

## Documentation

- [Documentation index](docs/README.md)
- [Implementation direction](docs/project-direction.md)
- [Deployment conformance review](docs/deployment-conformance.md)
- [Svelte application](web/README.md)
- [Configuration model](config/README.md)

## License

Hearthline is licensed under the [MIT License](LICENSE).
