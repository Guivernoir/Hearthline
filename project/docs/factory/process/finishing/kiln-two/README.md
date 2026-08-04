# Kiln 2

**Zone:** `OT-AREA-08`  
**Route:** `factory/process/kiln-two`

![Kiln 2 physical view](screenshot.png)

![Kiln 2 logical view](logical-screenshot.png)

Kiln 2 represents the second firing profile, combustion interfaces,
interlocks, and unloading.

## Current Representation

The architecture view renders the representative assets below from bootstrap
JSON. `AREA-08-HMI-01` is enterable as a Rust-backed operator session assembled
from the area appliance YAML. It displays configured temperature and pressure
samples, evaluates the three modeled burner-management permissives, and exposes
burner-demand and fan states after an authorized reset. Accepted commands
traverse the HMI, vPLC, remote I/O, and field actuator.

The reset and burner-demand path is a deterministic simulation contract. It is
not burner-management logic, a firing sequence, or evidence of functional
safety; Structured Text and kiln dynamics remain pending.

| Function | Representative component |
| --- | --- |
| Cell network | `AREA-08-SW-01` |
| Control workload | `AREA-08-vPLC-01` on `OT-vPLC-HOST-01/02` |
| Local operation | `AREA-08-HMI-01` |
| Distributed I/O | `AREA-08-RIO-01` |
| Sensors | `AREA-08-TT-01`, `AREA-08-PT-01` |
| Actuators | `AREA-08-BNR-01`, `AREA-08-FAN-01` |
| Burner-management interface | `AREA-08-BMS-01` |

## Planned Work

The same safety boundary applies as for Kiln 1: simulation and interface tests
do not constitute burner-management or functional-safety certification.
Per-appliance YAML is implemented; resolved control bindings and firing
behavior remain pending.
