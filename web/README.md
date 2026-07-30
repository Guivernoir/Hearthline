# Hearthline Architecture Application

This directory contains the static Svelte architecture application for
Hearthline. The current implementation establishes map-first navigation,
location and environment drill-downs, the factory process canvas, and
Rust-generated inspection and validated local editing of canonical appliance
and connection YAML.

The current development release is `0.2.0`. Project release compatibility is
defined in the [versioning policy](../docs/versioning.md), while frontend data
schemas retain their own independent versions.

## Current Status

The application is an interactive architecture viewer, not yet an executable
network or plant simulator. Navigation, physical and logical canvases,
inspection, and responsive controls are implemented. Most topology records
remain frontend bootstrap data, and no route, firewall, PLC, or process result
is currently supplied to the browser. Rust supplies validated appliance
identity, placement, behavior-family summaries, connection endpoints, and
complete YAML source documents. It does not yet supply an executed topology or
scenario result.

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
- Connection inspection of endpoint port state plus Rust-derived effective
  MTU, negotiated duplex, propagation delay, and medium-specific physical
  facts.
- Local YAML editing through a Rust API with revision checks, whole-project
  validation, and atomic catalog regeneration.
- Click-to-center minimap on desktop and tablet layouts.
- Responsive map, toolbar, inspector, location, and detailed network layouts.
- Distinct trust-path, control-network, and material-flow representations.

Regional, location, Customer Network, Central Office, and most Factory data is
still temporary view data declared in `src/lib/*.svelte`. OT process inventory
is now read from `src/generated/process-view.json`, a versioned bootstrap
derivative with source references for each area and component. It is not yet
canonical or Rust-generated. Configuration is independently read from
`src/generated/appliance-configs.json`, which Rust generates from 160
appliance and 205 connection YAML files. The process model will later be replaced with JSON
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

The viewer remains usable without the API. Validated editing additionally
requires this repository-root command:

```bash
cargo run -p hearthline-api
```

Vite proxies `/api` to `127.0.0.1:3001`. The editor disables write controls
when that service is unavailable.

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

1. Display formal Rust-generated device-to-device communication traces carried
   through the configured media layer.
2. Replace remaining Svelte topology arrays with generated view models.
3. Consume Rust validation diagnostics and explained connectivity results.
4. Display scenario state, process state, alarms, and fault outcomes without
   calculating them in Svelte.
5. Replace provisional architecture and configuration placeholders with
   cross-validated definitions derived from executable requirements.
6. Extend implemented appliance and connection provenance to policy and
   control sources.
7. Keep bootstrap compatibility explicit until all authoritative inputs exist.

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
