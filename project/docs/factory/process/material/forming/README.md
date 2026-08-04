# Forming

**Zone:** `OT-AREA-02`  
**Route:** `factory/process/forming`

![Forming physical view](screenshot.png)

![Forming logical view](logical-screenshot.png)

Forming covers local material feed, shape creation, press behavior, transfer,
and machine safety.

## Current Representation

The architecture view renders the representative assets below from bootstrap
JSON. `AREA-02-HMI-01` is enterable as a Rust-backed operator session assembled
from the area appliance YAML. It displays the configured part-presence and
pressure samples, starts with `AREA-02-SAFE-01` latched, and exposes the press
and conveyor state domains after an authorized safety reset. Accepted commands
traverse the HMI, vPLC, remote I/O, and selected field actuator.

This is a deterministic operator-command baseline. Structured Text execution,
machine sequencing, pressure dynamics, and material movement remain pending.

| Function | Representative component |
| --- | --- |
| Cell network | `AREA-02-SW-01` |
| Control workload | `AREA-02-vPLC-01` on `OT-vPLC-HOST-01/02` |
| Local operation | `AREA-02-HMI-01` |
| Distributed I/O | `AREA-02-RIO-01` |
| Sensors | `AREA-02-PE-01`, `AREA-02-PT-01` |
| Actuators | `AREA-02-PRESS-01`, `AREA-02-CV-01` |
| Safety interface | `AREA-02-SAFE-01` |

## Planned Work

Simulation will model machine cycles, pressure, part presence, discharge, jams,
guards, and emergency-stop state. Per-appliance YAML is implemented; resolved
control and I/O bindings remain pending.
