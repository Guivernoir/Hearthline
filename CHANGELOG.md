# Changelog

All notable changes to Hearthline are recorded here. Releases follow
[Semantic Versioning](project/docs/reference/versioning.md). Hearthline remains in initial
development, so minor releases may include documented configuration or API
migrations.

## Unreleased

### Added

- Continuous integration gates for the recursive 500-line file limit,
  seven-entry folder limit, production/test separation, allocator-free runtime
  boundary, standalone `no_std` compilation, Rust formatting, strict Clippy,
  integration tests, canonical configuration and generated-catalog validation,
  release-version consistency, Svelte diagnostics and builds, bounded fuzzing,
  and release benchmarks.
- External integration suites for model contracts, configuration parsing,
  connectors, routing, switching, services, firewall state, NAT, industrial
  control, media compatibility, and end-to-end event traversal.
- Fuzz targets for identifiers and appliance YAML plus benchmarks for
  longest-prefix routing and transparent-link service traversal.

### Changed

- Reorganized packages, project inputs, documentation, scripts, and standards
  so every repository folder satisfies the enforced fan-out limit.
- Converted deterministic model and engine crates to `no_std` fixed-capacity
  storage and a borrowed 192-component simulator registry.
- Isolated filesystem and YAML host adapters in `hearthline-config`.

### Planned

- Implement formal device-to-device communication through the typed media
  layer, beginning with executable Customer LAN and Customer Edge paths.
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
