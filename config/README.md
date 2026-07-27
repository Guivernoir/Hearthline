# Hearthline Configuration Source

This directory is reserved for Hearthline's canonical YAML desired state.
Configuration becomes authoritative only after the Rust model can parse and
validate its schema and references.

## Current Status

No canonical YAML files are implemented yet. The directory currently contains
configuration contracts and the proposed ownership model only. Device records,
policies, scenarios, and process bindings shown elsewhere in the repository are
therefore architecture targets rather than parsed configuration.

The intended ownership boundary is:

```text
config/**/*.yaml --------+
logic/**/*.st -----------+--> Rust validation and simulation --> generated JSON
logic/**/*.xml ----------+                                      |
                                                                  v
                                                               Svelte
```

Svelte must not parse YAML, Structured Text, Ladder Diagram, or device
configuration. It renders versioned JSON emitted by Rust. Rust owns schema
validation, reference resolution, network and policy evaluation, IEC 61131-3
source analysis, process simulation, and scenario results.

Credentials, private keys, reusable pre-shared secrets, and production
certificates must not be stored here. YAML records may contain only references
to secrets supplied outside version control.

## Planned Structure

```text
config
|-- inventory.yaml
|-- topology.yaml
|-- devices
|-- policies
|-- scenarios
`-- ot
    `-- process
        |-- areas
        |-- components
        |-- bindings
        `-- README.md
```

Stable identifiers are repository-wide. Renaming a device, area, program, tag,
or scenario is a model migration because generated routes and cross-references
may depend on it.
