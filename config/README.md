# Hearthline Configuration Source

This directory contains Hearthline's canonical YAML desired state. Rust parses
one document per appliance and one document per modeled connection before
configuration metadata reaches Svelte.

## Current Status

`appliances/` contains 160 schema `0.3.0` documents: 70 customer, provider,
enterprise, DMZ, operations, conduit, Level 3, and control-host records plus 90
process-area records. `connections/` contains 205 schema `0.2.0` documents
covering copper, fiber, radio, carrier, virtual, and field-wiring
relationships. File names and stable IDs are identical.

These files are the canonical inputs to the current parser, but their
engineering content is not finished. Most addresses, policies, services,
equipment characteristics, availability choices, and topology details are
provisional placeholders used to exercise schemas and unblock work on the
communication and simulation layers. They must not be interpreted as
deployment-ready device configurations.

Rust currently validates:

- YAML syntax and strict known fields.
- Stable appliance and interface identifiers.
- One-file-per-appliance file-name identity.
- Unique appliance IDs and interface IDs.
- Appliance-kind to behavior-family compatibility.
- Required family constraints such as switch VLANs, NAT inside/outside roles,
  default-deny firewalls, non-inline passive sensors, valid sensor ranges, and
  non-empty safety permissives.
- Render bindings used to associate one or more appliance files with a Svelte
  node.
- Connection endpoint appliance and interface references.
- Appliance-kind to port-hardware compatibility.
- Port administrative and initial operational-state consistency.
- Port speed and MTU bounds.
- Transport-to-medium and medium-to-port compatibility.
- Connection capacity against both endpoint speeds and modeled medium limits.
- Unique endpoint pairs and exclusive use of point-to-point physical ports.
- Valid deterministic-loss intervals and connector endpoint direction.

Connection files are authoritative for attachment relationships. Appliance
ports do not duplicate peer references. Radio and virtual media may
intentionally share an endpoint; copper, fiber, carrier, field wiring, and
telephone endpoints are exclusive. Passive monitoring is an `a-to-b`
transport over a declared copper or fiber bearer, not a separate physical
medium.

Validation does not yet prove address uniqueness, VLAN and route consistency,
NAT or policy correctness, HA behavior, or scenario outcomes. The 205
connections form validated topology input but are not yet instantiated as the
complete simulator graph.

The next configuration milestone is to construct executable ports and
connections from these records. Rust will then carry typed messages between
devices through the selected medium and produce deterministic per-hop transit
or drop results. Configuration refinement will follow those executable
scenarios so placeholder values can be replaced with requirements that are
actually exercised and cross-validated.

## Port And Connection Model

An appliance port declares configured device state:

- Hardware such as `ethernet-rj45`, `ethernet-sfp`, `wireless-radio`,
  `carrier-demarc`, `virtual-nic`, `field-io-channel`, or `telephone-rj11`.
- Administrative and initial operational state.
- Configured speed, duplex, and MTU.
- Logical mode, addresses, and VLAN membership.

Rust owns the capability matrix between appliance kinds, port hardware, and
media. For example, a router may expose Ethernet, fiber, or carrier ports but
cannot be configured with an analog telephone port. YAML selects from those
capabilities; it does not define new hardware behavior.

Transport and medium are separate fields. Ethernet is a transport that may use
copper or fiber; wireless LAN uses radio; provider access uses a carrier
service; virtual runtime attachment uses a virtual medium; and process I/O uses
field wiring. Analog telephone transport and RJ11 cabling are modeled by Rust
even though the current topology has no telephone-media connection. This
avoids treating protocol, port hardware, and physical bearer as equivalent.

Each connection declares:

- A stable ID, label, lifecycle, and tags.
- Exactly two appliance/interface endpoints.
- Transport and typed medium details such as straight-through copper category,
  cable length, fiber mode and connector, radio distance and security, or
  carrier service.
- Link capacity, fixed latency, deterministic loss, explicit direction, and
  configured operational state.

At runtime, Rust combines both endpoint port records with the connection
record. It derives initial link usability, effective MTU, negotiated duplex,
serialization delay, and medium propagation delay. A configured connection
cannot carry traffic when either endpoint port is administratively or
operationally down.

For example,
[`customer-pc-01-to-sw-01.yaml`](connections/customer/lan/customer-pc-01-to-sw-01.yaml)
defines one straight-through copper Ethernet link. The two passive-monitoring
capture files use `a-to-b`; ordinary network and field links are
`bidirectional`.

The physical media implementation is split by type under
`crates/hearthline-engine/src/media/`. Each module validates its own physical
parameters and computes the facts used by the connector. The current model
covers compatibility, length or range limits, capacity, and propagation. It
does not yet calculate copper electrical behavior, fiber optical budgets,
radio interference, connector loss, queueing, or autonegotiation timing.

The intended ownership boundary is:

```text
config/**/*.yaml --------+
logic/**/*.st -----------+--> Rust validation and simulation --> generated JSON
logic/**/*.xml ----------+                                      |
                                                                  v
                                                               Svelte
```

Svelte does not parse YAML, Structured Text, Ladder Diagram, or device
configuration. It renders versioned JSON emitted by Rust. Rust owns schema
validation, reference resolution, network and policy evaluation, IEC 61131-3
source analysis, process simulation, and scenario results.

Credentials, private keys, reusable pre-shared secrets, and production
certificates must not be stored here. YAML records may contain only references
to secrets supplied outside version control.

## Structure

```text
config
|-- appliances
|   |-- customer
|   |-- internet
|   |-- central-office
|   |-- factory
|   `-- shared
|-- connections
|   |-- customer
|   |-- internet
|   |-- central-office
|   |-- factory
|   `-- shared
`-- ot
    `-- process
        `-- README.md
```

Stable identifiers are repository-wide. Renaming a device, area, program, tag,
or scenario is a model migration because generated routes and cross-references
may depend on it.

## Commands

```bash
cargo run -p hearthline-cli -- config-validate
cargo run -p hearthline-cli -- config-generate
```

`config-generate` first validates both canonical directories and then
atomically replaces
[`web/src/generated/appliance-configs.json`](../web/src/generated/appliance-configs.json).
The generated catalog includes normalized appliance and connection summaries,
node and appliance-to-connection indexes, source revisions, and the original
YAML used by the configuration routes.

The localhost configuration API exposes revision-checked appliance and
connection updates:

```bash
cargo run -p hearthline-api
```

An update is parsed in memory and validated against the full project before the
source and generated catalog are replaced. The API binds to `127.0.0.1` by
default and is not a remote administration service. Application releases,
schema compatibility, and migration expectations are defined in the
[versioning policy](../docs/versioning.md) and [changelog](../CHANGELOG.md).
