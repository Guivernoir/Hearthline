# Deployment Conformance Review

**Review date:** 2026-07-30
**Scope:** Current Svelte architecture, documentation, bootstrap process model,
typed appliance and connection YAML, and initial Rust component engine

## Conclusion

Hearthline is a production-shaped reference architecture, not a deployable or
certified industrial design.

The site hierarchy, IT/OT separation, factory-local OT DMZ, controlled remote
access, brokered data exchange, independent policy boundaries, segmented
process areas, and corrected vPLC model are consistent with common real-world
deployment patterns.

The project does not yet prove production conformance because the implemented
appliance YAML is an initial structural baseline rather than a complete
deployable configuration. Product selection, detailed policy, failure-domain
engineering, environmental qualification, safety analysis, complete Rust
topology validation, virtual PLC execution, and site acceptance tests remain
incomplete.

The current architecture and configuration values are deliberately
provisional placeholders. Their consistency with common deployment patterns is
useful for simulation development, but it does not make the represented
topology, addressing, equipment, policy, redundancy, or physical layout a
finished design.

## Review Basis

The architecture was reviewed against:

- [NIST SP 800-82 Rev. 3, Guide to Operational Technology Security](https://csrc.nist.gov/pubs/sp/800/82/r3/final)
- [NIST SP 800-207, Zero Trust Architecture](https://csrc.nist.gov/pubs/sp/800/207/final)
- [NIST SP 1800-35, Implementing a Zero Trust Architecture](https://csrc.nist.gov/pubs/sp/1800/35/final)
- [CISA, Primary Mitigations to Reduce Cyber Threats to Operational Technology](https://www.cisa.gov/sites/default/files/2025-05/fact-sheet-primary-mitigations-to-reduce-cyber-threats-to-operational-technology-508c.pdf)
- [ISA/IEC 62443 series overview](https://www.isa.org/standards-and-publications/isa-standards/isa-iec-62443-series-of-standards)
- [Siemens, Network concept for redundant connections of virtual PLCs](https://support.industry.siemens.com/cs/attachments/109955831/109955831_Netzwerkkonzept_vPLC_DOC_V1_0_en.pdf)
- [CODESYS Virtual Control SL](https://www.codesys.com/products/runtime/virtual-control-sl/)
- [IETF RFC 5737, IPv4 Address Blocks Reserved for Documentation](https://datatracker.ietf.org/doc/html/rfc5737)

Vendor references establish realistic virtual-controller deployment patterns;
they do not select a Hearthline vendor or prove interoperability.

## Architecture Findings

| Domain | Status | Finding |
| --- | --- | --- |
| Regional model | Aligned target | Customer, Central Office, and Factory responsibilities are clearly separated |
| Customer network | Aligned example | Appropriate as an independently managed, untrusted source; it is not intended as an ISP design |
| Public addressing | Corrected | All public examples now use RFC 5737 documentation blocks |
| Enterprise Internet edge | Corrected target | Diverse access circuits and grouped redundant edge, firewall, switching, and web-gateway roles are represented |
| Public application path | Aligned target | Public traffic terminates on a reverse-proxy/WAF tier; internal application and data dependencies are separately governed |
| Business IT | Partially executable | Security zones, guest isolation, privileged administration, dedicated time, and redundant roles are represented; PC-01 through PC-04 execute internal DNS and HTTPS through two access switches, one recovery transfers the VLAN 20/30/80 virtual gateways and LACP forwarding to Core-02, and the northbound pair supports converged recovery, one protocol-timed media-synchronized TCP continuity case, HA-sync loss after state replication, fail-closed standby-state loss, idle-session expiry, and fenced sync-path isolation; vendor peer behavior, detailed enforcement, quorum, and broad session recovery remain pending |
| Zero trust | Conceptually aligned | Identity, device posture, policy decisions, and resource-side enforcement are represented; actual PE, PA, PEP, and policy data are pending |
| Operations Intelligence | Aligned target | Governance, monitoring, analytics, and approved changes do not create direct controller paths |
| OT DMZ | Aligned target | Independent IT-side and OT-side enforcement, access, exchange, monitoring, jump, replica, and transfer roles are present |
| vPLC platform | Corrected target | Physical compute hosts are distinct from logical vPLC workloads; cell separation extends to the host |
| Process I/O | Corrected target | Distributed I/O is local to each cell; sensors and actuators no longer appear physically connected directly to software |
| Safety and burner management | Correct boundary | Safety/status interfaces are separate from ordinary process control; no certification claim is made |
| Availability | Partially executable | Customer access-circuit restoration, a converged Business IT Core-02 gateway plus Rapid-PVST recovery, a converged northbound-firewall A-to-B path, one deterministic heartbeat/session continuity run, retained-state behavior after HA-sync loss, fail-closed behavior after standby-state loss, deterministic idle-session expiry, and single-active fencing under unconfirmed peer isolation are executable; BPDU convergence, vendor HA protocols, wall-clock RTO/RPO, quorum, arbitrary split-brain arbitration, bulk state reconciliation, and spares remain pending |
| Physical environment | Requirement recorded | Dust, heat, vibration, electromagnetic conditions, enclosure, cooling, power, and UPS engineering remain site-specific |
| Implementation evidence | Partial | Rust parses and cross-validates 222 appliance, 271 connection, and 30 scenario YAML files; independent customer public paths, Business IT PC-01 through PC-04 internal DNS and HTTPS, Business IT core recovery, converged and timed northbound-firewall recovery with sync-loss, state-loss, stale-session, and fenced-isolation variants, typed factory operations-data with Forming collection and DMZ replication, three WAF-prevented paths, one customer access-circuit restoration, and one composite factory conduit-outage/local-command case execute with forwarding, delivery, denial, or control results; six workstations expose repeated ICMP diagnostics, selected application behavior, isolated DNS caches, and compatible persistent baseline runtimes with tested ARP/PAT retention, while resilience scenarios remain fresh; 15 HMI sessions invoke selected Rust control behavior, Forming executes four independently controlled instances of one bounded Structured Text sequence against Rust plant dynamics and mould-specific setpoints, the dedicated robot controller arbitrates mould pickup and handoff, and the machine PC exposes object-based supervisory state, control source, bounded historian tiers, and the brokered analytics path, while durable historian persistence, complete topology evaluation, general IEC 61131-3 execution, broader plant state, protocol-specific HA, and broader process scenarios remain unimplemented |
| Security operations evidence | Partial | Controlled Customer PC-01 traversal, disallowed-method, and SQL-injection request-body probes produce distinct trace-derived WAF evidence in a bounded, filterable Central SOC session with acknowledgement; this is not a routed telemetry pipeline, SIEM, production WAF, incident-response platform, or proof of broad defensive coverage |
| Operator interface evidence | Partial | All ten process areas expose Rust-backed operator sessions with YAML-derived instruments, permissions, safety permissives, actuator state domains, alarm acknowledgement, and operator-interface-to-vPLC-to-remote-I/O-to-actuator command traces; Forming adds one shared machine PC, four equal mould panels, and a robot-pendant runtime with selector authority, authenticated setup sensor bypass that retains declared hardwired protections, retained manual commands, mould-local Start/Stop/End, runtime-bound mould setpoints, object-scoped PC navigation, live mould and robot views, four independent instances of a validated bounded Structured Text sequence, a FIFO robot-cell arbiter, mould-specific frames and handoffs, per-mould cabinets, an object-based supervisory model, explicit I/O binding, a 20-millisecond task, live measurements, five process disturbances, scoped safety trips, source inspection, bounded historian acquisition and DMZ replication, and authorized replica publication with route evidence. This is not evidence of durable historian persistence, industrial subscription protocols, production robot planning, general IEC 61131-3 conformance, deterministic field-network timing, functional-safety behavior, or production plant-process fidelity |

## vPLC Placement Decision

The selected reference pattern is:

```text
Factory Level 3 control compute
  OT-vPLC-HOST-01/02
      |
      +-- isolated OT-AREA-01 runtime and cell network
      +-- isolated OT-AREA-02 runtime and cell network
      +-- ...
      `-- isolated OT-AREA-10 runtime and cell network

Each automation cell
  Area switch
      |-- Local HMI
      `-- Distributed I/O
             |-- Sensors
             `-- Actuators
```

`AREA-xx-vPLC-01` is a logical real-time workload. It is not drawn as a
standalone physical controller. `OT-vPLC-HOST-01/02` represents the physical
factory-local compute platform. `AREA-xx-RIO-01` represents the physical
cell-local I/O station.

This pattern follows real deployments where virtual controllers run on
industrial edge devices or server/hypervisor infrastructure while network
separation continues from the automation cell to the assigned runtime.
Distributed I/O remains in the cell.

Before implementation, the selected platform must demonstrate:

- Declared real-time scheduling and CPU reservation.
- Dedicated or isolated control-network interfaces.
- Cell-specific VLAN or equivalent isolation through the host.
- Deterministic latency and jitter within process requirements.
- Defined host, network, storage, and management failure behavior.
- Controller state, restart, failover, and I/O reconnection behavior.
- Local operation without Central Office availability.
- A licensing and management design that does not introduce an unacceptable
  runtime dependency.

## Required Work Before Deployment Claims

1. Complete the asset, zone, conduit, interface, and service inventory.
2. Select products and map every abstract HA role to actual failure domains.
3. Define security levels or target security requirements from a documented
   risk assessment.
4. Implement identity, device, policy-engine, policy-administrator, and
   enforcement-point records.
5. Define every allowed IT, DMZ, OT, and cross-area flow.
6. Establish PKI, AAA, time, logging, backup, recovery, vulnerability, and
   change-management designs.
7. Define RTO, RPO, safe-state, degraded-mode, quorum, and critical-spares
   requirements.
8. Complete environmental and electrical engineering for the factory.
9. Perform process hazard analysis and independent safety engineering.
10. Extend the implemented YAML validation to complete topology, address, VLAN,
    route, NAT, policy, and control-source references.
11. Replace provisional configuration and architecture placeholders with
    reviewed definitions supported by requirements and executable scenarios.
12. Execute positive, negative, failover, isolation, and recovery scenarios.
13. Validate the selected virtual PLC runtime with representative control and
    I/O timing.
14. Test the final design in an appropriate emulator, integration lab, and
    staged site acceptance process.

## Claims Boundary

Hearthline may claim that the current architecture follows recognized
segmentation, remote-access, zero-trust, and virtual-controller deployment
patterns.

It must not claim:

- Compliance or certification to ISA/IEC 62443.
- Functional-safety or burner-management certification.
- Product interoperability.
- Deterministic process performance.
- High availability solely because paired icons are present.
- Deployment readiness before configuration and scenario validation exist.
