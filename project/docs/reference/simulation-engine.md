# Rust Simulation Engine

**Status:** Allocator-free deterministic component engine with typed host-side configuration  
**Scope:** Behavior contracts, fixed-capacity runtime, YAML validation adapters, configured scenarios, local API execution, frontend projection, and reusable network, connector, and OT primitives

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
| Ethernet | Frame-length and source validation, access and trunk VLAN admission, VLAN-scoped CAM learning and aging, known unicast, unknown or broadcast flooding, and per-VLAN spanning-tree discarding |
| Spanning tree | YAML bridge priority and MAC identity, deterministic Rapid-PVST root election, speed-derived IEEE long path cost, per-VLAN root/designated/alternate/disabled roles, CAM flushing, and converged-state report projection |
| Link aggregation | YAML LACP system and group identity, active/passive negotiation checks, minimum links, deterministic per-flow member selection, bundle-aware CAM failover, reciprocal multi-chassis peers, bounded flooded-traffic split horizon, and converged-state report projection |
| Routing | Interface-scoped ARP and neighbor aging, pending-packet release, static longest-prefix selection, route metrics, Layer 2 rewrite, TTL decrement, MTU enforcement, and explicit diagnostics |
| NAT | Bounded and timed PAT for TCP, UDP, and ICMP identifiers; remote-endpoint-validated reverse translation; one-to-one static NAT; and proxy ARP |
| Firewall | Ordered zone and address rules, protocol and destination-port matching, TCP opener validation, protocol-specific session timeout, deterministic capacity handling, routing, default deny, media-carried HA heartbeat and session updates, hold-timer promotion, and first-hop announcements |
| Appliance links | Transparent forwarding, operational failure, modeled encryption traversal, deterministic delay, and deterministic loss |
| Typed ports | Appliance capability, port hardware, administrative and initial operational state, configured speed, duplex, MTU, logical mode, addressing, and VLAN metadata |
| Physical media | Separate copper, fiber, radio, carrier, virtual, field-wiring, and telephone modules with type-specific validation, capacity limits, physical facts, and propagation delay |
| Typed connectors | Actual frame transit, endpoint direction, combined port/link state, effective MTU, negotiated duplex, capacity serialization and queue delay, fixed and physical latency, deterministic loss, and transport/medium validation |
| Services | Explicit service acceptance, ICMP echo response, authoritative test-record DNS responses, configured bounded HTTP documents, typed bounded process telemetry, and operational state |
| Web gateway | HTTP redirect, published-host validation, configuration-owned method allowlists and path/body inspection rules, body limits, configured static routing, bounded request correlation, upstream request origination, and response relay |
| Monitoring | Passive frame observation without forwarding |
| OT control | Periodic virtual-controller scans, a bounded Structured Text sequence parser/runtime, explicit Forming I/O binding validation, and Rust-owned Forming plant dynamics and trips |
| Operator interface | HMI/SCADA allowed-tag command submission, scoped observation, shared Forming cell state, automatic-cycle control, fault injection, alarm handling, and audit history |
| Distributed I/O | Declared input and output channels, channel validation, and output effects |
| Field devices | Scaled sensor samples, actuator commands, failures, and safe-state handling |
| Safety interface | Required permissives, latched trips, safe denial, and authorized reset |
| Runtime | Borrowed registry for up to 192 components, deterministic fixed-capacity queues, 256 links, microsecond delivery, media-transit trace records, capacity failures, and event limits |
| Configured scenarios | Versioned YAML packet, baseline expectation, optional recovery, continuity, isolation, local-autonomy, security, and connection-state contracts; participant and topology validation; selected-subgraph construction; packet and link-state overrides where permitted; stable JSON report projection; and API execution |
| Security evidence | Trace-derived prevention or control-failure disposition, configured detector and defender ownership, bounded session retention, acknowledgement, and local API projection |

The `hearthline-model` and `hearthline-engine` crates compile independently as
`no_std` code and use fixed-capacity storage. Host allocation, filesystem
access, YAML parsing, generated projection, HTTP, and CLI concerns are isolated
in adapter crates. The workspace currently contains 37 appliance kinds and 44
rendered-role contracts. The manually maintained coverage register records a
Rust kind for every currently identified rendered appliance role, and external
integration tests ensure those kinds exist in the catalog. It cannot
independently discover Svelte inventory drift and does not prove that every
node is instantiated in a running topology.

The configuration repositories discover 185 per-appliance and 231
per-connection YAML documents. They dispatch appliance behavior, validate
render bindings, resolve connection endpoints and ports, enforce appliance
port capabilities, port-to-medium compatibility, endpoint speed and medium
capacity limits, and exclusive point-to-point ports, and generate the Svelte
catalog. Every appliance participates in at least one connection. This
improves topology coverage. The host adapter now constructs selected endpoint,
DNS, web-server, switch, router, NAT-router, stateful-firewall, web-gateway,
HMI, virtual-PLC, remote-I/O, field-device, safety-interface, and link
subgraphs directly from these records. Thirty versioned scenarios are
executable: independent Customer PC-01 and PC-02 variants cover public paths;
Business IT PC-01 through PC-04 each cover internal DNS and HTTPS through two
user-access switches and Core-01 SVIs; Factory variants cover approved and
denied operations data, Forming historian collection, and OT DMZ replication;
and three Customer PC-01 exercises cover WAF-denied
path traversal, a disallowed HTTP method, and a SQL-injection request body. A
customer availability scenario uses the same seven-appliance DNS topology with
its access circuit operationally down and declares a restored-state DNS
delivery expectation. Each customer DNS path uses seven appliances and six
links for ARP, switching, PAT, routing, authoritative response, reverse PAT,
and client delivery. A Business IT availability scenario uses six appliances
and seven links to prove normal Core-01 delivery and a recovered Core-02 path
after two primary LACP members fail and three VRRP groups change role. Both
physical members of each selected logical uplink distribute at baseline; only
the Core-02-side members distribute after recovery. Each
successful public-service path uses 16
appliances and 15 links to prove static destination NAT, both named HTTPS
policies, host and path validation, internal application delivery, HTTP
response relay, reverse NAT, and final client delivery. Each denial counterpart
uses 12 appliances and 11 links to prove default-denied public SSH. The factory
pair uses a separate seven-appliance, six-link subgraph to prove named HTTPS
delivery of a typed telemetry frame from the OT DMZ historian replica to
Central Office analytics and default-denied SSH at the northbound firewall.
Forming SCADA can replace the canonical payload with its current process phase,
controller scan sequence, and selected live measurements for one
request-scoped run. Each Business IT path uses five
appliances and four links to prove user VLAN access, Core-01 inter-VLAN routing,
internal DNS or HTTPS response, and return delivery. Complete-project graph
construction remains unfinished.

Forming is the first area-specific plant-process implementation. One shared
Rust session supplies its SCADA workstation and four module HMIs. A bounded
Structured Text sequence with 14 seconds of timer presets advances mould filling, pressurization and
dwell, excess-slip drainage, depressurization, sequential release water and
air, mould opening, robot pickup and operator handoff, mould wash, air purge,
vacuum drying, and mould closure. Seventeen measurements and six outputs change
with phase. Five injected disturbances produce bounded trips; mould
overpressure also latches the separate machine-safety state. Timer completion
is observed on 20-millisecond scan boundaries. This is a tested Hearthline
subset, not ceramic-process physics, complete IEC 61131-3 conformance, or a
vendor-equivalent runtime.

The factory local-autonomy scenario combines two independent execution roots.
Both factory-facing inter-site handoffs are down, and the historian HTTPS
request is therefore dropped at the redundant OT DMZ switching boundary. In
the same run, Rust resolves an operational seven-link local path for the Body
Preparation HMI, vPLC, remote I/O, safety interface, and pump. The HMI resets a
healthy latched safety circuit and commands the pump to `running`. Passing the
scenario requires both the northbound drop and the local control result. This
is a bounded command-path proof, not controller-program execution, process
dynamics, or a general factory availability claim.

The converged northbound-firewall availability scenario uses ten appliances and fourteen
links. FRW-03A owns both shared virtual addresses at baseline. Recovery
withdraws its three data links, restores FRW-03B's three data links, transfers
the firewall roles and virtual identities, and verifies the unchanged
historian HTTPS request through FRW-03B.
The recovered trace includes eight media drops from flooded copies attempted
over deliberately withdrawn redundant paths; the expected HTTPS flow still
delivers. This is not a zero-loss or seamless-failover claim.

The protocol-timed continuity variant establishes one TCP session through
FRW-03A, serializes its bounded session record and four heartbeats as Ethernet
payloads over the dedicated HA-sync fiber, fails FRW-03A, changes to the
declared converged B data path, and promotes FRW-03B after its `750 ms` hold
timer. FRW-03B emits gratuitous ARP announcements for both virtual identities
and permits the exact reverse ACK from synchronized state. The report records
the last heartbeat, promotion time, deterministic failure-to-promotion interval, synchronized
session count, and successful continuation. This is Hearthline's abstract,
deterministic HA protocol; it does not reproduce a vendor protocol, TCP
retransmission, or production failover behavior.

Two fault variants reuse that timed contract. The HA-sync-loss scenario drops
the dedicated medium at `600 ms`, after the session update and two periodic
heartbeats have reached FRW-03B. The standby promotes from its last received
heartbeat and permits the retained reverse flow. The session-state-loss
scenario clears the standby's replicated table at `800 ms` while heartbeat
monitoring remains current; FRW-03B promotes normally but rejects the reverse
ACK under default policy. These cases distinguish control-plane liveness from
data-plane state availability. They do not model bulk resynchronization,
state-version conflict, or arbitrary connection recovery.

The stale-session variant also loses HA synchronization at `600 ms` and
promotes FRW-03B with one retained session. It delays the reverse ACK until
`301 s`; the firewall then ages the entry beyond the modeled `300 s` idle TCP
timeout, emits the exact expiry transition, and rejects the packet under
default policy. The report preserves both the promotion-time and
post-continuation session counts.

The HA-isolation contract drops only the synchronization connection at
`600 ms`; FRW-03A and its data path remain operational. FRW-03B reaches the
`750 ms` hold threshold from its last heartbeat but receives no confirmed peer
failure, so the runtime records inhibited promotion and leaves both shared
first-hop identities on FRW-03A. The reverse ACK then completes through the
existing FRW-03A session. This is a fail-closed fencing abstraction, not a
quorum, witness, or arbitrary network-partition algorithm.

Each security exercise uses 16 selected appliances and 15 links, but execution
stops at `business-web-gw-01` when its Rust WAF behavior rejects the configured
path, method, or body. Scenario report schema `0.15.0` projects the active
baseline, recovery, continuity, isolation, or autonomy expectation, link,
gateway, LACP, spanning-tree, detector, security, and local-control evidence.
The API stores each event for the configured defender in a bounded local
session; it does not simulate a routed logging protocol, SIEM, or
alert-correlation pipeline.

The parsed records and the architecture they describe are provisional
development placeholders. Validation proves the implemented structural rules,
not that the current values or topology are complete. The simulator will be
used to expose the requirements needed to replace those placeholders.

`hearthline-api` provides localhost-only, revision-checked editing plus
scenario catalog and execution routes. Candidate configuration source is
validated in memory against both repositories before atomic source and
generated-catalog replacement. Scenario packet, selected-connection,
first-hop role, and firewall-HA role overrides are validated for one execution
and do not modify
canonical YAML.
The workstation adapter projects
configured endpoint identity and network settings, parses a bounded
shell-like argument grammar and browser URLs, carries supported `curl` method
and data options into HTTP packets, selects compatible security scenarios by
source, method, path, and body, and constructs repeated ICMP echo packets for
`ping` over a compatible validated route template. Echo handling honors each
endpoint or DNS server's YAML `respond_to_icmp` policy and returns a typed
delivery for reply verification. A deterministic workstation session retains
successful DNS answers for 60 seconds, independently by workstation. Browser,
`curl`, `ping`, and SSH resolution consult that cache; `nslookup` always runs
the authoritative configured DNS path. Reports distinguish `dns-query`,
`client-cache`, and `literal-address` resolution without moving network
decisions into Svelte. Each workstation session also constructs one union of
all compatible baseline scenarios for that source and reuses its mutable
appliances and links. Runs use monotonic absolute simulator time for table and
media timers, then normalize projected traces back to action-relative time.
Endpoint ARP, VLAN-scoped switch CAM, routed-neighbor, active PAT, and stateful
firewall-session tables are projected as typed snapshots with remaining
lifetimes and regression-tested expiry. Capability-gated inspection actions
format those same snapshots for a bounded read-only simulator console; they do
not model device authentication, management transport, or a vendor CLI.
Controlled fault, recovery, continuity, HA-isolation, and local-autonomy
contracts are rejected by this runner and retain their existing fresh-runtime
semantics.

## Event Model

```text
SimulationEvent
  |-- Network ingress
  |-- Firewall HA timer control
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

- Complete graph assembly or configured construction for every behavior family.
- Full cross-file validation for addressing, VLAN consistency, routes, NAT,
  firewall policy, services, and HA relationships.
- The complete Hearthline topology or broader IT, policy, HA, and process
  scenarios beyond the current DNS, public-service, and operations-data paths.
- Router-generated ICMP unreachable or time-exceeded messages,
  duplicate-address detection, or IPv6 neighbor discovery.
- TCP sequence numbers, retransmission, full handshake tracking, or congestion
  control.
- Vendor firewall clustering protocols, quorum, split-brain arbitration,
  bulk resynchronization, stale-state reconciliation, TCP retransmission,
  full connection recovery, or
  measured wall-clock RTO/RPO. The implemented HA protocol deterministically
  validates one media-synchronized session and reverse ACK after timer-based
  promotion.
- Timed VRRP advertisements, election, preemption delay, or recovery-time
  measurement; the current core scenario applies validated converged roles.
- Dynamic routing, VRFs, IPv6, multicast routing, or policy-based routing.
- BPDU or LACP exchange, Rapid-PVST proposal/agreement timers,
  topology-change propagation, measured convergence, MSTP, vendor-specific
  multi-chassis peer protocols, synchronized MAC tables, or control-plane
  convergence. The implemented layers evaluate deterministic converged
  Rapid-PVST, LACP member, and split-horizon snapshots.
- Shared buffer limits, congestion control, jitter distributions, or
  stochastic loss.
- Copper electrical characteristics, fiber optical budgets, RF interference,
  connector loss, and autonegotiation or collision timing.
- Cryptographic VPN, TLS, certificate, or DNSSEC processing.
- Recursive DNS resolution or delegation.
- A real HTTP parser or production WAF rule language.
- General threat emulation, payload execution, host compromise, persistence,
  privilege escalation, or a production detection and response stack.
- Service-specific behavior for DHCP, PKI, identity, voice, historian,
  monitoring, printing, or managed transfer beyond explicit service
  acceptance.
- Durable historian persistence, telemetry subscriptions, authentication,
  encryption, or production OPC UA, MQTT, Sparkplug, or vendor historian
  protocols. The current Forming workflow samples compact typed records into
  two bounded API-session stores and retries only the modeled DMZ path.
- General IEC 61131-3 parsing or production-equivalent virtual PLC execution;
  only the declared Forming sequence subset is executable.
- Plant models, material state, and area-specific dynamics beyond the bounded
  Forming sequence.
- Area-specific control programs beyond Forming or production-equivalent
  virtual PLC task execution.
- General propagation of process effects between sensors, remote I/O,
  controllers, HMIs, safety interfaces, and actuators outside the Forming
  session adapter.
- Functional-safety, burner-management, deterministic timing, or deployment
  certification.

These omissions are explicit engineering work, not behavior inferred by the
Svelte diagrams.

## Commands

```bash
node project/scripts/repository-policy.mjs
cargo check --manifest-path packages/Cargo.toml -p hearthline-model
cargo check --manifest-path packages/Cargo.toml -p hearthline-engine
cargo fmt --manifest-path packages/Cargo.toml --all --check
cargo test --manifest-path packages/Cargo.toml --workspace --all-features
cargo clippy --manifest-path packages/Cargo.toml --workspace --all-targets --all-features -- -D warnings
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- catalog
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- coverage
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- demo
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- config-demo
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run customer-dns-lookup
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run customer-public-web-request
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run customer-public-web-management-denied
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run customer-public-web-path-traversal-detected
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run customer-public-web-method-denied
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run factory-operations-data
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run factory-operations-data-denied
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run factory-local-autonomy-conduit-outage
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run business-northbound-firewall-session-continuity
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- scenario-run business-northbound-firewall-isolation-fenced
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- config-validate
cargo run --manifest-path packages/Cargo.toml -p hearthline-cli -- config-generate
cargo bench --manifest-path packages/Cargo.toml --workspace --all-features
```

`catalog` lists appliance kinds and behavior families. `coverage` lists the
current rendered-role mappings. `demo` runs a hand-built deterministic
forwarding and HTTPS-delivery scenario. `config-demo` constructs the first
Customer LAN path from canonical appliance and connection YAML and runs ARP
plus ICMP across it. `scenario-run` executes a versioned configured scenario
and prints its projected trace. `config-validate` parses and cross-validates
all appliance, connection, and scenario YAML.
`config-generate` validates the same repositories and atomically emits the
Svelte configuration catalog. The `hearthline-api` package starts the local
validated editing and scenario-execution service. `GET /api/simulations`
returns the scenario catalog and `POST /api/simulations/{id}/run` executes a
canonical or packet-overridden scenario. CI also executes bounded identifier
and appliance-YAML fuzz campaigns. `GET /api/workstations/{id}` returns an
eligible endpoint profile; `POST /api/workstations/{id}/actions` executes a
supported terminal command, browser navigation, or read-only runtime
inspection. Terminal actions currently
include local identity/configuration, DNS, bounded repeated ICMP echo, HTTPS,
controlled SSH attempts, DNS-cache inspection and flushing, and `arp -a`. The
API advances each workstation's DNS cache and compatible network runtime on its
local 250-millisecond clock; configuration edits and process restart clear both.
The action report projects endpoint ARP and aggregate PAT summaries plus typed
per-appliance CAM, neighbor, PAT, and firewall-session tables. The workstation
schema is `0.10.0`; supported runtime `show` commands are capability-gated and
operate only on that workstation session's compatible baseline topology.
Interactive ICMP requires an existing configured flow to the
destination as its validated topology template; it is not yet an arbitrary
route-discovery engine. Runtime mutation, connector inspection, privileged
management protocols, and arbitrary topology discovery remain planned.
`GET /api/hmis/{id}` returns a configured HMI or SCADA snapshot, while
`POST /api/hmis/{id}/actions` executes permission, safety-reset,
acknowledgement, actuator-command, Forming-cycle, and Forming-fault behavior
against persistent shared cell state. A background API task advances active
Forming sessions while any authorized interface may observe them.
`GET /api/hmis/{id}/historian` is restricted to the authorized Forming SCADA
and returns bounded Level 3 and OT DMZ stores, pending and dropped counts, and
the latest collection, replication, and publication reports. The background
task samples once per simulated second and retries one pending replication
every 250 milliseconds.
`POST /api/hmis/{id}/telemetry` is restricted to an explicitly permitted SCADA
session. It requires a DMZ replica record, retargets that retained source,
sequence, and payload to the canonical historian-replica-to-analytics
scenario, and returns the standard scenario report.
`GET /api/hmis/{id}/program` returns the validated Forming Structured Text and
I/O-binding documents plus their task identity and combined source revision.
`GET /api/security/consoles/{id}` returns the current modeled event queue;
event acknowledgement and queue clearing affect only the running local API
session.

## Next Milestones

1. Extend the Forming control contract with reviewed process-condition
   transitions, recipes, retries, quality handling, and cross-area slip-tank
   replenishment while preserving the Rust plant/control boundary.
2. Add further exact positive, negative, failover, isolation, and outage
   scenarios for
   routing, NAT, policy, and services.
3. Extend cross-file validation with address, VLAN, service, NAT, policy, HA,
   and process-reference rules.
4. Replace the manual role coverage register with component instances
   constructed from canonical configuration.
5. Expand configured construction to the remaining behavior families as their
   placeholder YAML is replaced with executable values.
6. Extend process-state transitions and shared HMI behavior to the remaining
   areas as executable industrial models become available.
7. Extend protocol and service fidelity only where a documented scenario
   requires it.
8. Replace provisional configuration and architecture content with values and
   relationships proven by executable scenarios.
9. Select a formal IEC 61131-3 compatibility target before extending the
   bounded Structured Text subset or adding a Ladder interchange format.
10. Extend the controlled WAF baseline with additional offensive and
    defensive paths only where the relevant interactive and policy baselines
    are deterministic and tested.
