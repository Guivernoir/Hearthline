# Hearthline Architecture Application

This directory contains the static Svelte architecture application for
Hearthline. The current implementation establishes map-first navigation,
location and environment drill-downs, and the factory process canvas before
YAML and Rust integration.

## Current Status

The application is an interactive architecture viewer, not yet an executable
network or plant simulator. Navigation, physical and logical canvases,
inspection, and responsive controls are implemented. Most topology records
remain frontend bootstrap data, and no route, firewall, PLC, or process result
is currently calculated by the browser or a backend engine.

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
- Click-to-center minimap on desktop and tablet layouts.
- Responsive map, toolbar, inspector, location, and detailed network layouts.
- Distinct trust-path, control-network, and material-flow representations.

Regional, location, Customer Network, Central Office, and most Factory data is
still temporary view data declared in `src/lib/*.svelte`. OT process inventory
is now read from `src/generated/process-view.json`, a versioned bootstrap
derivative with source references for each area and component. It is not yet
canonical or Rust-generated. The Rust pipeline will replace it with JSON
generated from validated YAML and IEC 61131-3 cross-references.

## Commands

```bash
npm install
npm run dev
npm run check
npm run build
npm run preview
```

The development server listens on all interfaces and normally starts at
`http://localhost:5173`.

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

The process inventory has been extracted behind the first JSON view-model
contract. Remaining place, environment, node, and connection arrays will follow
after the shared schemas are defined. Svelte may retain presentation
coordinates and interaction state, but it must not parse YAML or IEC 61131-3
source, evaluate connectivity, or simulate the process.

## Planned Integration

1. Replace remaining Svelte topology arrays with generated view models.
2. Consume Rust validation diagnostics and explained connectivity results.
3. Display scenario state, process state, alarms, and fault outcomes without
   calculating them in Svelte.
4. Add provenance links from rendered assets to canonical YAML and control
   sources.
5. Keep bootstrap compatibility explicit until all authoritative inputs exist.

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
