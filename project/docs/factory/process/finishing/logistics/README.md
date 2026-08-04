# Logistics

**Zone:** `OT-AREA-10`  
**Route:** `factory/process/logistics`

![Logistics physical view](screenshot.png)

![Logistics logical view](logical-screenshot.png)

Logistics covers packing, palletizing, traceability, warehouse handoff, and
production reporting.

## Current Representation

The architecture view renders the representative assets below from bootstrap
JSON. `AREA-10-HMI-01` is enterable as a Rust-backed operator session assembled
from the area appliance YAML. It displays configured scanner and pallet
presence states, requires a healthy safety reset, and exposes packing-machine
and palletizer cycle commands through the HMI, vPLC, remote I/O, and field
actuator.

This is a deterministic operator-command baseline. Product identity handling,
packing sequences, warehouse integration, and Structured Text execution remain
pending.

| Function | Representative component |
| --- | --- |
| Cell network | `AREA-10-SW-01` |
| Control workload | `AREA-10-vPLC-01` on `OT-vPLC-HOST-01/02` |
| Local operation | `AREA-10-HMI-01` |
| Distributed I/O | `AREA-10-RIO-01` |
| Sensors | `AREA-10-SCAN-01`, `AREA-10-PE-01` |
| Actuators | `AREA-10-PACK-01`, `AREA-10-PAL-01` |
| Safety interface | `AREA-10-SAFE-01` |

## Planned Work

Simulation will model product identity, pallet presence, packing and
palletizing state, discharge, warehouse handoff, and representative reporting.
Per-appliance YAML is implemented; resolved control bindings and logistics
behavior remain pending.
