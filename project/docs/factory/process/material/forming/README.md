# Forming

**Zone:** `OT-AREA-02`  
**Route:** `factory/process/forming`

![Forming physical view](screenshot.png)

![Forming logical view](logical-screenshot.png)

Forming models one ceramic-slip pressure-casting cell. Prepared slip arrives
from Body Preparation and is held in an agitated or recirculating supply tank
at a configured reference temperature of approximately 40 degrees Celsius.
The cell fills and pressurizes a mould, drains excess slip, demoulds the piece
with a robotic arm, and conditions the mould before the next cycle.

The architecture is representative and vendor-neutral. The configured values,
timings, equipment capacities, and interlocks are development values rather
than a production machine specification.

## Current Representation

The view derives 32 components from canonical Forming appliance and connection
YAML. The inventory contains eight control and operator components plus 24
field or safety components. It is divided into four functional modules:

| Module | Process inputs | Controlled outputs | Interface |
| --- | --- | --- | --- |
| Ceramic-slip supply | Tank level, density, viscosity, temperature, feed flow, feed pressure | Slip supply and recirculation | `AREA-02-HMI-01` |
| Pressure-casting mould | Casting pressure, mould temperature, fill-head position, mould position | Mould movement | `AREA-02-HMI-02` |
| Water, air, and vacuum | Water flow, excess-slip drain flow, residual mould moisture, compressed-air supply pressure, vacuum | Water, compressed air, vacuum | `AREA-02-HMI-03` |
| Robotic demoulding | Robot position, piece-gripped confirmation | Robot motion | `AREA-02-HMI-04` |

`AREA-02-SCADA-01` exposes all 17 process signals and six process outputs.
Each module HMI receives a narrower scope from the same Rust session. SCADA
and the four HMIs therefore observe one shared controller, process, alarm,
output, and safety state rather than independent copies.

![Forming SCADA workstation](scada-screenshot.png)

The controller-source toolbar opens the exact validated Structured Text and
YAML I/O documents that back the running session, together with the current
step, scan interval, watchdog, source paths, and combined revision.

![Forming control-source viewer](control-source-screenshot.png)

## Implemented Cycle

`AREA-02-vPLC-01` executes this deterministic sequence from the versioned
[Structured Text source](../../../../../control/programs/forming/area-02-vplc-01.st)
and [I/O binding](../../../../../control/bindings/forming/area-02-vplc-01.yaml)
on a 20-millisecond task:

1. Fill the closed mould with prepared ceramic slip.
2. Apply compressed-air casting pressure and hold it for the configured dwell.
3. Drain excess slip while the piece remains supported by the mould.
4. Remove casting pressure.
5. Apply release water, followed by release air.
6. Open the mould and move the robot into the pickup position.
7. Confirm the piece is gripped and deliver it to the operator handoff.
8. Wash the empty mould with water.
9. Purge the mould with compressed air.
10. Apply vacuum until the modeled residual moisture is reduced.
11. Close the mould and return the cell to its ready state.

The configured timer presets total 14 seconds. Because timer completion is
observed on the next 20-millisecond controller scan, the current presets
complete in 14.04 simulated seconds. This accelerated duration is a simulator
setting and does not represent production cycle time.

Release water and release air are demoulding-assistance steps performed before
robot pickup. They are distinct from the later water wash and compressed-air
purge performed after the piece reaches the operator handoff.

## Control and I/O Flow

```text
Field input -> AREA-02-RIO-01 -> AREA-02-vPLC-01
                                      |          |
                                      v          v
                               AREA-02-SCADA  module HMI

SCADA or module HMI command -> AREA-02-vPLC-01
  -> AREA-02-RIO-01 -> field output
```

`AREA-02-RIO-01` owns the 24 configured field channels: 17 measured inputs,
six controlled outputs, and one safety-status interface. The vPLC is the
ordinary process-control authority. Operator interfaces display PLC-exposed
state and submit only commands permitted by their YAML scope; they do not
terminate sensor wiring or drive outputs directly.

`AREA-02-SAFE-01` publishes permissive and trip status into ordinary control.
The model keeps the protective function outside the standard HMI command
path. A simulated mould-overpressure event latches the machine-safety state;
the other implemented process disturbances stop the sequence without claiming
functional-safety behavior.

## Operations Telemetry

Rust captures the controller scan and selected values for tank inventory, slip
properties, mould pressure and moisture, utilities, robot position, and
piece-gripped state once per simulated second. `AREA-02-vPLC-01` originates the
bounded typed telemetry frame through its virtual host and Level 3 core to the
factory-local service `ot-operations-services-01`.

The API-session historian retains up to 60 local records. Accepted records are
queued for replication through `OT FRW-01A` and VLAN 352 to
`ot-dmz-hist-replica-01`, which retains a separate 60-record view. Failed
replication remains pending and is retried every 250 milliseconds; an
unreplicated record displaced by the bounded local store increments a visible
loss counter.

`AREA-02-SCADA-01` displays both tiers, the latest replicated payload, pending
and loss state, and the collection and replication traces. Its authorized
publication action uses only the latest DMZ record, then traverses the
inter-site conduit, northbound firewall, Business IT core, and server switch
to `operations-analytics-01`. Publication fails closed until a replicated
record exists.

This is volatile simulation state, not a production historian. It does not
provide durable database persistence, protocol subscriptions, OPC UA, MQTT,
Sparkplug, authentication, encryption, certificate handling, analytics
processing, retention policy, or data-quality governance.

## Fault Coverage

The SCADA simulation controls currently inject five deterministic
disturbances:

- Ceramic-slip supply loss during mould filling.
- Low compressed-air supply during pressure or air-use phases.
- Mould overpressure during pressurization or dwell.
- Vacuum loss during mould drying.
- Missing piece-gripped confirmation during robot pickup.

Each applicable disturbance produces a named alarm, stops the cycle in safe
output states, and requires the fault to be cleared before process reset.
Tests cover a complete normal cycle, vacuum-loss handling, shared SCADA/HMI
state, authorized reset behavior, the separate safety latch caused by mould
overpressure, and bounded historian eviction and acknowledgement. The two
canonical historian scenarios separately validate collection and replication
through their selected network paths.

## Engineering Basis

Public SACMI and SAMA material documents pressure-casting systems with
controlled slip feed, compressed air, water, vacuum, PLC automation, and
manual or robotic demoulding. Those references support the equipment
categories and control hierarchy used here; they do not validate Hearthline's
setpoints, timing, interlocks, or suitability for a real machine.

- [SACMI tableware pressure casting](https://sacmi.com/en-US/ceramics/tableware)
- [SAMA core-casting systems](https://sama.sacmi.com/en-US/ceramics/tableware/casting/core-casting-systems)
- [SACMI AVB high-pressure casting system](https://www.sacmi.com/en-US/ceramics/news/5803/Sacmi-AVB-high-pressure-battery-WC-casting)
- [SACMI body and glaze preparation](https://sacmi.com/en-us/ceramics/tableware/Body-and-glaze-preparation)

## Simulation Boundary

The Rust plant model advances local measurements, alarms, and physical fault
conditions. The bounded vPLC runtime parses and validates the Forming
Structured Text source, executes its step transitions on controller scans, and
maps integer commands to configured actuator states through YAML. It is the
first executable area-specific control-source slice in Hearthline.

This remains a simplified deterministic model. It does not calculate rheology,
pressure distribution, filtration, wall thickness, deformation, robot
kinematics, drying physics, material quality, or machine wear. The supported
Structured Text grammar is a Hearthline sequence subset, not a complete IEC
61131-3 implementation or a vendor-equivalent controller runtime. Ladder
Diagram is not parsed or executed.
The telemetry workflow carries compact JSON inside Hearthline's typed
application model; it is not a vendor historian or industrial messaging
protocol implementation.

## Planned Work

The next control phase will replace fixed elapsed-time transitions with
reviewed process conditions where appropriate, then add recipe parameters,
hold and retry behavior, richer permissives, bad-quality signals,
communication failures, part-quality state, and an executable cross-area
material balance for replenishment of the approximately 40 C Forming slip tank
from Body Preparation. Later work may extend the parser only after a declared
language edition and compatibility target are selected.
