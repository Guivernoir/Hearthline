# OT Process Configuration Contract

The OT process configuration will describe ten independently enterable process
areas and the components shown inside them. The current Svelte application uses
[`process-view.json`](../../../web/src/generated/process-view.json) as a
bootstrap derivative while the Rust workspace and canonical YAML schemas are
still pending.

## Current Status

This document is a proposed contract. None of the example records are currently
parsed, validated, or used to generate the Svelte application. The bootstrap
JSON contains presentation records and future `configRef` values so the
consumer contract can be developed before the authoritative configuration
pipeline exists.

## Source Rules

- One area file is stored at `areas/<area-id>.yaml`.
- One component file is stored at `components/<component-id>.yaml`.
- Area files reference component identifiers; they do not duplicate component
  configuration.
- Controller-to-program, tag-to-I/O, and simulation bindings are separate
  records under `bindings/`.
- Structured Text remains in `.st` files and Ladder Diagram uses the selected
  machine-readable interchange format. Neither is embedded in YAML.
- Every reference is resolved and validated by Rust before JSON is emitted.
- Svelte consumes generated JSON only and never supplies configuration defaults
  or simulation behavior.

## Minimum Component Record

Each component YAML file will contain:

```yaml
schema_version: 0.1.0
id: area-01-lt-01
kind: sensor
site: factory
zone: ot-area-01
label: AREA-01-LT-01
role: Body tank level transmitter
icon: gauge
upstream: area-01-rio-01
network: null
io:
  direction: input
  signal: analog
  variable: BodyTankLevel
simulation:
  model: tank-level
  parameters_ref: process/body-preparation
```

The example `schema_version` refers to the planned configuration schema, not
the current frontend view schema `0.2.0`. Its exact structure remains
provisional until the Rust validator is implemented. Fields unsupported by a
device or process are explicitly `null` or omitted according to the final
schema; they are not inferred in Svelte.

## Generation Contract

Rust will produce a versioned process view model containing:

- Process sequence and material-flow order.
- Area routes, labels, display metadata, and source references.
- Component inventory and upstream relationships.
- Network and I/O relationships.
- IEC 61131-3 source and symbol cross-references.
- Current simulation state and timestamp.
- Connectivity and policy scenario results.
- Validation diagnostics with source locations.

Generated files are replaced atomically. Svelte rejects incompatible schema
versions rather than silently rendering partial data.

## vPLC and I/O Records

The planned canonical model distinguishes three separate assets:

- The physical `OT-vPLC-HOST-01/02` compute cluster.
- The logical `AREA-xx-vPLC-01` runtime assigned to one process area.
- The physical `AREA-xx-RIO-01` distributed I/O station inside that cell.

Each area network remains isolated from the cell boundary to the runtime
interface on the host. Sensors and actuators bind to channels on the local
distributed-I/O station; the vPLC exchanges process I/O with that station over
the declared industrial control network.
