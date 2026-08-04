# Hearthline Architecture Application

This directory contains the static Svelte architecture application for
Hearthline. The current implementation establishes map-first navigation,
location and environment drill-downs, the factory process canvas,
Rust-generated inspection, validated local editing of canonical appliance and
connection YAML, and the configured simulation workspace.

The current development release is `0.2.0`. Project release compatibility is
defined in the [versioning policy](versioning.md), while frontend data
schemas retain their own independent versions.

## Current Status

The application is an interactive architecture viewer with 28 executable
configured scenarios, not yet a general-purpose network or plant simulator.
Navigation, physical and logical canvases, inspection, responsive controls,
packet composition, scenario execution, and trace inspection are implemented.
Most topology records remain frontend bootstrap data. Rust supplies validated
appliance and connection data and executes Customer DNS, permitted public
HTTPS, denied public management, and approved or denied factory
operations-data scenarios. Firewall-rule, translation, application-forward,
HTTP-response, and default-deny results are available for those selected paths.
The Business IT availability scenario exposes effective uplink and VRRP member
state, applies a Core-02 recovery preset, and displays the resulting secondary
forwarding trace.
The northbound-firewall recovery exposes active/standby ownership, HA-sync
link status, monitored virtual interfaces, and the converged FRW-03B path.
Its continuity variants present the injected fault, synchronized-session
transition, sync state, last heartbeat, promotion, and deterministic
failure-to-promotion evidence from two-packet Rust runs. The interface labels
an expected policy drop after standby state loss as a passing fail-closed
expectation rather than as successful traffic continuity.
The HA-isolation contract displays synchronization loss, the single-active
owner count, standby-fencing time, and unconfirmed peer-failure state while
the healthy active path continues serving the retained flow.
Three controlled security exercises send a configured traversal probe,
disallowed DELETE request, and SQL-injection POST body from Customer PC-01,
record distinct WAF prevention evidence, and expose all three in a session-local
Central SOC queue.
Customer PC-01/02 and Business IT PC-01/02/03/04 are enterable as endpoint
sessions whose terminals and browsers invoke independent Rust paths; each browser
renders bounded page content returned by its configured service. All ten
process areas have Rust-backed HMI sessions with safety, alarms, YAML-derived
instruments, equipment-specific actuator commands, audit state, and component
traces.
The factory local-autonomy workspace combines both failed inter-site handoffs,
the expected historian-path drop, a seven-link local control path, safety
reset, pump command, resulting actuator state, and six-stage control trace in
one Rust-derived report.
IEC 61131-3 execution and changing process results are not yet available.

The rendered architecture and underlying YAML content are provisional
placeholders, not finished network or plant definitions. The viewer accurately
shows the current working model, while topology, placement, equipment, policy,
and configuration details remain subject to revision as executable behavior
and scenarios are implemented.

## Current Capabilities

- Regional map with selectable Customer Network, Central Office, and Factory
  locations.
- Location inspector with an explicit entry action and direct hash routes.
- Customer Network, Central Office, and Factory environment overviews.
- Customer Network physical overview represented as a residential property,
  a separate edge cabinet and service demarcation, utility handoff, and public
  access boundary without overlapping environment markers.
- Central Office physical overview represented as a controlled campus with a
  WAN demarcation and separate perimeter-services, enterprise-office, and
  operations-center buildings.
- Customer LAN environment entry with a residential physical layout and a
  logical network diagram that ends at the router's LAN-facing boundary.
- Customer Edge environment entry focused on the service cabinet, PAT
  boundary, provider CPE, WAN-01, and provider gateway without duplicating the
  Customer LAN switch and endpoints.
- Public Web Path environment entry covering ISP routing and DNS, WAN-02,
  business static NAT, perimeter policy, and the public DMZ web-gateway tier
  across three explicit physical sites.
- Business IT DMZ environment entry covering the WAN demarcation, edge routing,
  static NAT, perimeter policy, public service VLAN, and downstream IT
  boundary.
- Business IT environment entry covering the collapsed core, server, user,
  voice, printer, guest, and management VLANs, plus both security handoffs.
- Operations Intelligence environment entry covering Central Office network
  governance, NOC and SOC functions, identity and policy, brokered production
  data, process analysis, change approval, and the encrypted Factory conduit.
- Enterable Central SOC console with a bounded modeled-event queue, detector
  evidence, source and destination context, all/active/acknowledged filtering,
  analyst acknowledgement, and clear controls.
- Factory OT DMZ environment entry covering independent northbound and southbound HA
  firewalls, redundant switching, access, exchange, and monitoring subzones,
  passive sensors, explicit service trunks, and the Level 3 engineering
  handoff.
- Factory overview and drill-down into the ten-stage ceramics process
  architecture.
- Enterable process-area views that distinguish the shared physical vPLC
  compute cluster from the logical area runtime and show local switching, HMI,
  distributed I/O, sensors, actuators, and safety or permissive roles.
- Icon-based equipment and environment markers in physical views, with
  information cards reserved for logical architecture and inspectors.
- Persistent physical and logical view modes.
- Select and pan interaction modes.
- Button and keyboard zoom with focal-point preservation.
- Fit-to-view and reset controls.
- Native horizontal and vertical scrolling.
- Optional major and minor canvas grid.
- Clickable architecture nodes with a compact inspector.
- Rust-derived appliance summaries for individual and grouped HA nodes.
- Dedicated appliance and connection routes showing complete validated YAML
  without parsing YAML in the browser.
- Appliance inspection of port hardware, administrative and initial
  operational state, configured speed, duplex, MTU, and supported media.
- Scenario editing of effective connection state and segmented active/standby
  controls for configured first-hop members, with one-click recovery presets.
- Connection inspection of endpoint port state plus Rust-derived effective
  MTU, negotiated duplex, propagation delay, and medium-specific physical
  facts.
- Local YAML editing through a Rust API with revision checks, whole-project
  validation, and atomic catalog regeneration.
- A simulation route with a Rust-supplied scenario catalog, editable packet
  fields and selected-link state, run and canonical reset controls, result
  metrics, trace filtering, and microsecond per-hop effects.
- A customer access-circuit outage whose canonical down state can be restored
  with one command without modifying connection YAML; Rust switches from the
  baseline drop expectation to the declared recovery delivery expectation.
- A configured Customer DNS exchange across the customer LAN, PAT boundary,
  provider access path, ISP router, and authoritative DNS server.
- Configured public HTTPS request and response through customer PAT, provider
  transit, business static NAT, both firewall boundaries, the DMZ web gateway,
  and the internal application tier, plus public SSH denial.
- Configuration-owned path-traversal, disallowed-method, and SQL-injection
  request-body exercises that use workstation `curl`, reach the DMZ
  reverse-proxy WAF through the modeled media path, are prevented by Rust
  behavior, and emit distinct defensive evidence.
- Configured factory operations-data delivery and denial traces through the OT
  DMZ exchange subzone, inter-site conduit, northbound firewall, enterprise
  core, and analytics service.
- A composite factory autonomy run that keeps both conduit handoffs down while
  displaying the independent local HMI-to-pump command result and its
  safety/control-path evidence.
- Desktop catalog navigation plus a compact mobile scenario selector.
- Direct navigation from scenario participants and trace components to their
  appliance configuration routes.
- Enterable Customer PC-01 and PC-02 desktops with browsers, terminals,
  configuration launchers, independent YAML-derived network identities,
  command history, responsive controls, and expandable Rust trace activity.
- Enterable Business IT PC-01 through PC-04 desktops selected from a grouped
  office node, with scenario-derived portal homes and internal DNS and HTTPS
  traces across both user-access switches and Core-01 VLAN 20, 30, and 80
  SVIs.
- Rust-backed `hostname`, `ipconfig`, `nslookup`, method- and body-aware
  `curl`, and `ssh` terminal commands plus bounded quoted arguments, browser
  URL parsing, DNS resolution, modeled HTTP content, and explicit policy-denial
  presentation.
- Enterable HMIs for all ten process areas with configured sensor values,
  safety permissives and reset, alarm acknowledgement, equipment-specific
  controls, operator audit, and Rust-generated command-path traces.
- Click-to-center minimap on desktop and tablet layouts.
- Responsive map, toolbar, inspector, location, and detailed network layouts.
- Distinct trust-path, control-network, and material-flow representations.

Regional, location, Customer Network, Central Office, and most Factory data is
still temporary view data declared in `src/lib/*.svelte`. OT process inventory
is now read from `src/generated/process-view.json`, a versioned bootstrap
derivative with source references for each area and component. It is not yet
canonical or Rust-generated. Configuration is independently read from
`src/generated/appliance-configs.json`, which Rust generates from 162
appliance and 208 connection YAML files. The process model will later be replaced with JSON
generated from validated area topology and IEC 61131-3 cross-references.

## Commands

```bash
npm install
npm run dev
npm run check
npm run build
npm run preview
npm run version:check
```

The development server listens on all interfaces and normally starts at
`http://localhost:5173`.

Architecture viewing remains usable without the API. Scenario execution and
validated editing require this repository-root command:

```bash
cargo run --manifest-path packages/Cargo.toml -p hearthline-api
```

Vite proxies `/api` to `127.0.0.1:3001`. The editor disables write controls
and simulation execution reports an unavailable service when that API is not
running.

## Canvas Controls

| Control | Action |
| --- | --- |
| Location marker | Select a site and open its location inspector |
| Marker double-click | Enter the selected location directly |
| Enter environment | Open an available environment-level view |
| Physical / Logical | Change the active architecture representation |
| Pointer button | Select architecture nodes |
| Hand button | Drag the canvas |
| Mouse wheel or trackpad | Scroll the workspace |
| Ctrl/Cmd + wheel | Zoom at the pointer |
| `+` / `-` | Zoom in or out |
| `F` | Fit the architecture to the viewport |
| `0` | Reset to 100% and the canvas origin |
| Space + drag | Temporarily pan |
| Escape | Clear the current selection |
| Minimap click | Center the selected overview location |

## Implementation Boundary

Svelte owns presentation, view navigation, and local interaction only. It must
not implement network routing, firewall, identity, PLC, process, or simulation
decisions. Those results will arrive as validated data from the Rust engine and
virtual PLC integration.

The process inventory and configuration catalog are behind separate versioned
JSON contracts. Remaining place, environment, and node arrays will follow as
their canonical schemas mature. Svelte may retain presentation coordinates
and interaction state, but it does not parse YAML or IEC 61131-3 source,
evaluate connectivity, or simulate the process. YAML updates are submitted to
Rust as opaque text and only accepted after server-side parsing and validation.

## Planned Integration

1. Add changing sensor, alarm, permissive, and actuator effects behind the
   ten-area HMI baseline as the plant model becomes executable.
2. Extend the bounded autonomy proof with further deterministic outage and
   local-control cases where they add distinct evidence.
3. Replace remaining Svelte topology arrays with generated view models.
4. Consume Rust validation diagnostics and explained connectivity results.
5. Display process state, alarms, and fault outcomes without
   calculating them in Svelte.
6. Replace provisional architecture and configuration placeholders with
   cross-validated definitions derived from executable requirements.
7. Extend implemented appliance and connection provenance to policy and
   control sources.
8. Extend the controlled Phase 3 WAF baseline with additional attack,
   detection, investigation, and response workflows.

## Navigation Model

The application uses the same drill-down semantics at every level:

```text
Regional map
  -> Location overview
     -> Environment detail
        -> Process area
           -> Device inspection
```

Physical mode moves from geographic placement to site placement and then to
equipment placement. Logical mode follows the same route while replacing
spatial context with zones, networks, conduits, and interfaces. New location
and environment views should preserve this correspondence. Every environment
declared in a location overview must provide an environment-level route; an
overview-only placeholder is not considered complete.
