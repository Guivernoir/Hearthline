# Rust Simulation Engine

**Status:** Typed appliance and connection configuration with an initial deterministic component engine  
**Scope:** YAML validation, local configuration API, frontend projection, behavior contracts, and reusable network, connector, and OT primitives

## Purpose

The Rust workspace provides the behavioral layer that the Svelte architecture
will eventually visualize. Each rendered appliance must resolve to a declared
`ComponentKind` and a reusable behavior family. Configuration should create
instances of those behaviors; labels must not select ad hoc code paths.

The engine models appliance-level decisions and explains forwarding, delivery,
observation, translation, process output, and denial effects. It is not intended
to reproduce vendor firmware or every byte exchanged by a production protocol.

## Current Implementation

| Area | Implemented baseline |
| --- | --- |
| Shared model | Stable component and port identifiers, appliance kinds, behavior families, Ethernet and IPv4 data, routes, services, and process events |
| Ethernet | Access and trunk VLAN admission, source-MAC learning, known unicast, and unknown or broadcast flooding |
| Routing | Static longest-prefix selection, route metrics, TTL decrement, and explicit no-route or TTL-expired results |
| NAT | Stateful PAT for TCP, UDP, and ICMP identifiers; reverse translation; and one-to-one static NAT |
| Firewall | Ordered zone and address rules, protocol and destination-port matching, simplified bidirectional session state, routing, and default deny |
| Appliance links | Transparent forwarding, operational failure, modeled encryption traversal, deterministic delay, and deterministic loss |
| Typed ports | Appliance capability, port hardware, administrative and initial operational state, configured speed, duplex, MTU, logical mode, addressing, and VLAN metadata |
| Physical media | Separate copper, fiber, radio, carrier, virtual, field-wiring, and telephone modules with type-specific validation, capacity limits, physical facts, and propagation delay |
| Typed connectors | Endpoint direction, combined port/link state, effective MTU, negotiated duplex, capacity serialization delay, fixed and physical latency, deterministic loss, and transport/medium validation |
| Services | Explicit service acceptance, ICMP echo response, authoritative test-record DNS responses, and operational state |
| Web gateway | HTTP redirect, published-host validation, method and body limits, path rejection, and an abstract upstream-forward effect |
| Monitoring | Passive frame observation without forwarding |
| OT control | Periodic virtual-controller scans using deterministic provisional rules |
| HMI | Allowed-tag command submission and process-state observation |
| Distributed I/O | Declared input and output channels, channel validation, and output effects |
| Field devices | Scaled sensor samples, actuator commands, failures, and safe-state handling |
| Safety interface | Required permissives, latched trips, safe denial, and authorized reset |
| Runtime | Deterministic event queue, links, delayed delivery, trace records, and event limits |

The workspace currently contains 35 appliance kinds and 43 rendered-role
contracts. The manually maintained coverage register records a Rust kind for
every currently identified rendered appliance role, and tests ensure those
kinds exist in the catalog. It cannot independently discover Svelte inventory
drift and does not prove that every node is instantiated in a running topology.

The configuration repositories discover 160 per-appliance and 205
per-connection YAML documents. They dispatch appliance behavior, validate
render bindings, resolve connection endpoints and ports, enforce appliance
port capabilities, port-to-medium compatibility, endpoint speed and medium
capacity limits, and exclusive point-to-point ports, and generate the Svelte
catalog. Every appliance participates in at least one connection. This
improves topology coverage but does not yet construct the complete simulation
graph.

The parsed records and the architecture they describe are provisional
development placeholders. Validation proves the implemented structural rules,
not that the current values or topology are complete. The simulator will be
used to expose the requirements needed to replace those placeholders.

`hearthline-api` provides localhost-only, revision-checked editing. Candidate
source is validated in memory against both repositories before atomic source
and generated-catalog replacement.

## Event Model

```text
SimulationEvent
  |-- Network ingress
  |-- Process event
  `-- Operational-state change
           |
           v
SimulatedComponent::handle
           |
           +-- Transmit
           +-- Deliver
           +-- Application forward
           +-- Observe
           +-- Process effect
           `-- Drop with reason
```

The simulator records each effect against simulation time and component ID.
Delayed link events use a stable sequence number so identical input produces
the same event order.

## Fidelity Boundary

The current engine does not yet implement:

- Simulated component construction or complete graph assembly from the parsed
  appliance configuration.
- Full cross-file validation for addressing, VLAN consistency, routes, NAT,
  firewall policy, services, and HA relationships.
- The complete Hearthline topology or end-to-end project scenarios.
- ARP cache behavior, address resolution, or router-generated ICMP errors.
- TCP sequence numbers, retransmission, handshake validation, or session
  timeout behavior.
- NAT and firewall state expiration, synchronization, or HA failover.
- Dynamic routing, VRFs, IPv6, multicast routing, or policy-based routing.
- STP, RSTP, LACP, multi-chassis switching, or control-plane convergence.
- Bandwidth queues, congestion, jitter distributions, or stochastic loss.
- Copper electrical characteristics, fiber optical budgets, RF interference,
  connector loss, and autonegotiation or collision timing.
- Cryptographic VPN, TLS, certificate, or DNSSEC processing.
- Recursive DNS resolution or delegation.
- A real HTTP parser or production WAF rule language.
- Service-specific behavior for DHCP, PKI, identity, voice, historian,
  monitoring, printing, or managed transfer beyond explicit service
  acceptance.
- IEC 61131-3 parsing or production-equivalent virtual PLC execution.
- A ceramics plant model, material state, process dynamics, or area-specific
  control programs.
- Automatic propagation of process effects between sensors, remote I/O,
  controllers, HMIs, safety interfaces, actuators, and the future plant model.
- Functional-safety, burner-management, deterministic timing, or deployment
  certification.

These omissions are explicit engineering work, not behavior inferred by the
Svelte diagrams.

## Commands

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p hearthline-cli -- catalog
cargo run -p hearthline-cli -- coverage
cargo run -p hearthline-cli -- demo
cargo run -p hearthline-cli -- config-validate
cargo run -p hearthline-cli -- config-generate
```

`catalog` lists appliance kinds and behavior families. `coverage` lists the
current rendered-role mappings. `demo` runs a small deterministic forwarding
and HTTPS-delivery scenario; it is not an end-to-end Hearthline test.
`config-validate` parses and cross-validates all appliance and connection YAML.
`config-generate` validates the same repositories and atomically emits the
Svelte configuration catalog. `cargo run -p hearthline-api` starts the local
validated editing service.

## Next Milestones

1. Implement formal device-to-device communication through configured ports
   and typed media, with deterministic transit and drop traces.
2. Construct the Customer LAN and Customer Edge as the first executable
   communication path.
3. Extend cross-file validation with address, VLAN, service, NAT, policy, HA,
   and process-reference rules.
4. Replace the manual role coverage register with component instances
   constructed from canonical configuration.
5. Add positive and negative scenarios with exact trace assertions.
6. Connect Rust-generated component and scenario output to Svelte.
7. Extend protocol and service fidelity only where a documented scenario
   requires it.
8. Replace provisional configuration and architecture content with values and
   relationships proven by executable scenarios.
9. Add the IEC 61131-3 and plant-model layers after network configuration and
   scenario execution are stable.
