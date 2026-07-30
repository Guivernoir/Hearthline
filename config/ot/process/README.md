# OT Process Configuration Contract

The ceramics process contains ten independently enterable areas. Canonical
per-appliance records are stored under
[`config/appliances/factory/process`](../../appliances/factory/process); this
directory documents the additional area, control-source, and I/O-binding model
that is still required.

## Implemented Baseline

Each area currently has nine parsed appliance files:

- One industrial access switch.
- One logical area vPLC.
- One HMI.
- One distributed-I/O station.
- Two field sensors.
- Two field actuators.
- One safety or permissive interface.

The separate physical `OT-vPLC-HOST-01/02` records are stored under
[`config/appliances/factory/platform`](../../appliances/factory/platform).
Physical-mode render bindings associate both hosts with each grouped vPLC host
marker, while logical mode resolves the area-specific controller file.

Every process appliance uses schema `0.3.0` and is validated by Rust. Port
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

- Area sequence and material-flow relationships.
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

[`process-view.json`](../../../web/src/generated/process-view.json) remains a
bootstrap presentation model. Rust currently generates only
[`appliance-configs.json`](../../../web/src/generated/appliance-configs.json)
from validated appliance YAML. A future process generator must add area
topology, control-source and I/O cross-references, simulation state, scenario
results, and source-located diagnostics before the bootstrap model can be
retired.

Generated files are replaced atomically. Svelte rejects incompatible schema
versions and does not supply missing control, network, or process defaults.

Before process-area control execution is integrated, the Rust engine will
establish the formal communication contract used to carry network and field
messages through these configured ports and media.
