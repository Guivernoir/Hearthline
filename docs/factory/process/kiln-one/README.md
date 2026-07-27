# Kiln 1

**Zone:** `OT-AREA-06`  
**Route:** `factory/process/kiln-one`

![Kiln 1 physical view](screenshot.png)

![Kiln 1 logical view](logical-screenshot.png)

Kiln 1 represents the primary firing profile, temperature zones, pressure,
airflow, combustion interfaces, and unloading.

## Current Representation

The Svelte view renders the representative assets below from bootstrap JSON. It
does not currently execute control logic, I/O, burner-management, or process
behavior.

| Function | Representative component |
| --- | --- |
| Cell network | `AREA-06-SW-01` |
| Control workload | `AREA-06-vPLC-01` on `OT-vPLC-HOST-01/02` |
| Local operation | `AREA-06-HMI-01` |
| Distributed I/O | `AREA-06-RIO-01` |
| Sensors | `AREA-06-TT-01`, `AREA-06-PT-01` |
| Actuators | `AREA-06-BNR-01`, `AREA-06-FAN-01` |
| Burner-management interface | `AREA-06-BMS-01` |

## Planned Work

Hearthline will simulate the process and control interface without claiming
functional-safety or burner-management certification. Canonical YAML and
control bindings remain pending.
