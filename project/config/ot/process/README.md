# OT Process Configuration Contract

The ceramics process contains ten independently enterable areas. Canonical
per-appliance records are stored under
[`project/config/appliances/factory/process`](../../appliances/factory/process); this
directory documents the additional area, control-source, and I/O-binding model
that is still required.

## Implemented Baseline

The eight baseline areas outside Body Preparation and Forming each have nine
parsed appliance files:

- One industrial access switch.
- One logical area vPLC.
- One HMI.
- One distributed-I/O station.
- Two field sensors.
- Two field actuators.
- One safety or permissive interface.

The separate physical `OT-vPLC-HOST-01/02` records are stored under
[`project/config/appliances/factory/platform`](../../appliances/factory/platform).
Physical-mode render bindings associate both hosts with each grouped vPLC host
marker, while logical mode resolves the area-specific controller file.

Every process appliance uses schema `0.10.0` and is validated by Rust. Port
hardware, state, speed, duplex, and MTU are appliance configuration; individual
Ethernet, virtual-runtime, and field-wiring attachments remain separate
connection documents. The
vPLC records also reserve `program_ref` and `io_binding` paths. Those references
are declared intent only: the referenced Structured Text programs and binding
documents do not exist yet, and the current validator does not resolve them.

The current process configuration is a provisional placeholder baseline.
Channel names, ranges, assignments, controller settings, and relationships
will be completed after the communication and process-simulation contracts can
exercise them.

## Remaining Sources

The process model still needs canonical records for:

- Cell-network and interface peer relationships.
- Controller tasks and program assignments.
- Symbolic tags and distributed-I/O channels.
- Sensor and actuator simulation parameters.
- Safety-status boundaries and independent protection ownership.
- Process scenarios, expected results, and fault cases.

Structured Text will remain in `.st` files. Ladder Diagram will use one declared
machine-readable interchange format, with PLCopen XML as the current
vendor-neutral candidate. Control logic is not embedded into appliance YAML.

## Generation Boundary

[`architecture.yaml`](architecture.yaml) owns the process areas, support nodes,
network relationships, and ordered material flow. Rust validates those records
against the appliance repository and generates both
[`process-view.json`](../../../../packages/web/src/generated/process-view.json)
and
[`appliance-configs.json`](../../../../packages/web/src/generated/appliance-configs.json).
The generated process catalog is presentation metadata, not process state.

Generated files are replaced atomically. Svelte rejects incompatible schema
versions and does not supply missing control, network, or process defaults.

Broader process-area control execution will continue to use the formal Rust
communication contracts for network and field messages through configured
ports and media.
