# Hearthline Documentation

The documentation follows the same hierarchy as the Svelte application. Each
folder represents a documented application view and contains a local
`README.md`, a physical `screenshot.png`, and a logical
`logical-screenshot.png`.

![Hearthline regional architecture](screenshot.png)

![Hearthline regional logical architecture](logical-screenshot.png)

## Current Coverage

The documentation currently covers all 22 documented application views with
matching physical and logical captures. It documents the rendered Svelte model,
the parsed appliance and connection YAML baseline, the bootstrap process
contract, and target engineering requirements. Captures and supporting
evidence track development release `0.3.0`; the Forming SCADA capture was
refreshed on 2026-08-09. Reference pages also document the
30 current configured scenarios, including Business IT internal
service paths, deterministic Core-02 gateway and Rapid-PVST recovery,
converged and protocol-timed northbound-firewall recovery, and the customer
access-circuit outage and restoration expectation. The set also includes one
composite factory case that proves the expected conduit-path drop and an
independent local HMI-to-pump command path, plus Forming historian collection
and OT DMZ replication. Complete topology,
controller-program, plant-process, and vendor-protocol HA validation do not
yet exist.

Customer LAN documentation also includes a representative interactive
appliance capture. Both customer PCs expose the same class of endpoint session
through independent configuration and scenario records, including repeated
Rust-backed ICMP probes, isolated 60-second DNS client caches, and retained
baseline-network ARP/PAT state. The capture shows a cached storefront request
with the live session counts. This endpoint view supplements the physical and
logical architecture captures without changing the count of documented
architecture routes.

Business IT documentation includes an additional workstation capture showing
PC-01 rendering the internal employee portal after Rust executes its configured
DNS and HTTPS paths. PC-02 through PC-04 expose independent paths and DNS-cache
state with the same interaction contract across both user-access switches. The
capture also records the expected gateway ARP entry and zero internal PAT.

Business IT also includes a recovery-workflow capture showing Core-01 uplink
failure, VLAN 20/30/80 VRRP role transfer, Rapid-PVST port-role evidence, a
passing recovery expectation, and Core-02 forwarding. It records a
deterministic converged-state simulation rather than protocol election timing
or production HA behavior.

Customer Edge includes a recovery-workflow capture showing the restored
customer access circuit, Rust-selected recovery expectation, zero-drop DNS
run, and deterministic trace. It is scenario evidence, not proof of provider
or HA recovery behavior beyond that selected path.

Body Preparation includes an additional representative HMI capture. It records
the shared Rust-backed operator-session design now available in all ten process
areas without changing the documented architecture-route count.

Operations Intelligence includes an additional Central SOC capture showing
three trace-derived WAF events and the filterable analyst-session workflow. It is
evidence of the modeled local session, not a production telemetry or SIEM
implementation.

Operations Intelligence also documents a deterministic northbound-firewall
continuity run. Its trace proves one session update and heartbeat stream over
the configured HA medium, timer-based FRW-03B promotion, virtual-identity
announcements, and reverse-flow delivery. It is not evidence of vendor HA
protocol conformance or production RTO/RPO.

The same capture now records the HA-sync-loss variant. The synchronized flow
survives because its state reached the standby before the link failed. A
separate session-state-loss scenario proves the reverse ACK is rejected after
promotion when that state is absent. A long-idle variant promotes with retained
state, ages it beyond the modeled 300-second TCP timeout, and then rejects the
delayed reverse ACK; these results do not generalize to arbitrary connections
or vendor clustering behavior.

An additional HA-isolation run drops only the synchronization path and records
FRW-03B being fenced after its hold timer because FRW-03A failure is
unconfirmed. FRW-03A remains the sole active owner and completes the reverse
flow; this is not evidence of general quorum or partition arbitration.

The documented architecture is also a provisional working model. Screenshots
record the current application faithfully, but they do not freeze topology,
equipment selection, placement, addressing, or policy as final design
decisions. Those details remain planned refinement after the behavioral
contracts can test them.

```text
project/docs
|-- customer-network
|   |-- customer-lan
|   |-- customer-edge
|   `-- public-web-path
|-- central-office
|   |-- it-dmz
|   |-- business-it
|   `-- operations-intelligence
`-- factory
    |-- ot-dmz
    `-- process
        |-- material
        |-- thermal
        `-- finishing
```

## Sites

| Site | Scope |
| --- | --- |
| [Customer Network](customer-network/README.md) | Private LAN, customer edge, and public web access |
| [Central Office](central-office/README.md) | Public services, enterprise IT, governance, security operations, and analytics |
| [Factory](factory/README.md) | Factory OT DMZ and the segmented ceramics process |

## Project-Wide Decisions

- [Implementation direction](reference/project-direction.md)
- [Deployment conformance review](reference/deployment-conformance.md)
- [Rust simulation engine](reference/simulation-engine.md)
- [Svelte architecture application](reference/svelte-application.md)
- [Configuration model](../config/README.md)
- [Continuous integration policy](../standards/CI_POLICY.md)
- [Changelog](../../CHANGELOG.md)
- [Versioning and releases](reference/versioning.md)

Network and process details belong at their lowest owning level. Parent pages
describe scope, authority, and relationships without duplicating child
inventories.

## Status Language

- **Implemented** describes behavior that can be exercised in the current
  Svelte application or build pipeline.
- **Bootstrap** describes representative data used to establish a view or
  interface contract before authoritative generation exists.
- **Provisional baseline** describes structurally valid configuration or
  architecture placeholders that are intentionally unfinished and expected to
  change as executable requirements mature.
- **Planned** describes work for which no executable implementation currently
  exists.
- **Validation target** describes an expected result that the future Rust
  engine must prove; it is not a passing test today.

Paired devices express a redundancy requirement or logical role. They do not,
by themselves, prove independent failure domains, synchronized state, or
tested failover. The Business IT core pair is the current exception only for
its explicitly configured VRRP identities, deterministic Rapid-PVST
converged-state calculation, and selected recovery scenario.

Future documentation will be generated or cross-checked against canonical YAML,
Rust diagnostics, control-source references, and reproducible scenario
results. Screenshots and architecture text must be updated together whenever a
rendered route changes.
