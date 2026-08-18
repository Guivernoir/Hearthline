# Changelog

All notable changes to Hearthline are recorded here. Releases follow
[Semantic Versioning](project/docs/reference/versioning.md). Hearthline remains in initial
development, so minor releases may include documented configuration or API
migrations.

## 0.3.1 - 2026-08-17

### Added

- Added four Body Preparation trains for ceramic slip, fresh process water,
  segregated return-water recovery, and liquid glaze. Slip and glaze have local
  control cells; water has separate industrial-treatment,
  industrial-distribution, return-treatment, and return-pipeline HMI/vPLC/RIO
  scopes over one shared utilities access switch. Rust advances shared plant
  inventories, bounded quality, phase outputs, and release state.
- Added public-reference slip and glaze recipes, fresh-water treatment,
  `30-50%` sanitaryware return-water context, body/glaze return segregation,
  laboratory-style rheology and glaze checks, and explicit documentation of
  project assumptions and simulation limits.
- Expanded Body Preparation to 166 YAML-defined appliances, seven remote-I/O
  stations, 175 connection files, 85 live signals, 53 commanded actuators,
  six local safety interfaces, and dedicated physical, logical, train,
  handoff, recipe, quality, and diagnostic views.
- Added eight industrial- and return-water routes, 16 duty/standby pumps,
  `500 ms` pump heartbeats, `1,500 ms` stale detection, automatic healthy
  standby transfer, scoped heartbeat alarms, and pipeline-HMI maintenance
  dispatch. Direct temperature, pH, conductivity, turbidity, pressure, flow,
  and balance readings replace narrative-only water-condition summaries.
- Added four monitored material handoffs for water-to-slip, water-to-glaze,
  slip-to-forming, and glaze-to-glazing service. Paired pressure/flow, receive
  flow, entrained-air, and derived leak instruments feed Rust-owned line-loss,
  leak, delivery-quality, and downstream slip-quality behavior.
- Added a typed released-slip contract whose measured rheology updates all
  Forming sessions and carries bounded green-moisture, drying-shrinkage,
  drying-energy, green-strength, and fired-defect-risk indicators.
- Retained the validated Structured Text source and I/O binding for the slip
  train while documenting that all four Body Preparation trains remain
  Rust-owned until a broader controller-language target is selected.
- Added Body Preparation regression coverage for reference mass balance,
  independent train controls, Hold/Resume inventory retention, water and glaze
  completion, return-water reuse, quality trips, reset behavior, local control
  authority, pipeline leaks, and the released-slip handoff to Forming.
- Expanded Forming into four equal mould stations, each with one local HMI,
  local remote I/O, six process signals, a valve manifold, movement output,
  and safety interface. The cell also has an embedded machine-PC supervisory
  application and an independent robot pendant and safety interface.
- Added one machine-PC tab per mould and a live mould schematic to both the PC
  and each local HMI, including mould inclination, position, fill-head,
  pressure, temperature, moisture, selector, and active-circuit state.
- Added machine-PC object navigation for moulds, slip supply, production,
  trends, alarms, and audit records, plus a live robot-pendant view with
  retained manual motion commands.
- Added typed manual, auto, and authenticated setup selector state. Setup
  bypasses declared process-sensor permissives while retaining emergency-stop
  and hardwired-travel-limit protections.
- Added 28 range-checked mould parameters, three development recipe identities,
  shared station status, local-manual valve authorization, and explicit denial
  of machine-PC robot commands.
- Added mould-local Start, phase-boundary Stop, and cycle-boundary End controls,
  continuous production, independent mould phase state, and identified mould
  telemetry.
- Added regression coverage for independent phase offsets, continuous cycles,
  Stop/End boundaries, retained commands, selector authority, setup
  protections, scoped safety trips, telemetry, and machine-PC/robot separation.
- Added a first-class robot-controller appliance with six-axis motion-group
  metadata, controller/manipulator/pendant/safety boundaries, five user
  frames, tool and payload records, 17 taught positions, four separately
  assigned pickup and operator-handoff definitions, and a default bounded
  `.g` motion program.
- Added Rust-owned Cartesian and joint interpolation, workspace enforcement,
  jog and coordinate targets, pendant motion-enable authority, taught-position
  updates, program loading, single-line execution, and active-line reporting.
- Rebuilt the robot pendant with an always-live cell view, motion percentage,
  TCP and joint values, sequence commands, Cartesian and joint jog controls,
  taught positions, source editing/import, and executing-line highlighting.
- Added a bounded FIFO robot-cell arbiter. Concurrent mould requests now wait
  for exclusive robot ownership, and pickup/delivery PLC transitions remain
  held until the selected station's motion and handoff states complete.
- Added four complete mould-specific robot routines (`O0201` through `O0204`)
  to the canonical `.g` source. Automatic motion now executes those
  coordinates directly and reports the active assigned routine.
- Added pickup and operator-handoff geometry validation. A `.g` coordinate
  outside its YAML-defined translation or orientation tolerance stops the
  robot cell and raises a named trip alarm, with a negative-path regression
  test using a deliberately incorrect pickup coordinate.
- Bound all seven independently configured values per mould to the executing
  sequence and Rust plant dynamics. Fill, dwell, drain, pickup-delay, wash,
  and vacuum values override timer transitions; casting pressure drives the
  simulated pressure profile.
- Added one YAML-defined external control cabinet and one mould-embedded
  utility section per mould, including local I/O modules and independently
  addressed slip, compressed-air, water, vacuum, and hydraulic circuits with
  live state projection.
- Added a YAML-defined fenced robot cell with an interlocked personnel gate,
  dedicated guard remote I/O, and four mould-specific transfer stations with
  in-cell and operator-side position feedback.
- Added Rust-owned guard inhibition and trip behavior. Motion requested with
  the gate open is denied, opening the gate during motion stops affected state
  machines, and a latched trip can be reset only after the gate is closed.
- Added live machine-PC guarded-cell controls and transfer status, including
  gate state, motion permissive, alarm/reset state, travel progress, piece
  state, and both end-position sensors for each handoff.
- Added an object-based supervisory runtime with reusable templates, asset
  instances, quality-aware timestamped tags, bounded trends, alarm/event and
  operator-audit projection, role identity, repository revision state, and
  active/standby deployment nodes.
- A detailed YAML-defined ceramic-slip Forming cell with 84 components,
  object-based machine supervision, four mould HMIs, shared and mould-local
  remote I/O, external control cabinets, mould-embedded utility sections, and
  explicit field, motion, safety, and control-network connections.
- A deterministic Rust process model for mould filling, compressed-air
  pressure and dwell, excess-slip drainage, depressurization, sequential
  release water and air, robotic pickup and operator handoff, mould washing,
  air purging, vacuum drying, and mould closure.
- Shared Forming state across SCADA and module HMIs, a background process
  clock, automatic-cycle controls, scan and cycle counters, phase-aware output
  inhibition, and five injectable process disturbances with named alarms.
- Operator-interface signal scopes, a first-class `scada-workstation`
  component kind, and Rust tests for module visibility, shared process state,
  normal-cycle completion, fault handling, reset authorization, and separate
  safety latching.
- A catalog-driven Forming architecture view with separate ceramic-slip,
  pressure-casting, robotic-demoulding, and water/air/vacuum modules plus
  directional telemetry and command paths.
- A typed, allocator-free process-telemetry packet carrying a service,
  controller source, sequence, and bounded payload through the existing media,
  routing, and policy engine.
- An authorized Forming SCADA publication workflow that captures the current
  process snapshot, uses the OT DMZ historian replica as the network publisher,
  delivers to Central Office analytics, and displays payload, route metrics,
  outcome, and trace evidence.
- Automatic Forming telemetry collection from the addressed vPLC through its
  virtual host and Level 3 core into the factory-local historian, followed by
  policy-controlled replication through `OT FRW-01A` into the OT DMZ replica.
- A bounded API-session historian runtime with one-second samples, 60-record
  local and replica stores, a pending queue, 250-millisecond replication
  retries, unreplicated-eviction accounting, and fail-closed publication when
  no replicated record is available.
- A responsive SCADA historian panel showing both storage tiers, payload
  freshness, backlog and loss counters, collection and replication evidence,
  and the governed northbound publication route.
- A versioned Forming Structured Text source, explicit YAML I/O binding, and
  bounded parser/compiler that validates program declarations, task timing,
  sequence transitions, field tags, phase codes, and actuator states before
  configuration is accepted.
- Source-driven Forming vPLC scans with PLC-timer quantization, Rust-owned plant
  dynamics and trips, and regression coverage for start, fault, reset, and
  scan-boundary behavior.
- A responsive HMI control-source viewer and API document exposing the
  executing source, I/O binding, task, watchdog, current step, and revision.
- Interactive workstation `ping [-n COUNT] <host-or-ip>` diagnostics with
  optional DNS resolution, one dynamically constructed ICMP packet per probe,
  verified echo-reply delivery, packet-loss and timing output, and complete
  Rust network/media traces.
- Per-workstation, API-session DNS caches with a deterministic 60-second TTL,
  cache-aware browser, `curl`, `ping`, and SSH resolution, authoritative
  `nslookup` queries, and terminal `ipconfig /displaydns` and
  `ipconfig /flushdns` controls.
- Per-workstation interactive network sessions that reuse one union of
  compatible baseline scenario appliances and media, advance monotonic
  simulated time, retain endpoint ARP and customer PAT state across actions,
  and normalize each returned trace to action-relative time.
- Structured workstation network-state reports, terminal `arp -a`, browser
  ARP/PAT details, and activity/status summaries backed by live simulator
  tables rather than inferred trace text.
- Capability-scoped runtime snapshots for switch CAM tables, endpoint and
  routed-neighbor caches, PAT translations, and stateful firewall sessions,
  including remaining lifetimes and regression-tested expiry.
- A responsive workstation Network State application with appliance
  selection, structured runtime tables, configuration links, and a bounded
  Rust-backed read-only console supporting `show status`,
  `show mac address-table`, `show arp`, `show ip nat translations`, and
  `show sessions` where applicable.

### Changed

- Reworked Body Preparation into a campus gateway with separately enterable
  Slip Preparation, Water Preparation and Distribution, and Glaze Preparation
  buildings. Each building now has its own local controls, scoped physical
  walkdown, complete logical view, owning remote I/O, field channels, and
  monitored inter-building handoffs.
- Preserved return-water treatment inside the utilities building while keeping
  body and glaze collection, storage, and reuse routes segregated. The detailed
  utilities view now separates four physical process/pump zones and four
  logical control authorities.
- Advanced the HMI API schema to `0.9.0` for runtime-bound mould setpoints,
  cabinet state, robot architecture and arbitration, typed supervisory
  objects, and Body Preparation process state.
- Reduced the Forming physical architecture to a top-down machine-floor view
  of four moulds with embedded utility sections, their external control
  cabinets and operator HMIs, four transfer stations, the fenced robot cell,
  and the operator-side robot controller, pendant, and machine PC. The logical
  view continues to show all 84 configured control, I/O, field, and safety
  components and their relationships.
- Improved reduced-zoom Forming readability with larger physical labels,
  viewport-fixed physical and logical line legends, pattern-distinct network,
  I/O, safety, transfer, guard, utility, and gate paths, a horizontal guarded
  boundary label, and utility routing separated from the access-gate area.
- Moved production authority to each mould-local HMI while retaining
  supervisory, parameter, recipe, historian, and valve-service functions on
  the machine PC. The robot controller now arbitrates all four stations while
  the independent pendant retains manual/setup motion authority.
- Migrated appliance configuration to schema `0.10.0`, the generated catalog
  to `0.9.0`, and introduced the versioned HMI API that subsequent `0.3.1`
  work advances to `0.9.0`.
- Replaced the provisional dry-powder hydraulic-press representation with the
  intended ceramic-slip pressure-casting, robotic-demoulding, and mould-care
  workflow.
- Made the Body Preparation feed to the 40 C Forming slip tank explicit,
  separated demoulding-assistance water and air from post-handoff mould
  cleaning, and aligned the closed-mould position across YAML and Rust.
- Expanded deterministic process storage for the 24-channel Forming I/O
  station and updated the SCADA layout for sequence and fault supervision.
- Migrated scenario configuration to schema `0.12.0` and scenario reports to
  `0.15.0` for typed process telemetry.
- Gave addressed virtual PLCs the existing routed endpoint stack so they can
  originate IPv4 traffic while retaining their process scan and I/O behavior.
- Connected endpoint and DNS-server `respond_to_icmp` YAML policy to runtime
  behavior, added a typed `icmp-echo` delivery, and advanced the workstation
  action schema to `0.10.0` for explicit DNS-resolution provenance, retained
  structured network-state projection, and runtime inspection actions.

### Fixed

- Raised the embedded virtualization-host switch capacity from 16 to 32 ports
  so the expanded vPLC inventory does not panic the background historian
  scenario during API startup. A dense-control-host regression covers the
  current 20-port class.
- Corrected Body Preparation HMI responsiveness so all six page tabs remain
  visible, long panel headers wrap coherently, water-quality tables stay within
  their viewport, and diagnostic controls remain usable on narrow screens.
- Releasing pendant motion enable now stops active robot motion and pauses a
  running manual program, preventing the next runtime tick from reissuing the
  interrupted instruction.
- Guard interlock side effects now follow action validation and station
  authorization. An unauthorized motion-shaped request is denied without
  latching a guard trip or stopping cell state machines.
- Safety reset recovery is evaluated per mould, so one repaired mould can
  return to idle while another mould's local safety trip remains latched.
- Cancelling an active robot-cell request no longer increments the completed
  handoff counter. A guard-interrupted transfer now preserves its piece,
  progress, and travel direction when closed-gate reset permits recovery.
- Added an explicit `Clear fence alarm` control to the machine-PC and general
  safety workspaces. The control remains disabled, and the Rust reset remains
  denied, until the personnel gate is closed.

### Documentation

- Split Body Preparation documentation into a gateway README and dedicated
  Slip Preparation, Water Preparation and Distribution, and Glaze Preparation
  folders, each with current physical and logical captures.
- Replaced the representative Forming inventory with its current control/data
  flow, exact process sequence, implemented signal and output inventory,
  simulation boundary, engineering basis, fault coverage, and planned control
  integration.
- Documented the live Forming historian collection, DMZ replication,
  northbound analytics publication, operator audit, route evidence, and the
  remaining boundary to a production historian; refreshed the SCADA capture.
- Documented workstation DNS-cache lifetime, command behavior, session scope,
  persistent baseline-network ownership, ARP/PAT evidence, resilience-scenario
  exclusion, and the remaining boundary to broader interactive topology state.
- Documented the session-context CAM, neighbor, PAT, and firewall tables, the
  read-only simulator console, and the distinction between runtime
  instrumentation and privileged device management.

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
