# Hearthline Configuration Source

This directory contains Hearthline's canonical YAML desired state. Rust parses
one document per appliance, one per modeled connection, and one per configured
scenario before configuration or simulation results reach Svelte.

## Current Status

`appliances/` contains 162 schema `0.9.0` documents: 72 customer, provider,
enterprise, DMZ, operations, conduit, Level 3, and control-host records plus 90
process-area records. `connections/` contains 208 schema `0.2.0` documents
covering copper, fiber, radio, carrier, virtual, and field-wiring
relationships. File names and stable IDs are identical.
`scenarios/` currently contains 28 schema `0.11.0` simulations: independent
Customer PC-01 and PC-02 public paths, independent Business IT PC-01 through
PC-04 internal DNS and HTTPS paths, approved and denied factory
operations-data transfer, and three controlled public-web security exercises.
Availability coverage adds a customer WAN restoration contract, a Business IT
core failover with explicit link and VRRP role transitions, and six
northbound-firewall contracts for converged ownership transfer, media-carried
session continuity, HA-sync loss after replication, fail-closed standby
session-state loss, idle-session expiry, and fenced sync-path isolation.
The twenty-eighth scenario combines both failed factory-facing conduit
handoffs with an independently validated Body Preparation control path and a
configured HMI safety-reset plus actuator command.

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
- Optional endpoint hostnames and DNS-server addresses used by interactive
  workstation profiles.
- Required family constraints such as switch VLANs, NAT inside/outside roles,
  default-deny firewalls, application-gateway HTTP method allowlists and
  bounded inspection rules, non-inline passive sensors, valid sensor ranges,
  and non-empty safety permissives.
- Render bindings used to associate one or more appliance files with a Svelte
  node.
- Connection endpoint appliance and interface references.
- Appliance-kind to port-hardware compatibility.
- Layer 3 switch SVIs with virtual hardware, one VLAN, a MAC address, and an
  IPv4 address; SVIs cannot terminate media connections.
- VRRP first-hop groups with on-link virtual addresses, protocol-derived
  virtual MACs, unique member priorities, and exactly one configured active
  member across each appliance pair.
- LACP system identities, logical bundles, active or passive modes, minimum
  active members, unique local membership, and eligible switch interfaces.
- Reciprocal multi-chassis peers with one shared LACP system MAC, opposite
  primary and secondary roles, and one direct bidirectional peer link.
- Reciprocal stateful-firewall peers with opposite active and standby roles,
  matching monitored virtual identities, synchronized policy and route intent,
  and one direct operational HA-sync link.
- Aggregate member compatibility across physical connections, including
  logical systems, interface mode, speed, duplex, MTU, and VLANs.
- Port administrative and initial operational-state consistency.
- Port speed and MTU bounds.
- Transport-to-medium and medium-to-port compatibility.
- Connection capacity against both endpoint speeds and modeled medium limits.
- Unique endpoint pairs and exclusive use of point-to-point physical ports.
- Valid deterministic-loss intervals and connector endpoint direction.
- Scenario identity, packet structure, event bounds, participant and source
  references, source-address ownership, and connected operational topology.
- Unique scenario connection overrides that resolve to links inside the
  selected participant topology.
- Unique scenario first-hop overrides that resolve to configured interfaces
  inside the selected participant topology and reject simultaneous active
  members of one virtual gateway group.
- Unique firewall-HA overrides that preserve exactly one active member in each
  selected complete domain.
- Recovery contracts with at least one link, first-hop, or firewall-HA change,
  selected-topology references, and a valid expectation component and outcome.
- Continuity contracts with ordered bounded fault injection, reciprocal TCP
  flows, active-member failure, heartbeat and hold timing, converged data paths,
  HA-sync-state consistency, and explicit delivery or fail-closed expectations.
- Local-autonomy contracts with at least two failed inter-site links, a valid
  HMI-owned safety interface and actuator command, operational local media
  paths, a northbound drop expectation, and derived controller and remote-I/O
  participation.
- Security-exercise tactic, technique, severity, control, detector
  participation, and operations-console defender references.

Connection files are authoritative for attachment relationships. Appliance
ports do not duplicate peer references. Radio and virtual media may
intentionally share an endpoint; copper, fiber, carrier, field wiring, and
telephone endpoints are exclusive. Passive monitoring is an `a-to-b`
transport over a declared copper or fiber bearer, not a separate physical
medium.

Validation does not yet prove project-wide address uniqueness, VLAN and route
consistency, general NAT or policy correctness, vendor HA behavior, or every possible
scenario outcome. Selected appliance and connection subgraphs are executable;
the complete 162-appliance graph is not assembled as one running topology.

The customer scenarios carry a DNS request and response through the customer
edge and provider network, then exercise the selected public-service path.
HTTPS to `shop.hearthline.test` is translated from `192.0.2.10` to the DMZ web
gateway, admitted by a named perimeter rule, and forwarded to an abstract
application VIP; public SSH is dropped by default policy. The factory pair
carries selected historian-replica data across VLAN 352 and the governed
inter-site conduit to the Applications VLAN: TCP 443 matches a named firewall
rule and reaches analytics, while TCP 22 is dropped by default policy. Each run
emits deterministic per-hop transit, policy, delivery, forwarding, or drop
results.

The local-autonomy case expands that factory baseline into two independently
evaluated execution domains. Both factory-facing conduit handoffs are down, so
the historian request fails on each redundant path. The Body Preparation HMI,
vPLC, remote I/O, safety interface, and pump remain connected through seven
factory-local links; the healthy safety circuit resets and the pump reaches
`running`. This proves the configured command path only. It does not yet run
the referenced Structured Text program or evolve plant material state.

The Business IT scenarios carry PC-01 through PC-04 traffic from VLAN 30
through `Business IT-USR-SW-01` or `Business IT-USR-SW-02`. Core-01 initially
owns the virtual gateways for VLANs 20, 30, and 80 and routes between physical
trunks. DNS reaches `Business IT-DNS-01` at `10.10.20.10`; HTTPS reaches
`Business IT-PORTAL-01` at `10.10.80.20`. Both services return to each
originating workstation. The availability scenario then fails Core-01's
selected user and infrastructure uplinks, transfers all three VRRP groups to
Core-02, and verifies the unchanged PC-03 DNS request through the secondary
forwarding path. All eight Business IT switching members declare Rapid-PVST
bridge identity and priority. The six access and service switches also declare
LACP uplinks to a shared multi-chassis core system. Rust derives member
selection and minimum-link health, chooses one physical egress per flow,
preserves learned bundle reachability across member failure, applies flooded
traffic split horizon, and reconciles aggregate members with the per-VLAN
spanning-tree snapshot. The selected availability topology begins with eight
distributing aggregate endpoints, then continues with the four Core-02-side
endpoints after the Core-01 links fail. This is an instantaneous converged
state model, not BPDU or LACP timer execution, measured convergence, a vendor
peer protocol, or full control-plane state synchronization.

The northbound-firewall recovery scenario begins with `Business FRW-03A`
owning the shared Business IT and factory-conduit virtual addresses. Recovery
withdraws its data links, restores the peer data links, promotes `Business
FRW-03B`, transfers both virtual identities, and verifies the unchanged
historian-to-analytics HTTPS request through the standby member's matching
policy. The scenario represents an already converged state. It does not run a
heartbeat or election protocol, measure recovery time, or transfer session
records over the HA-sync connection.

The availability scenario starts from the same valid customer DNS topology,
then applies a request-scoped down state to
`customer-cpe-01-to-wan-01`. The trace stops when
`Customer INET-CPE-01` attempts carrier transit. Simulation API requests may
override selected link states for one run; neither canonical connection YAML
nor subsequent scenarios are modified.

The security exercises use the same selected public path. One configured
`curl` request contains a traversal pattern, another uses `DELETE` against a
gateway whose YAML allows only GET, HEAD, and POST, and a third submits a
bounded POST body containing a configured SQL tautology signature. All three
reach `Business WEB-GW-01`, whose Rust WAF behavior rejects them for distinct
reasons defined by method or inspection policy. Reports project the detector,
control, evidence, severity, source and destination, and intended Central SOC
defender. The SOC queue is session-local API state; no routed telemetry or
SIEM correlation pipeline is claimed.

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
[`packages/crates/hearthline-engine/src/physical/media`](../../packages/crates/hearthline-engine/src/physical/media).
Each module validates its own physical parameters and computes the facts used
by the connector. The current model covers compatibility, length or range
limits, capacity, and propagation. It does not yet calculate copper electrical
behavior, fiber optical budgets, radio interference, connector loss, queueing,
or autonegotiation timing.

The intended ownership boundary is:

```text
project/config/**/*.yaml -+
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
project/config
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
|-- scenarios
|   |-- business-it
|   |   |-- dns
|   |   `-- portal
|   |-- customer
|   |   |-- customer-dns-lookup.yaml
|   |   |-- customer-pc-02-dns-lookup.yaml
|   |   |-- customer-pc-02-public-web-management-denied.yaml
|   |   |-- customer-pc-02-public-web-request.yaml
|   |   |-- customer-public-web-management-denied.yaml
|   |   `-- customer-public-web-request.yaml
|   |-- factory
|   |   |-- factory-operations-data.yaml
|   |   |-- factory-operations-data-denied.yaml
|   |   `-- resilience
|   |       `-- factory-local-autonomy-conduit-outage.yaml
|   |-- resilience
|   |   |-- business-it-core-failover-dns.yaml
|   |   |-- firewall-ha
|   |   |   |-- business-northbound-firewall-failover.yaml
|   |   |   |-- business-northbound-firewall-ha-sync-loss.yaml
|   |   |   |-- business-northbound-firewall-isolation-fenced.yaml
|   |   |   |-- business-northbound-firewall-session-continuity.yaml
|   |   |   |-- business-northbound-firewall-session-state-loss.yaml
|   |   |   `-- business-northbound-firewall-stale-session-expiry.yaml
|   |   `-- customer-wan-access-outage.yaml
|   `-- security
|       |-- customer-public-web-method-denied.yaml
|       |-- customer-public-web-path-traversal-detected.yaml
|       `-- customer-public-web-sql-injection-detected.yaml
`-- ot
    `-- process
        `-- README.md
```

Stable identifiers are repository-wide. Renaming a device, area, program, tag,
or scenario is a model migration because generated routes and cross-references
may depend on it.

## Commands

```bash
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- config-validate
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- config-generate
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run customer-dns-lookup
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run customer-public-web-request
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run customer-public-web-management-denied
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run customer-public-web-path-traversal-detected
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run customer-public-web-method-denied
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run customer-public-web-sql-injection-detected
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run customer-wan-access-outage
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run business-it-user-pc-01-dns
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run business-it-user-pc-01-portal
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run factory-operations-data
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run factory-operations-data-denied
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run factory-local-autonomy-conduit-outage
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run business-northbound-firewall-session-continuity
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run business-northbound-firewall-ha-sync-loss
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run business-northbound-firewall-session-state-loss
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run business-northbound-firewall-stale-session-expiry
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run business-northbound-firewall-isolation-fenced
```

`config-generate` first validates both canonical directories and then
atomically replaces
[`packages/web/src/generated/appliance-configs.json`](../../packages/web/src/generated/appliance-configs.json).
The generated catalog includes normalized appliance and connection summaries,
node and appliance-to-connection indexes, source revisions, and the original
YAML used by the configuration routes.

The localhost API exposes revision-checked appliance and connection updates,
scenario catalog and execution endpoints, workstation profile and action
routes, and a session-local security-console event queue:

```bash
cargo run --manifest-path packages/Cargo.toml -p hearthline-api
```

An update is parsed in memory and validated against the full project before the
source and generated catalog are replaced. The API binds to `127.0.0.1` by
default and is not a remote administration service. Scenario execution accepts
optional packet and selected-connection state overrides without modifying
canonical YAML. When effective connection state matches a scenario's declared
recovery state, Rust evaluates the recovery expectation instead of the
baseline expectation.
`GET /api/workstations/{id}` projects a configured endpoint profile and
`POST /api/workstations/{id}/actions` executes supported terminal or browser
actions through compatible scenarios. `GET /api/security/consoles/{id}`
returns modeled evidence, while acknowledgement and clear routes update only
the current local API session. Application releases, schema
compatibility, and migration expectations are defined in the
[versioning policy](../docs/reference/versioning.md) and
[changelog](../../CHANGELOG.md).
