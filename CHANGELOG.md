# Changelog

All notable changes to Hearthline are recorded here. Releases follow
[Semantic Versioning](project/docs/reference/versioning.md). Hearthline remains in initial
development, so minor releases may include documented configuration or API
migrations.

## 0.3.0 - 2026-08-04

### Added

**Core Networking Runtime**

- YAML-driven runtime construction for endpoints, switches, routers, NAT
  routers, and link appliances, including typed Ethernet transport (queueing,
  serialization, propagation, MTU, port state, deterministic loss) and an
  allocation-free Layer 3 switch runtime combining VLAN-scoped MAC learning
  with routed SVIs.
- Host and routed-interface ARP resolution (neighbor aging, pending-packet
  release, Layer 2 destination checks) and endpoint-originated IPv4 traffic.
- Static-NAT proxy ARP and bounded PAT mappings with timeout and
  remote-endpoint validation.
- Typed `web-server` and `svi` appliance support, plus dedicated Business IT
  DNS and employee-portal appliances, with one-SVI-per-VLAN enforcement.

**Layer 2/3 Redundancy & High Availability**

- Allocation-free VRRP virtual identities with active/standby validation,
  split-brain rejection, and a Business IT recovery path transferring VLAN
  20/30/80 gateways from Core-01 to Core-02.
- YAML-configured Rapid-PVST bridge priorities with a deterministic per-VLAN
  root-election solver (IEEE long path costs, port roles, forwarding or
  discarding state) and per-VLAN enforcement in the switch runtime, including
  CAM flushing and Svelte inspection.
- A data-carrying Business IT core peer trunk and YAML-configured
  LACP/multi-chassis domain with shared system identity, deterministic flow
  selection, bundle-aware failover, and split-horizon handling.
- A first-hop role editor and failover capture views in the simulation
  workspace.

**Firewall High Availability**

- YAML-driven stateful-firewall construction (routed interfaces, zones,
  static routes, address/service rules, sessions, denial outcomes) and
  reciprocal northbound-firewall HA with a dedicated sync link, shared
  virtual identities, and active/standby enforcement with session-table
  replication.
- A protocol-timed continuity scenario (HA heartbeat, hold-timer promotion,
  gratuitous announcements, reverse-ACK validation) plus four fault variants
  — HA-sync loss, standby session-state loss, stale-session expiry, and
  isolation fencing — each with dedicated evidence in the API and Svelte
  topology inspector.

**Scenario Catalog & Simulation Workspace**

- Versioned scenario YAML with repository/topology validation, editable
  packet overrides, deterministic report projection, and a local scenario
  catalog/execution API, surfaced through a responsive Svelte simulation
  workspace (packet composition, execution controls, trace filters,
  timing/outcome metrics) and a compact mobile scenario selector.
- End-to-end scenarios for customer DNS lookup, factory operations-data (OT
  DMZ to Central Office analytics), customer public-service delivery, and
  eight independent Business IT PC-01–04 DNS/HTTPS paths.
- Scenario-owned connection-state overrides, request-time fault injection,
  and a link editor, powering a customer WAN access-outage scenario with a
  YAML-declared recovery state and recovery command in the workspace.

**Endpoints, HMI & Process Interaction**

- A Rust-backed workstation profile and action contract (hostname,
  interface, gateway, resolver) with terminal `ipconfig`, `nslookup`,
  `curl`, `ssh` actions and structured browser navigation, exposed through
  enterable Customer PC-01/PC-02 and Business IT PC-01–04 desktops with
  independent addressing and per-action traces.
- A bounded HTTP document contract with configuration-owned content, routed
  reverse-proxy requests, and browser rendering of returned service pages.
- Rust-backed HMI sessions for all ten process areas (configured sensor
  values, startup safety, alarm acknowledgement, actuator state, operator
  audit) with four-stage HMI-to-actuator command traces and repository
  validation for field samples, command ownership, and remote-I/O mappings.
- A composite factory-autonomy scenario disabling inter-site handoffs while
  an independently evaluated local control chain retains operation, resets a
  safety circuit, and executes a pump command, with fault-aware continuity
  evidence in the workspace.

**Application Security & SOC**

- Method-aware workstation `curl` handling (GET/HEAD/POST/PUT/PATCH/DELETE/
  OPTIONS) with bounded quoted arguments and request-data flags (`-d`,
  `--data`, `--data-raw`), including POST inference and body-size evidence.
- Configuration-owned gateway inspection rules (path/body targets, patterns,
  stable rule IDs) enforcing three controlled Customer PC-01 security
  exercises — path traversal, a disallowed method, and SQL injection —
  prevented by the DMZ reverse-proxy WAF.
- Trace-derived security evidence, a bounded API session store, and an
  enterable Central SOC console with queue filtering, event review,
  acknowledgement, and clearing.

**Testing, CI & Tooling**

- CI gates enforcing the 500-line file limit, seven-entry folder limit,
  production/test separation, allocator-free runtime boundary, standalone
  `no_std` compilation, formatting, strict Clippy, integration tests,
  configuration/catalog validation, release-version consistency, Svelte
  diagnostics/builds, bounded fuzzing, and release benchmarks.
- External integration suites covering model contracts, configuration
  parsing, connectors, routing, switching, services, firewall state, NAT,
  industrial control, and media compatibility, plus fuzz targets and
  benchmarks for identifiers, appliance YAML, longest-prefix routing, and
  service traversal.

### Changed

- Reorganized packages, project inputs, documentation, scripts, and
  standards so every repository folder satisfies the enforced fan-out limit;
  isolated filesystem/YAML host adapters in `hearthline-config`.
- Converted the deterministic model and engine crates to `no_std`
  fixed-capacity storage with a borrowed 192-component simulator registry.
- Replaced the switch forwarding shortcut with VLAN-scoped CAM learning and
  aging (Layer 2 switches intentionally hold no ARP table), added timed
  firewall sessions with TCP-opener validation and interface MTU
  enforcement, and moved reverse-proxy HTTP method/inspection policy from
  Rust constants into required, validated YAML.
- Progressively migrated appliance configuration (schema `0.3.0` →
  `0.9.0`), scenario configuration (→ `0.11.0`), scenario reports (→
  `0.14.0`), and the generated catalog (→ `0.8.0`) to support SVI/VRRP
  identities, spanning-tree state, LACP/multi-chassis membership,
  firewall-HA contracts and heartbeat timers, HA fault/isolation evidence,
  and composite local-autonomy contracts.
- Hardened component-kind parsing so oversized unknown YAML values return a
  bounded validation error instead of panicking during deserialization.

### Planned

- Add changing process effects behind the ten-area HMI baseline, further Phase
  1 outage and failover simulations, and additional controlled Phase 3
  security paths.
- Replace the current provisional configuration and architecture placeholders
  with scenario-derived, cross-validated engineering definitions as the
  behavioral layers mature.

## 0.2.0 - 2026-07-29

### Added

- Rust workspace with shared models, deterministic appliance primitives,
  connector behavior, event scheduling, trace output, CLI validation, and a
  localhost configuration API.
- One validated YAML document for each of 160 appliances and 205 modeled
  connections.
- Typed appliance ports with hardware capability, administrative and initial
  operational state, speed, duplex, MTU, addressing, and VLAN configuration.
- Separate Rust physical-media modules for copper, fiber, radio, carrier,
  virtual links, field wiring, and telephone cabling.
- Medium validation for physical limits, capacity, endpoint compatibility,
  propagation delay, and exclusive point-to-point ownership.
- Appliance and connection configuration routes with complete YAML inspection,
  revision-checked editing, and whole-project validation.
- Physical and logical documentation captures for all 22 architecture routes.

### Changed

- Migrated appliance configuration to schema `0.3.0`.
- Migrated connection configuration to schema `0.2.0`.
- Moved port state, speed, duplex, and MTU ownership from connection records to
  appliance port records.
- Defined connection YAML as the authority for endpoint attachment and
  medium-specific physical configuration.
- Derived initial link state, effective MTU, duplex, serialization delay, and
  propagation delay in Rust.
- Corrected WLAN links to resolve as half duplex in the current model.

### Documentation

- Documented the port, connection, and physical-media ownership boundaries.
- Recorded the current fidelity limitations and deployment-conformance status.
- Established the project versioning and release procedure.

## 0.1.0 - 2026-07-26

### Added

- Initial map-first Svelte architecture viewer.
- Customer Network, Central Office, Factory, OT DMZ, and ten-stage ceramics
  process navigation.
- Initial physical and logical architecture documentation baseline.