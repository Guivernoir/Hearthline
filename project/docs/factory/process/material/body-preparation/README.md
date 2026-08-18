# Body Preparation Complex

- **Zone:** `OT-AREA-01`
- **Gateway route:** `factory/process/body-preparation`
- **Implementation status:** Executable development model with provisional architecture

![Body Preparation building gateway](screenshot.png)

![Body Preparation control overview](logical-screenshot.png)

![Body Preparation operator interface](hmi-screenshot.png)

Body Preparation is the gateway to three separately modeled process buildings.
The separation reflects their different equipment, material risks, utility
needs, and maintenance boundaries. The buildings share one coupled material
and utility process model, but not one operator or controller authority.

| Building | Detailed route | Runtime responsibility |
| --- | --- | --- |
| [Slip Preparation](slip-preparation/README.md) | `body-preparation/slip` | Mineral batching, blunging, conditioning, quality release, and transfer to Forming |
| [Water Preparation and Distribution](water-treatment/README.md) | `body-preparation/water` | Industrial-water treatment/distribution plus segregated return-water recovery |
| [Glaze Preparation](glaze-preparation/README.md) | `body-preparation/glaze` | Seven-material batching, wet milling, finishing, quality release, storage, and transfer |

The gateway physical view shows separate buildings, local control rooms,
utility corridors, and downstream handoffs. Each detailed physical view is a
walkdown-oriented arrangement of the major equipment installed in that
building. Detailed logical views expose every configured control and field
endpoint owned by the building.

## Local Control Architecture

The canonical Area 01 topology contains `166` appliances and `175` connections:
`22` control appliances, `144` field appliances, `37` control-network links,
and `138` sensor/actuator plus `6` safety attachments. Slip and Glaze each own
one local HMI, virtual controller, access switch, and safety scope. Water uses
four independent HMI/controller/safety scopes for industrial treatment,
industrial distribution, return treatment, and return distribution; those
scopes share one VLAN `111` access switch. Seven remote-I/O stations preserve
field ownership.

| Station | Ownership | Channels |
| --- | --- | ---: |
| `AREA-01-RIO-01` | Slip batching and wet mixing | 12 |
| `AREA-01-RIO-02` | Slip finishing, quality, transfer, safety, and handoff pipeline | 22 |
| `AREA-01-RIO-03` | Industrial-water treatment and analyzers | 21 |
| `AREA-01-RIO-06` | Industrial-water routes and eight duplex-group pumps | 22 |
| `AREA-01-RIO-04` | Segregated return-water treatment and analyzers | 20 |
| `AREA-01-RIO-07` | Return collection/reuse routes and eight duplex-group pumps | 21 |
| `AREA-01-RIO-05` | Glaze process, safety, and handoff pipeline | 26 |

The six HMIs collectively expose `85` measured signals, `53` commanded
actuators, `32` editable parameters, six safety interfaces, and four local
development recipe objects. Each HMI can command only its controller-owned
process or distribution scope. Rust owns live batch progression, inventory, bounded material
properties, output state, quality disposition, and deterministic faults. YAML owns appliance,
port, I/O, connection, permission, recipe, and parameter declarations. Svelte
renders the gateway, building views, configuration inspectors, and HMI.

## Runtime Model

The three buildings contain four independently controllable trains:

| Train | Start/Hold scope | Primary disposition |
| --- | --- | --- |
| `slip` | Slip Preparation only | Released ceramic slip for Forming |
| `water` | Fresh-water treatment only | Released process water |
| `return-water` | Return-water recovery only | Segregated body or glaze reuse water |
| `glaze` | Glaze Preparation only | Released liquid glaze |

Each train supports Start, Hold, and Resume without changing the others. Hold
drives that train's outputs to their safe states while preserving phase and
inventory. Parameter changes are idle-only. A process trip latches an alarm,
stops the affected train, and requires a healthy safety circuit, removal of
the injected cause, and an operator reset before restart.

The engineering time base is `50 ms` of wall time per simulated process
minute. It is intended for deterministic development and regression tests, not
for prediction of production throughput.

## Monitored Handoffs

Four cross-building material handoffs remain explicit runtime paths: treated
water to Slip, treated water to Glaze, released slip to Forming, and released
glaze to the glazing process. The water utility additionally models eight
industrial and return-water routes with direct pressure, flow, balance, and
quality readings, `16` heartbeat-supervised pumps, duty/standby transfer, and
maintenance dispatch. YAML-defined instruments terminate on their owning RIO.
The slip line also has an ultrasonic entrained-air monitor.

An injected leak creates inlet/outlet flow imbalance, pressure loss, reduced
delivered quality, and a warning alarm. A slip-line leak additionally raises
the delivered batch's entrained-air value and reduces the Forming filling-flow,
casting-rate, and green-strength factors while increasing fired-defect risk.
This relationship follows public evidence that gas bubbles can enter ceramic
slip through process pipework and remain latent until firing
([Ultrasonics research](https://doi.org/10.1016/j.ultras.2009.07.008)). Paired
flow and pressure monitoring is a general leak-detection inference, not a
ceramics-specific standard ([pipeline monitoring study](https://doi.org/10.3390/pr13082459)).
The simulated `24%` line loss and `3.5%` entrained-air disturbance are visible
engineering test assumptions, not measured plant values.

## Public Reference Basis

The development values are derived from public sanitaryware body, slip, glaze,
and ceramic-manufacturing references:

- [Published sanitaryware body composition](https://doi.org/10.1016/j.ceramint.2013.11.139)
- [Published sanitaryware slip preparation and measured properties](https://doi.org/10.1590/0366-69132019653752687)
- [Published sanitaryware glaze formulation and preparation](https://doi.org/10.2298/SOS2102209B)
- [European Commission ceramic-manufacturing BREF](https://bureau-industrial-transformation.jrc.ec.europa.eu/sites/default/files/2019-11/cer_bref_0807.pdf)

These values are transparent simulation baselines. They are not production
formulas, commissioning setpoints, discharge limits, safety requirements, or
substitutes for laboratory trials and site-specific engineering.

## Implementation Boundary

All four bounded process trains currently execute in Rust. The Structured Text
source and YAML I/O binding cover only the slip sequence; they are parsed and
validated as a controller definition, but the composite Body Preparation model
does not claim a general IEC 61131-3 runtime. Water, return-water, and glaze
control sources remain planned.

The model does not yet represent particle-size distributions, detailed
rheological constitutive behavior, chemical equilibrium, membrane fouling,
filter-cake resistance, full mill-power dynamics, laboratory uncertainty,
production scheduling, or environmental permit compliance. Architecture and
configuration values remain provisional development inputs.

## Planned Work

1. Add finite pipe volume, transit delay, downstream receiving capacity,
   demand, interrupted transfer, isolation, and recovery behavior.
2. Add water, return-water, and glaze control-source definitions after a
   formal controller-language compatibility target is selected.
3. Add laboratory sampling, recipe revision and deployment, batch genealogy,
   reject routing, and historian retention.
4. Refine water balance, filter loading, sludge inventory, reuse constraints,
   disposal paths, distributed monitoring, and pump condition signals using
   additional public-reference scenarios.
5. Replace provisional building arrangements and configuration values as
   executable scenarios establish stronger requirements.
