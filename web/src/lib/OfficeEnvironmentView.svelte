<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { Component } from "svelte";
  import {
    ArrowLeft,
    Building2,
    Cable,
    ChevronDown,
    Cloud,
    Database,
    Factory,
    FileCheck,
    Globe2,
    Grid2X2,
    Laptop,
    Map,
    Maximize2,
    Minus,
    Network,
    Phone,
    Plus,
    Printer,
    RadioTower,
    RotateCcw,
    Router,
    Server,
    ShieldCheck,
    Users,
    Wifi,
    X,
  } from "@lucide/svelte";
  import PhysicalDeviceMarker from "./PhysicalDeviceMarker.svelte";
  import type { ViewMode } from "./types";

  type OfficeEnvironment =
    | "it-dmz"
    | "business-it"
    | "operations-intelligence"
    | "ot-dmz";

  interface NodePosition {
    x: number;
    y: number;
  }

  interface OfficeNode {
    id: string;
    label: string;
    role: string;
    area: string;
    address: string;
    facts: string[];
    accent: string;
    icon: Component<any>;
    physical: NodePosition;
    logical: NodePosition;
  }

  export let environment: OfficeEnvironment;
  export let onBack: () => void = () => {};
  export let siteLabel = "Central Office";
  export let viewMode: ViewMode = "logical";

  const WORLD_WIDTH = 1800;
  const WORLD_HEIGHT = 900;
  const MIN_ZOOM = 0.22;
  const MAX_ZOOM = 1.6;
  const ZOOM_STEP = 0.1;

  const itDmzNodes: OfficeNode[] = [
    {
      id: "wan-02",
      label: "WAN-02A / 02B",
      role: "Diverse business-facing provider circuits",
      area: "Provider handoff",
      address: "192.0.2.0/24",
      facts: ["Independent carrier paths are required", "No enterprise routing function at the access handoff"],
      accent: "#6a5c91",
      icon: Cloud,
      physical: { x: 35, y: 400 },
      logical: { x: 35, y: 400 },
    },
    {
      id: "business-modem",
      label: "Business INET-CPE-01 / 02",
      role: "Provider-managed access CPE at the business demarcation",
      area: "WAN demarcation",
      address: "No Layer 3 address",
      facts: ["Independent access circuits", "Customer handoffs to the enterprise edge pair"],
      accent: "#7a6546",
      icon: RadioTower,
      physical: { x: 250, y: 400 },
      logical: { x: 250, y: 400 },
    },
    {
      id: "business-edge",
      label: "Business EDGE-RTR-01 / 02",
      role: "Redundant Internet edge and public static NAT role",
      area: "Enterprise edge",
      address: "192.0.2.2/24 · 10.255.0.1/30",
      facts: [
        "192.0.2.10 maps to 172.16.10.2",
        "Default route to 192.0.2.1",
        "DMZ route through 10.255.0.2",
      ],
      accent: "#b65034",
      icon: Router,
      physical: { x: 465, y: 400 },
      logical: { x: 465, y: 400 },
    },
    {
      id: "perimeter-firewall",
      label: "Business FRW-01A / 01B",
      role: "High-availability policy boundary between edge transit and public DMZ",
      area: "Perimeter security",
      address: "10.255.0.2/30 · 172.16.10.1/24",
      facts: [
        "Independent outside and public DMZ zones",
        "Inbound policy permits HTTPS to the published gateway VIP",
        "HTTP is redirect-only when explicitly enabled",
      ],
      accent: "#9e3f2f",
      icon: ShieldCheck,
      physical: { x: 690, y: 400 },
      logical: { x: 690, y: 400 },
    },
    {
      id: "dmz-switch",
      label: "Business IT-DMZ-SW-01 / 02",
      role: "Redundant public DMZ switching role",
      area: "Public DMZ",
      address: "VLAN 10 · PUBLIC_DMZ",
      facts: [
        "Independent links to FRW-01A/01B",
        "Dual-attached web gateway tier",
        "Independent links to FRW-02A/02B",
      ],
      accent: "#3567a6",
      icon: Network,
      physical: { x: 900, y: 400 },
      logical: { x: 930, y: 400 },
    },
    {
      id: "public-web",
      label: "Business WEB-GW-01 / 02",
      role: "Reverse proxy, TLS termination, and web application firewall tier",
      area: "Published service",
      address: "172.16.10.2/24 · public 192.0.2.10",
      facts: [
        "HTTPS terminates at the DMZ gateway VIP",
        "HTTP is redirect-only when enabled",
        "Only named internal application dependencies are proxied",
      ],
      accent: "#51704c",
      icon: Server,
      physical: { x: 1160, y: 260 },
      logical: { x: 1210, y: 260 },
    },
    {
      id: "business-firewall",
      label: "Business FRW-02A / 02B",
      role: "High-availability downstream boundary from the public DMZ to Business IT",
      area: "IT handoff",
      address: "172.16.10.3/24 · 10.255.0.6/30",
      facts: [
        "Ethernet0/1 faces the public DMZ",
        "Ethernet0/0 faces Business IT",
        "No unsolicited DMZ-to-IT access by default",
      ],
      accent: "#9e3f2f",
      icon: ShieldCheck,
      physical: { x: 1160, y: 545 },
      logical: { x: 1210, y: 545 },
    },
    {
      id: "business-core-handoff",
      label: "Business IT Core Handoff",
      role: "Adjacent routed handoff into the internal Business IT environment",
      area: "Adjacent environment",
      address: "10.255.0.5/30",
      facts: ["FRW-02A/02B inside next hop", "Detailed in the Business IT environment"],
      accent: "#267168",
      icon: Building2,
      physical: { x: 1510, y: 545 },
      logical: { x: 1500, y: 545 },
    },
  ];

  const businessItNodes: OfficeNode[] = [
    {
      id: "frw-02",
      label: "Business FRW-02A / 02B",
      role: "High-availability upstream security boundary toward the public IT DMZ",
      area: "IT perimeter",
      address: "10.255.0.6/30",
      facts: ["Inside Ethernet0/0", "Default route through the public DMZ"],
      accent: "#9e3f2f",
      icon: ShieldCheck,
      physical: { x: 70, y: 280 },
      logical: { x: 35, y: 390 },
    },
    {
      id: "it-core",
      label: "Business IT-CORE-SW-01 / 02",
      role: "Redundant Layer 3 core and gateway role for internal security zones",
      area: "Network core",
      address: "VLAN gateway VIPs 10.10.20.1–90.1",
      facts: [
        "Redundant routed transit to FRW-02A/02B",
        "No user traffic is carried on the native VLAN",
        "Inter-zone traffic is deny by default and explicitly permitted",
      ],
      accent: "#267168",
      icon: Network,
      physical: { x: 290, y: 280 },
      logical: { x: 285, y: 390 },
    },
    {
      id: "server-switch",
      label: "Business IT-SRV-SW-01 / 02",
      role: "Redundant access switching for infrastructure and application zones",
      area: "Server access",
      address: "VLANs 20, 70, 80, 90, 999",
      facts: ["Dual uplinks to the core pair", "Management uses VLAN 70"],
      accent: "#3567a6",
      icon: Network,
      physical: { x: 525, y: 190 },
      logical: { x: 590, y: 155 },
    },
    {
      id: "server-fleet",
      label: "Internal Service Clusters",
      role: "Infrastructure, application, data, transfer, monitoring, backup, and time services",
      area: "VLANs 20, 80, 90",
      address: "Infrastructure, application, and data VIPs",
      facts: [
        "Dedicated DNS, DHCP, PKI, and primary/secondary time sources",
        "Application/API services are separated from data services",
        "Managed transfer, monitoring, backup, and recovery roles are defined",
        "Public gateways reach only named application VIPs",
      ],
      accent: "#51704c",
      icon: Database,
      physical: { x: 790, y: 150 },
      logical: { x: 875, y: 85 },
    },
    {
      id: "voice-gateway",
      label: "Business IT-VOICE-GW-01",
      role: "Enterprise voice call-control gateway",
      area: "VLAN 20 · IT_SERVERS",
      address: "10.10.20.50/24",
      facts: ["Voice registration and call control", "Uses the dedicated enterprise time service", "Attached through the server access pair"],
      accent: "#6a5c91",
      icon: Phone,
      physical: { x: 790, y: 285 },
      logical: { x: 875, y: 230 },
    },
    {
      id: "user-switch",
      label: "Business IT-USR-SW-01 / 02",
      role: "Redundant employee access switching role",
      area: "User access",
      address: "VLANs 30, 70, 999",
      facts: ["Redundant uplinks to the core pair", "Management uses VLAN 70"],
      accent: "#3567a6",
      icon: Network,
      physical: { x: 525, y: 520 },
      logical: { x: 590, y: 390 },
    },
    {
      id: "user-pcs",
      label: "Business IT-USR-PC-01–04",
      role: "Representative managed employee workstations",
      area: "VLAN 30 · IT_USERS",
      address: "DHCP from 10.10.30.100",
      facts: ["Fa0/1–4 on USR-SW-01", "Internal DNS 10.10.20.10"],
      accent: "#426d9d",
      icon: Users,
      physical: { x: 790, y: 520 },
      logical: { x: 875, y: 390 },
    },
    {
      id: "service-switch",
      label: "Business IT-SVC-SW-01 / 02",
      role: "Redundant access switching for voice, printers, and wireless",
      area: "Office services",
      address: "VLANs 40, 50, 70, 999",
      facts: ["Redundant uplinks to the core pair", "Management uses VLAN 70"],
      accent: "#3567a6",
      icon: Network,
      physical: { x: 525, y: 680 },
      logical: { x: 590, y: 625 },
    },
    {
      id: "phone",
      label: "Business IT-PHONE-01",
      role: "Registered enterprise voice endpoint",
      area: "VLAN 40 · IT_VOICE",
      address: "DHCP from 10.10.40.100",
      facts: ["SVC-SW Fa0/1 voice VLAN 40", "Extension 2001", "Option 150 points to 10.10.20.50"],
      accent: "#6a5c91",
      icon: Phone,
      physical: { x: 790, y: 650 },
      logical: { x: 875, y: 555 },
    },
    {
      id: "printer",
      label: "Business IT-PRN-01",
      role: "Shared office printer",
      area: "VLAN 50 · IT_PRINTERS",
      address: "10.10.50.10/24",
      facts: ["SVC-SW Fa0/2", "Gateway 10.10.50.1"],
      accent: "#7a6546",
      icon: Printer,
      physical: { x: 1010, y: 705 },
      logical: { x: 875, y: 700 },
    },
    {
      id: "guest-access",
      label: "Guest Wireless",
      role: "Isolated guest WLAN and representative unmanaged client",
      area: "VLAN 60 · IT_GUEST",
      address: "Business-Guest · DHCP from 10.10.60.100",
      facts: [
        "Access points use a separate management network",
        "Client isolation is enabled",
        "Guest traffic is limited to filtered Internet egress",
      ],
      accent: "#b65034",
      icon: Wifi,
      physical: { x: 80, y: 640 },
      logical: { x: 1190, y: 150 },
    },
    {
      id: "admin-pc",
      label: "Business IT-PAW-01",
      role: "Privileged access workstation for approved administration",
      area: "VLAN 70 · IT_MANAGEMENT",
      address: "10.10.70.30/24",
      facts: ["Dedicated management source role", "MFA and device posture required", "No general productivity use"],
      accent: "#267168",
      icon: Laptop,
      physical: { x: 1080, y: 210 },
      logical: { x: 1190, y: 350 },
    },
    {
      id: "network-controller",
      label: "Business IT-NET-CTRL-01",
      role: "Central network-management controller",
      area: "VLAN 70 · IT_MANAGEMENT",
      address: "10.10.70.20/24",
      facts: ["Management VLAN only", "AAA, configuration backup, and approved automation", "No public service exposure"],
      accent: "#267168",
      icon: Server,
      physical: { x: 1300, y: 330 },
      logical: { x: 1450, y: 440 },
    },
    {
      id: "frw-03",
      label: "Business FRW-03A / 03B",
      role: "High-availability boundary from Business IT to the Factory OT DMZ",
      area: "OT security handoff",
      address: "Canonical addressing pending",
      facts: [
        "Only intended Business IT-to-OT DMZ boundary",
        "Reached through the Operations Intelligence policy path",
        "Conduits require explicit source, destination, service, and purpose",
      ],
      accent: "#9e3f2f",
      icon: ShieldCheck,
      physical: { x: 1535, y: 360 },
      logical: { x: 1190, y: 650 },
    },
    {
      id: "ot-dmz-handoff",
      label: "OT DMZ Handoff",
      role: "Adjacent Level 3.5 environment; no direct path to OT",
      area: "Adjacent environment",
      address: "Canonical addressing pending",
      facts: ["Terminates in the OT DMZ", "Does not bypass the OT-side firewall"],
      accent: "#51704c",
      icon: Building2,
      physical: { x: 1535, y: 580 },
      logical: { x: 1480, y: 650 },
    },
  ];

  const operationsIntelligenceNodes: OfficeNode[] = [
    {
      id: "business-it-source",
      label: "Business IT",
      role: "Enterprise source for approved users, identity, and governance workflows",
      area: "Enterprise handoff",
      address: "Business IT management networks",
      facts: [
        "No direct route to controllers",
        "Administrative access requires named users and managed devices",
      ],
      accent: "#3567a6",
      icon: Building2,
      physical: { x: 30, y: 400 },
      logical: { x: 30, y: 400 },
    },
    {
      id: "identity-policy",
      label: "Identity & Policy Services",
      role: "Central identity, authorization, device posture, and policy decision services",
      area: "Enterprise governance",
      address: "Canonical addressing pending",
      facts: [
        "Policy engine and policy administrator roles",
        "MFA and phishing-resistant authentication",
        "Managed-device posture",
        "Maintenance-window and work-order context",
        "Resource-side enforcement points apply the decision",
      ],
      accent: "#6a5c91",
      icon: ShieldCheck,
      physical: { x: 270, y: 180 },
      logical: { x: 270, y: 180 },
    },
    {
      id: "central-noc",
      label: "Central NOC",
      role: "Network architecture, configuration governance, availability, and change coordination",
      area: "Network operations",
      address: "Canonical addressing pending",
      facts: [
        "Owns enterprise and inter-site network decisions",
        "Changes follow approval and staging",
        "No direct interactive controller access",
      ],
      accent: "#267168",
      icon: Network,
      physical: { x: 520, y: 180 },
      logical: { x: 520, y: 180 },
    },
    {
      id: "central-soc",
      label: "Central SOC",
      role: "Security monitoring, investigation, and incident coordination",
      area: "Security operations",
      address: "Canonical addressing pending",
      facts: [
        "Consumes approved security telemetry",
        "Coordinates response with factory OT operations",
        "Monitoring is not a process forwarding dependency",
      ],
      accent: "#9e3f2f",
      icon: ShieldCheck,
      physical: { x: 520, y: 545 },
      logical: { x: 520, y: 545 },
    },
    {
      id: "analytics-platform",
      label: "Process Analytics Platform",
      role: "Enterprise analysis of selected production, quality, energy, and maintenance data",
      area: "Operations analytics",
      address: "Canonical addressing pending",
      facts: [
        "Reads brokered or replicated data",
        "Does not query the primary OT historian directly",
        "Analytics output is advisory until approved",
      ],
      accent: "#51704c",
      icon: Database,
      physical: { x: 850, y: 180 },
      logical: { x: 850, y: 180 },
    },
    {
      id: "process-analysts",
      label: "Process Analysis Workstations",
      role: "Engineering and business analysis of process performance and product quality",
      area: "Analysis workspace",
      address: "Managed enterprise endpoints",
      facts: [
        "Uses approved datasets and dashboards",
        "Cannot issue direct control commands",
        "Recommendations enter the governed change workflow",
      ],
      accent: "#426d9d",
      icon: Users,
      physical: { x: 1100, y: 180 },
      logical: { x: 1100, y: 180 },
    },
    {
      id: "change-staging",
      label: "Change Approval & Staging",
      role: "Review, approval, scanning, signing, and staging of authorized changes",
      area: "Governed change",
      address: "Canonical addressing pending",
      facts: [
        "Separates analysis from implementation",
        "Produces auditable approved packages",
        "Factory personnel retain execution authority",
      ],
      accent: "#7a6546",
      icon: FileCheck,
      physical: { x: 850, y: 545 },
      logical: { x: 850, y: 545 },
    },
    {
      id: "factory-conduit",
      label: "Encrypted Factory Conduit",
      role: "Authenticated inter-site transport to the factory security perimeter",
      area: "Inter-site WAN",
      address: "Canonical transport pending",
      facts: [
        "Carries only named data and administrative workflows",
        "WAN loss does not stop local factory operation",
        "Factory-local firewalls remain authoritative",
      ],
      accent: "#6a5c91",
      icon: Globe2,
      physical: { x: 1325, y: 545 },
      logical: { x: 1280, y: 545 },
    },
    {
      id: "factory-dmz-target",
      label: "Factory OT DMZ",
      role: "Adjacent factory-local security and exchange environment",
      area: "Adjacent environment",
      address: "Factory Level 3.5",
      facts: [
        "Terminates Central Office access locally",
        "Brokers selected OT data outward",
        "No conduit bypasses the OT-side firewall",
      ],
      accent: "#b65034",
      icon: Factory,
      physical: { x: 1550, y: 545 },
      logical: { x: 1550, y: 545 },
    },
  ];

  const otDmzNodes: OfficeNode[] = [
    {
      id: "business-it-handoff",
      label: "Central Office Conduit",
      role: "Encrypted inter-site transport carrying approved data and administrative workflows",
      area: "Northbound WAN",
      address: "Canonical transport pending",
      facts: [
        "Named Central Office sources only",
        "No direct route to factory OT",
        "Factory-local policy remains authoritative",
      ],
      accent: "#3567a6",
      icon: Building2,
      physical: { x: 30, y: 400 },
      logical: { x: 30, y: 400 },
    },
    {
      id: "north-firewall",
      label: "Business FRW-03A / 03B",
      role: "Northbound HA policy boundary between Business IT and the OT DMZ",
      area: "IT-side boundary",
      address: "Canonical addressing pending",
      facts: ["Synchronized HA policy role", "Independent from OT FRW-01", "Deny by default"],
      accent: "#9e3f2f",
      icon: ShieldCheck,
      physical: { x: 265, y: 400 },
      logical: { x: 265, y: 400 },
    },
    {
      id: "dmz-switch-pair",
      label: "OT-DMZ-SW-01 / 02",
      role: "Redundant Layer 2 attachment without unrestricted inter-subzone routing",
      area: "DMZ switching",
      address: "Subzone VLANs pending",
      facts: [
        "Independent firewall and service attachment paths",
        "Explicit loop-avoidance or multi-chassis design required",
        "Access, exchange, and monitoring remain separate",
      ],
      accent: "#267168",
      icon: Network,
      physical: { x: 500, y: 400 },
      logical: { x: 500, y: 400 },
    },
    {
      id: "jump-service",
      label: "OT-DMZ-JUMP-SRV-01 / 02",
      role: "Controlled administrative access and session termination",
      area: "Access subzone",
      address: "Canonical addressing pending",
      facts: [
        "Separate authenticated session into Level 3",
        "No general browsing, email, or direct Internet",
        "Time-bound and recorded privileged access",
      ],
      accent: "#426d9d",
      icon: Server,
      physical: { x: 800, y: 160 },
      logical: { x: 800, y: 135 },
    },
    {
      id: "historian-replica",
      label: "OT-DMZ-HIST-REPLICA-01",
      role: "Business-facing replica of selected OT historian data",
      area: "Exchange subzone",
      address: "Canonical addressing pending",
      facts: [
        "IT consumers do not query the primary OT historian",
        "One-way or brokered exchange policy pending",
      ],
      accent: "#51704c",
      icon: Database,
      physical: { x: 800, y: 370 },
      logical: { x: 800, y: 315 },
    },
    {
      id: "transfer-service",
      label: "OT-DMZ-XFER-SRV-01",
      role: "Managed file transfer and update staging",
      area: "Exchange subzone",
      address: "Canonical addressing pending",
      facts: ["Staged and scanned files", "Approval and direction are explicit", "All transfers are audited"],
      accent: "#7a6546",
      icon: FileCheck,
      physical: { x: 800, y: 515 },
      logical: { x: 800, y: 495 },
    },
    {
      id: "monitoring",
      label: "OT-DMZ-MON-01",
      role: "Security telemetry collector and monitoring management role",
      area: "Monitoring subzone",
      address: "Canonical addressing pending",
      facts: ["Not an inline forwarding hop", "Capture and management paths are distinct"],
      accent: "#6a5c91",
      icon: Server,
      physical: { x: 800, y: 700 },
      logical: { x: 770, y: 675 },
    },
    {
      id: "passive-sensors",
      label: "OT-SENSOR-*",
      role: "Passive sensors on defined SPAN or TAP sources",
      area: "Monitoring subzone",
      address: "Out-of-band capture",
      facts: ["Process traffic never depends on sensor forwarding", "Active probing requires explicit approval"],
      accent: "#6a5c91",
      icon: RadioTower,
      physical: { x: 970, y: 700 },
      logical: { x: 970, y: 675 },
    },
    {
      id: "south-firewall",
      label: "OT FRW-01A / 01B",
      role: "Independent southbound HA policy boundary into Level 3 OT",
      area: "OT-side boundary",
      address: "Canonical addressing pending",
      facts: [
        "Permission at FRW-03 does not imply permission here",
        "Named conduits only",
        "Failover preserves deny-by-default policy",
      ],
      accent: "#9e3f2f",
      icon: ShieldCheck,
      physical: { x: 1285, y: 400 },
      logical: { x: 1285, y: 400 },
    },
    {
      id: "level-three",
      label: "Level 3 OT Engineering",
      role: "Adjacent OT core and engineering zone",
      area: "Southbound environment",
      address: "Canonical addressing pending",
      facts: [
        "OT-ENG-WS-01 resides here, not in the DMZ",
        "Named controller access crosses cell or area policy",
      ],
      accent: "#51704c",
      icon: Globe2,
      physical: { x: 1550, y: 400 },
      logical: { x: 1550, y: 400 },
    },
  ];

  $: nodes =
    environment === "it-dmz"
      ? itDmzNodes
      : environment === "business-it"
        ? businessItNodes
        : environment === "operations-intelligence"
          ? operationsIntelligenceNodes
          : otDmzNodes;
  $: title =
    environment === "it-dmz"
      ? "Business IT DMZ"
      : environment === "business-it"
        ? "Business IT"
        : environment === "operations-intelligence"
          ? "Operations Intelligence"
          : "Factory OT DMZ";
  $: subtitle =
    viewMode === "physical"
      ? environment === "it-dmz"
        ? "WAN demarcation, perimeter rack, public service bay, and internal handoff"
        : environment === "business-it"
          ? "Office floor plan, secured infrastructure rooms, work areas, and controlled access"
          : environment === "operations-intelligence"
            ? "Central network, security, data, process-analysis, and change-governance workspaces"
            : "Factory-local secure-access, exchange, monitoring, and OT boundary facilities"
      : environment === "it-dmz"
        ? "Public static NAT, perimeter policy, DMZ isolation, and Business IT handoff"
        : environment === "business-it"
          ? "Collapsed core, six internal VLANs, shared services, and explicit trust boundaries"
          : environment === "operations-intelligence"
            ? "Brokered factory data, enterprise decision services, and governed change workflows"
            : "Independent factory policy boundaries and separated Level 3.5 service subzones";

  let viewport: HTMLDivElement;
  let zoom = 0.72;
  let gridVisible = false;
  let selectedId: string | null = null;
  let dragging = false;
  let dragStartX = 0;
  let dragStartY = 0;
  let dragScrollLeft = 0;
  let dragScrollTop = 0;
  let viewportWidth = 1;
  let viewportHeight = 1;

  $: selectedNode = nodes.find((node) => node.id === selectedId) ?? null;
  $: worldPixelWidth = WORLD_WIDTH * zoom;
  $: worldPixelHeight = WORLD_HEIGHT * zoom;
  $: worldOffsetX = Math.max(0, (viewportWidth - worldPixelWidth) / 2);
  $: worldOffsetY = Math.max(0, (viewportHeight - worldPixelHeight) / 2);

  function clampZoom(value: number) {
    return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, value));
  }

  async function setZoom(nextZoom: number, focalX?: number, focalY?: number) {
    if (!viewport) return;
    const targetZoom = clampZoom(Number(nextZoom.toFixed(2)));
    if (targetZoom === zoom) return;

    const focusX = focalX ?? viewport.clientWidth / 2;
    const focusY = focalY ?? viewport.clientHeight / 2;
    const currentOffsetX = Math.max(0, (viewport.clientWidth - WORLD_WIDTH * zoom) / 2);
    const currentOffsetY = Math.max(0, (viewport.clientHeight - WORLD_HEIGHT * zoom) / 2);
    const worldX = (viewport.scrollLeft + focusX - currentOffsetX) / zoom;
    const worldY = (viewport.scrollTop + focusY - currentOffsetY) / zoom;

    zoom = targetZoom;
    await tick();
    if (!viewport) return;

    const nextOffsetX = Math.max(0, (viewport.clientWidth - WORLD_WIDTH * zoom) / 2);
    const nextOffsetY = Math.max(0, (viewport.clientHeight - WORLD_HEIGHT * zoom) / 2);
    viewport.scrollLeft = worldX * zoom + nextOffsetX - focusX;
    viewport.scrollTop = worldY * zoom + nextOffsetY - focusY;
  }

  async function fitToView() {
    if (!viewport) return;
    const compactLayout = viewport.clientWidth < 620;
    const horizontalPadding = viewport.clientWidth < 720 ? 24 : 80;
    const verticalPadding = viewport.clientHeight < 620 ? 24 : 64;
    const fittedZoom = Math.min(
      (viewport.clientWidth - horizontalPadding) / WORLD_WIDTH,
      (viewport.clientHeight - verticalPadding) / WORLD_HEIGHT,
    );
    zoom = Number(
      clampZoom(compactLayout ? Math.max(0.65, fittedZoom) : fittedZoom).toFixed(2),
    );
    await tick();
    if (!viewport) return;
    viewport.scrollLeft = compactLayout
      ? 0
      : Math.max(0, (WORLD_WIDTH * zoom - viewport.clientWidth) / 2);
    viewport.scrollTop = Math.max(0, (WORLD_HEIGHT * zoom - viewport.clientHeight) / 2);
  }

  async function resetView() {
    zoom = 1;
    await tick();
    if (!viewport) return;
    viewport.scrollLeft = 0;
    viewport.scrollTop = 0;
  }

  function selectNode(event: MouseEvent, node: OfficeNode) {
    event.stopPropagation();
    selectedId = node.id;
  }

  function handleWheel(event: WheelEvent) {
    if (!(event.ctrlKey || event.metaKey)) return;
    event.preventDefault();
    const rect = viewport.getBoundingClientRect();
    void setZoom(
      zoom + (event.deltaY > 0 ? -ZOOM_STEP : ZOOM_STEP),
      event.clientX - rect.left,
      event.clientY - rect.top,
    );
  }

  function handlePointerDown(event: PointerEvent) {
    const target = event.target as Element;
    if (target.closest(".lan-device")) return;
    if (event.button === 0 || event.button === 1) {
      selectedId = null;
      event.preventDefault();
      dragging = true;
      dragStartX = event.clientX;
      dragStartY = event.clientY;
      dragScrollLeft = viewport.scrollLeft;
      dragScrollTop = viewport.scrollTop;
      viewport.setPointerCapture(event.pointerId);
    }
  }

  function handlePointerMove(event: PointerEvent) {
    if (!dragging) return;
    viewport.scrollLeft = dragScrollLeft - (event.clientX - dragStartX);
    viewport.scrollTop = dragScrollTop - (event.clientY - dragStartY);
  }

  function handlePointerUp(event: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    if (viewport.hasPointerCapture(event.pointerId)) {
      viewport.releasePointerCapture(event.pointerId);
    }
  }

  function canvasInteractions(node: HTMLDivElement) {
    node.addEventListener("wheel", handleWheel, { passive: false });
    node.addEventListener("pointerdown", handlePointerDown);
    node.addEventListener("pointermove", handlePointerMove);
    node.addEventListener("pointerup", handlePointerUp);
    node.addEventListener("pointercancel", handlePointerUp);
    return {
      destroy() {
        node.removeEventListener("wheel", handleWheel);
        node.removeEventListener("pointerdown", handlePointerDown);
        node.removeEventListener("pointermove", handlePointerMove);
        node.removeEventListener("pointerup", handlePointerUp);
        node.removeEventListener("pointercancel", handlePointerUp);
      },
    };
  }

  function syncViewport() {
    if (!viewport) return;
    viewportWidth = viewport.clientWidth;
    viewportHeight = viewport.clientHeight;
  }

  onMount(() => {
    const observer = new ResizeObserver(syncViewport);
    observer.observe(viewport);
    syncViewport();
    void fitToView();
    return () => observer.disconnect();
  });
</script>

<svelte:head>
  <title>{title} | Hearthline</title>
</svelte:head>

<div class="app-shell">
  <header class="topbar">
    <div class="brand-block">
      <button
        type="button"
        class="brand-back"
        aria-label={`Back to ${siteLabel}`}
        title={`Back to ${siteLabel}`}
        onclick={onBack}
      >
        <ArrowLeft size={18} strokeWidth={1.9} />
      </button>
      <span class="brand-mark" aria-hidden="true"><Network size={20} strokeWidth={1.8} /></span>
      <div class="brand-copy">
        <strong>Hearthline</strong>
        <span>Architecture</span>
      </div>
    </div>

    <div class="view-context" aria-label="Current view">
      <span>{siteLabel}</span>
      <ChevronDown size={14} strokeWidth={1.8} />
      <strong>{title}</strong>
    </div>

    <div class="toolbar" aria-label={`${title} tools`}>
      <div class="view-mode-control" aria-label="Architecture view">
        <button
          type="button"
          class:active={viewMode === "physical"}
          aria-pressed={viewMode === "physical"}
          onclick={() => (viewMode = "physical")}
        >
          <Map size={15} strokeWidth={1.9} />
          <span>Physical</span>
        </button>
        <button
          type="button"
          class:active={viewMode === "logical"}
          aria-pressed={viewMode === "logical"}
          onclick={() => (viewMode = "logical")}
        >
          <Network size={15} strokeWidth={1.9} />
          <span>Logical</span>
        </button>
      </div>

      <span class="toolbar-divider"></span>

      <div class="zoom-control" aria-label="Zoom controls">
        <button type="button" aria-label="Zoom out" title="Zoom out" disabled={zoom <= MIN_ZOOM} onclick={() => setZoom(zoom - ZOOM_STEP)}>
          <Minus size={17} strokeWidth={1.9} />
        </button>
        <button type="button" class="zoom-value" aria-label="Reset zoom" title="Reset zoom" onclick={resetView}>
          {Math.round(zoom * 100)}%
        </button>
        <button type="button" aria-label="Zoom in" title="Zoom in" disabled={zoom >= MAX_ZOOM} onclick={() => setZoom(zoom + ZOOM_STEP)}>
          <Plus size={17} strokeWidth={1.9} />
        </button>
      </div>

      <button type="button" aria-label="Fit to view" title="Fit to view" onclick={fitToView}>
        <Maximize2 size={17} strokeWidth={1.9} />
      </button>
      <button type="button" aria-label="Reset view" title="Reset view" onclick={resetView}>
        <RotateCcw size={17} strokeWidth={1.9} />
      </button>
      <button
        type="button"
        class:active={gridVisible}
        aria-pressed={gridVisible}
        aria-label="Toggle reference grid"
        title="Toggle reference grid"
        onclick={() => (gridVisible = !gridVisible)}
      >
        <Grid2X2 size={17} strokeWidth={1.9} />
      </button>
    </div>
  </header>

  <main class="workspace">
    <div
      class:is-dragging={dragging}
      class="viewport lan-viewport"
      bind:this={viewport}
      use:canvasInteractions
      role="region"
      aria-label={`${title} architecture`}
    >
      <div
        class="canvas-size"
        style={`width: ${Math.max(worldPixelWidth, viewportWidth)}px; height: ${Math.max(worldPixelHeight, viewportHeight)}px;`}
      >
        <section
          class:grid-visible={gridVisible}
          class:physical-view={viewMode === "physical"}
          class:logical-view={viewMode === "logical"}
          class:it-dmz-focus={environment === "it-dmz"}
          class:business-it-focus={environment === "business-it"}
          class:operations-intelligence-focus={environment === "operations-intelligence"}
          class:ot-dmz-focus={environment === "ot-dmz"}
          class="office-focus-world"
          style={`left: ${worldOffsetX}px; top: ${worldOffsetY}px; width: ${WORLD_WIDTH}px; height: ${WORLD_HEIGHT}px; transform: scale(${zoom});`}
          aria-label={`${viewMode} ${title} view`}
        >
          <div class="lan-heading">
            <span>HEARTHLINE / {siteLabel.toUpperCase()} / {environment.toUpperCase()}</span>
            <h1>{title}</h1>
            <p>{subtitle}</p>
          </div>

          <svg class="office-focus-drawing" viewBox={`0 0 ${WORLD_WIDTH} ${WORLD_HEIGHT}`} aria-hidden="true">
            <defs>
              <marker id="office-arrow" markerWidth="9" markerHeight="9" refX="8" refY="4.5" orient="auto">
                <path d="M0,0 L9,4.5 L0,9 Z"></path>
              </marker>
            </defs>

            {#if environment === "it-dmz"}
              <g class="office-physical-layer">
                <rect class="office-floor-shell" x="20" y="145" width="1760" height="635"></rect>
                <rect class="office-room provider-room" x="20" y="170" width="400" height="590"></rect>
                <rect class="office-room perimeter-room" x="435" y="170" width="660" height="590"></rect>
                <rect class="office-room service-room" x="1110" y="170" width="390" height="590"></rect>
                <rect class="office-room internal-room" x="1515" y="170" width="265" height="590"></rect>

                <g class="facility-details">
                  <path class="office-exterior-wall" d="M20 170 H1780 V760 H20 Z"></path>
                  <path class="office-floor-tile" d="M455 245 H1075 M455 315 H1075 M455 655 H1075 M455 725 H1075"></path>
                  <rect class="facility-demarc-cabinet" x="285" y="245" width="84" height="112"></rect>
                  <path class="office-rack" d="M535 250 H625 V365 H535 Z M650 250 H740 V365 H650 Z M765 250 H855 V365 H765 Z"></path>
                  <path class="office-rack-slot" d="M548 270 H612 M548 292 H612 M548 314 H612 M663 270 H727 M663 292 H727 M663 314 H727 M778 270 H842 M778 292 H842 M778 314 H842"></path>
                  <path class="office-cable-tray" d="M390 225 H1465 V680 H1545"></path>
                  <path class="office-door" d="M420 555 H455 M435 555 A35 35 0 0 1 470 590"></path>
                  <path class="office-door" d="M1095 555 H1130 M1110 555 A35 35 0 0 1 1145 590"></path>
                  <path class="office-door" d="M1500 555 H1535 M1515 555 A35 35 0 0 1 1550 590"></path>
                  <path class="office-window" d="M1170 170 H1280 M1315 170 H1425"></path>
                  <text class="office-detail-label" x="295" y="235">SERVICE DEMARC CABINET</text>
                  <text class="office-detail-label" x="535" y="238">PERIMETER RACK ROW</text>
                  <text class="office-detail-label" x="1150" y="720">LOCKED SERVICE CAGE</text>
                </g>

                <path class="office-link provider-link" d="M157 452 H318 M372 452 H533"></path>
                <path class="office-link" d="M587 452 H758 M812 452 H968"></path>
                <path class="office-link" d="M1022 452 H1125 V312 H1228"></path>
                <path class="office-link" d="M1022 452 H1125 V597 H1228 M1282 597 H1578"></path>
                <text class="office-zone-label" x="50" y="210">ISP DEMARCATION</text>
                <text class="office-zone-label" x="465" y="210">PERIMETER EQUIPMENT ROOM</text>
                <text class="office-zone-label" x="1140" y="210">PUBLIC SERVICE BAY</text>
                <text class="office-zone-label" x="1545" y="210">INTERNAL HANDOFF</text>
              </g>
              <g class="office-logical-layer">
                <rect class="office-zone internet-zone" x="20" y="170" width="420" height="590"></rect>
                <rect class="office-zone transit-zone" x="450" y="170" width="460" height="590"></rect>
                <rect class="office-zone dmz-zone" x="920" y="170" width="580" height="590"></rect>
                <rect class="office-zone enterprise-zone" x="1510" y="170" width="270" height="590"></rect>
                <text class="office-zone-label" x="50" y="210">UNTRUSTED · 192.0.2.0/24</text>
                <text class="office-zone-label" x="480" y="210">EDGE TRANSIT · 10.255.0.0/30</text>
                <text class="office-zone-label" x="950" y="210">PUBLIC DMZ · VLAN 10 · 172.16.10.0/24</text>
                <text class="office-zone-label" x="1540" y="210">BUSINESS IT</text>
                <path class="office-flow" d="M225 452 H250 M440 452 H465 M655 452 H690 M880 452 H930"></path>
                <path class="office-flow" d="M1120 452 H1165 V312 H1210" marker-end="url(#office-arrow)"></path>
                <path class="office-flow" d="M1120 452 H1165 V597 H1210 M1400 597 H1500" marker-end="url(#office-arrow)"></path>
                <text class="office-policy-label" x="565" y="350">STATIC NAT · 192.0.2.10 ↔ 172.16.10.2</text>
                <text class="office-policy-label" x="560" y="650">HTTPS · HTTP REDIRECT · LIMITED ICMP</text>
                <text class="office-policy-label" x="1280" y="485">NO UNSOLICITED DMZ → IT</text>
              </g>
            {:else if environment === "business-it"}
              <g class="office-physical-layer">
                <rect class="office-floor-shell" x="25" y="110" width="1750" height="715"></rect>
                <rect class="office-floor-room network-room" x="45" y="170" width="435" height="365"></rect>
                <rect class="office-floor-room reception-room" x="45" y="555" width="435" height="250"></rect>
                <rect class="office-floor-room server-room" x="500" y="135" width="520" height="285"></rect>
                <rect class="office-floor-room open-office-room" x="500" y="440" width="700" height="365"></rect>
                <rect class="office-floor-room operations-room" x="1040" y="135" width="470" height="285"></rect>
                <rect class="office-floor-room meeting-room" x="1220" y="440" width="290" height="365"></rect>
                <rect class="office-floor-room secure-corridor" x="1530" y="135" width="225" height="670"></rect>
                <rect class="office-cable-duct" x="400" y="110" width="1120" height="30"></rect>

                <g class="office-interior">
                  <path class="office-exterior-wall" d="M25 110 H1775 V825 H25 Z"></path>
                  <path class="office-window" d="M545 110 H690 M730 110 H875 M1090 110 H1225 M1265 110 H1400"></path>
                  <path class="office-window" d="M540 825 H670 M710 825 H840 M880 825 H1010 M1050 825 H1180"></path>

                  <path class="office-rack" d="M80 205 H145 V260 H80 Z M160 205 H225 V260 H160 Z M240 205 H305 V260 H240 Z"></path>
                  <path class="office-rack-slot" d="M90 220 H135 M90 235 H135 M170 220 H215 M170 235 H215 M250 220 H295 M250 235 H295"></path>
                  <path class="office-cable-tray" d="M490 140 V795 M1028 140 V795 M1518 140 V795"></path>

                  <path class="office-reception-desk" d="M105 590 H275 V630 H105 Z M275 590 Q330 590 330 645 V680 H292 V642 Q292 630 275 630 Z"></path>
                  <path class="office-lounge" d="M350 610 H440 V665 H350 Z M350 700 H440 V755 H350 Z"></path>
                  <circle class="office-chair" cx="245" cy="730" r="14"></circle>
                  <circle class="office-chair" cx="285" cy="730" r="14"></circle>

                  <path class="office-server-rack" d="M925 170 H985 V355 H925 Z"></path>
                  <path class="office-rack-slot" d="M937 195 H973 M937 220 H973 M937 245 H973 M937 270 H973 M937 295 H973 M937 320 H973"></path>

                  <g class="office-desk-row">
                    <path d="M720 470 H850 V515 H720 Z M905 470 H1035 V515 H905 Z M720 600 H850 V645 H720 Z M905 600 H1035 V645 H905 Z"></path>
                    <circle cx="785" cy="535" r="13"></circle>
                    <circle cx="970" cy="535" r="13"></circle>
                    <circle cx="785" cy="665" r="13"></circle>
                    <circle cx="970" cy="665" r="13"></circle>
                  </g>

                  <path class="office-noc-console" d="M1070 175 H1465 V205 H1070 Z M1070 370 H1465 V400 H1070 Z"></path>
                  <path class="office-display-wall" d="M1130 150 H1215 V175 H1130 Z M1235 150 H1320 V175 H1235 Z M1340 150 H1425 V175 H1340 Z"></path>

                  <path class="office-meeting-table" d="M1280 505 H1450 V650 H1280 Z"></path>
                  <circle class="office-chair" cx="1260" cy="535" r="13"></circle>
                  <circle class="office-chair" cx="1260" cy="610" r="13"></circle>
                  <circle class="office-chair" cx="1470" cy="535" r="13"></circle>
                  <circle class="office-chair" cx="1470" cy="610" r="13"></circle>
                  <path class="office-service-counter" d="M1260 710 H1470 V775 H1260 Z"></path>

                  <path class="office-security-door" d="M1530 280 H1565 M1530 500 H1565"></path>
                  <path class="office-badge-reader" d="M1540 260 H1552 V274 H1540 Z M1540 480 H1552 V494 H1540 Z"></path>
                  <path class="office-door" d="M480 565 H515 M500 565 A35 35 0 0 1 535 600"></path>
                  <path class="office-door" d="M1020 360 H1055 M1040 360 A35 35 0 0 1 1075 395"></path>
                  <path class="office-door" d="M1200 565 H1235 M1220 565 A35 35 0 0 1 1255 600"></path>

                  <text class="office-floor-label" x="70" y="195">SECURE NETWORK ROOM</text>
                  <text class="office-floor-label" x="70" y="580">RECEPTION / GUEST LOUNGE</text>
                  <text class="office-floor-label" x="525" y="160">SERVER ROOM</text>
                  <text class="office-floor-label" x="525" y="465">OPEN OFFICE</text>
                  <text class="office-floor-label" x="1065" y="160">NETWORK OPERATIONS CENTER</text>
                  <text class="office-floor-label" x="1245" y="465">MEETING / SHARED SERVICES</text>
                  <text class="office-floor-label" x="1545" y="160">CONTROLLED OT CORRIDOR</text>
                  <text class="office-detail-label" x="116" y="790">MAIN ENTRY</text>
                  <text class="office-detail-label" x="420" y="129">OVERHEAD STRUCTURED CABLING</text>
                </g>

                <path class="office-link" d="M192 332 H358"></path>
                <path class="office-link" d="M412 332 H500 V242 H593 M500 332 V572 H593 M500 332 V732 H593"></path>
                <path class="office-link" d="M647 242 H750 V202 H858 M750 242 V337 H858"></path>
                <path class="office-link" d="M647 572 H858"></path>
                <path class="office-link" d="M647 732 H750 V702 H858 M750 732 V757 H1078"></path>
                <path class="office-link" d="M358 332 H240 V692 H202"></path>
                <path class="office-link" d="M412 332 H490 V120 H1028 V262 H1148 M1028 120 H1280 V382 H1368"></path>
                <path class="office-link secure-link" d="M412 332 H500 V132 H1510 V412 H1603 M1630 439 V605"></path>
              </g>
              <g class="office-logical-layer">
                <rect class="office-zone perimeter-zone" x="20" y="170" width="470" height="590"></rect>
                <rect class="office-zone server-vlan" x="505" y="80" width="560" height="275"></rect>
                <rect class="office-zone user-vlan" x="505" y="365" width="560" height="190"></rect>
                <rect class="office-zone service-vlan" x="505" y="565" width="560" height="265"></rect>
                <rect class="office-zone guest-vlan" x="1080" y="80" width="330" height="230"></rect>
                <rect class="office-zone management-vlan" x="1080" y="320" width="700" height="245"></rect>
                <rect class="office-zone ot-conduit-zone" x="1080" y="575" width="700" height="255"></rect>
                <text class="office-zone-label" x="50" y="205">ROUTED PERIMETER</text>
                <text class="office-zone-label" x="535" y="112">SERVER ZONES · VLAN 20 INFRA / 80 APP / 90 DATA</text>
                <text class="office-zone-label" x="535" y="397">VLAN 30 · IT_USERS</text>
                <text class="office-zone-label" x="535" y="597">VLAN 40 VOICE / VLAN 50 PRINTERS</text>
                <text class="office-zone-label" x="1110" y="112">VLAN 60 · IT_GUEST</text>
                <text class="office-zone-label" x="1110" y="352">VLAN 70 · IT_MANAGEMENT</text>
                <text class="office-zone-label" x="1110" y="607">OT DMZ CONDUIT · DENY BY DEFAULT</text>
                <path class="office-flow" d="M225 442 H285 M475 442 H535 V207 H590 M475 442 H590 M475 442 H535 V677 H590"></path>
                <path class="office-flow" d="M780 207 H840 V137 H875 M840 207 V282 H875 M780 442 H875"></path>
                <path class="office-flow" d="M780 677 H840 V607 H875 M840 677 V752 H875"></path>
                <path class="office-flow" d="M475 442 H1080 V202 H1190 M1080 442 V402 H1190 M1080 442 V492 H1450"></path>
                <path class="office-flow" d="M475 442 H1080 V702 H1190 M1380 702 H1480" marker-end="url(#office-arrow)"></path>
              </g>
            {:else if environment === "operations-intelligence"}
              <g class="office-physical-layer">
                <rect class="office-floor-shell" x="20" y="110" width="1760" height="720"></rect>
                <rect class="office-floor-room operations-entry-room" x="20" y="170" width="230" height="590"></rect>
                <rect class="office-floor-room central-operations-room" x="260" y="130" width="540" height="680"></rect>
                <rect class="office-floor-room analytics-room" x="810" y="130" width="500" height="680"></rect>
                <rect class="office-floor-room conduit-room" x="1320" y="480" width="200" height="330"></rect>
                <rect class="office-floor-room factory-adjacent-room" x="1530" y="480" width="250" height="330"></rect>

                <g class="office-interior">
                  <path class="office-exterior-wall" d="M20 110 H1780 V830 H20 Z"></path>
                  <path class="office-window" d="M330 110 H460 M500 110 H630 M875 110 H1005 M1045 110 H1175"></path>
                  <path class="office-cable-tray" d="M245 145 H1515 M800 145 V790 M1315 145 V790"></path>

                  <path class="office-display-wall" d="M330 155 H430 V180 H330 Z M450 155 H550 V180 H450 Z M570 155 H670 V180 H570 Z"></path>
                  <path class="office-noc-console" d="M300 310 H755 V350 H300 Z M300 675 H755 V715 H300 Z"></path>
                  <circle class="office-chair" cx="380" cy="375" r="13"></circle>
                  <circle class="office-chair" cx="500" cy="375" r="13"></circle>
                  <circle class="office-chair" cx="620" cy="375" r="13"></circle>
                  <circle class="office-chair" cx="380" cy="650" r="13"></circle>
                  <circle class="office-chair" cx="500" cy="650" r="13"></circle>
                  <circle class="office-chair" cx="620" cy="650" r="13"></circle>

                  <path class="office-server-rack" d="M825 185 H875 V350 H825 Z M890 185 H940 V350 H890 Z"></path>
                  <path class="office-rack-slot" d="M835 205 H865 M835 230 H865 M835 255 H865 M835 280 H865 M900 205 H930 M900 230 H930 M900 255 H930 M900 280 H930"></path>
                  <g class="office-desk-row">
                    <path d="M1010 360 H1140 V405 H1010 Z M1160 360 H1290 V405 H1160 Z M1010 700 H1140 V745 H1010 Z M1160 700 H1290 V745 H1160 Z"></path>
                    <circle cx="1075" cy="425" r="13"></circle>
                    <circle cx="1225" cy="425" r="13"></circle>
                    <circle cx="1075" cy="680" r="13"></circle>
                    <circle cx="1225" cy="680" r="13"></circle>
                  </g>

                  <path class="office-rack" d="M1355 510 H1410 V650 H1355 Z M1430 510 H1485 V650 H1430 Z"></path>
                  <path class="office-rack-slot" d="M1365 535 H1400 M1365 560 H1400 M1365 585 H1400 M1440 535 H1475 M1440 560 H1475 M1440 585 H1475"></path>
                  <path class="office-security-door" d="M1310 610 H1345 M1520 610 H1555"></path>
                  <path class="office-badge-reader" d="M1315 590 H1327 V604 H1315 Z M1525 590 H1537 V604 H1525 Z"></path>

                  <text class="office-floor-label" x="45" y="195">BUSINESS IT HANDOFF</text>
                  <text class="office-floor-label" x="285" y="155">CENTRAL NOC / SOC</text>
                  <text class="office-floor-label" x="835" y="155">OPERATIONS INTELLIGENCE LAB</text>
                  <text class="office-floor-label" x="1340" y="505">SECURE WAN ROOM</text>
                  <text class="office-floor-label" x="1550" y="505">FACTORY BOUNDARY</text>
                </g>

                <path class="office-link" d="M152 452 H245 V232 H338"></path>
                <path class="office-link" d="M392 232 H588 M392 232 H465 V597 H588"></path>
                <path class="office-link" d="M642 232 H918 M972 232 H1168"></path>
                <path class="office-link" d="M642 232 H760 V597 H918 M972 597 H1393 M1447 597 H1618"></path>
                <path class="office-link data-link" d="M1618 570 H1295 V340 H1070 V260 H972" marker-end="url(#office-arrow)"></path>
              </g>
              <g class="office-logical-layer">
                <rect class="office-zone enterprise-zone" x="20" y="170" width="460" height="590"></rect>
                <rect class="office-zone decision-zone" x="490" y="120" width="330" height="690"></rect>
                <rect class="office-zone analytics-zone" x="830" y="120" width="420" height="690"></rect>
                <rect class="office-zone conduit-zone" x="1260" y="450" width="260" height="360"></rect>
                <rect class="office-zone factory-zone" x="1530" y="450" width="250" height="360"></rect>
                <text class="office-zone-label" x="50" y="205">ENTERPRISE IDENTITY AND GOVERNANCE</text>
                <text class="office-zone-label" x="520" y="155">CENTRAL DECISION PLANE</text>
                <text class="office-zone-label" x="860" y="155">BROKERED DATA AND PROCESS ANALYSIS</text>
                <text class="office-zone-label" x="1290" y="485">ENCRYPTED SITE CONDUIT</text>
                <text class="office-zone-label" x="1560" y="485">FACTORY-LOCAL POLICY</text>

                <path class="office-flow" d="M220 452 H245 V232 H270 M460 232 H520 M460 232 H490 V597 H520"></path>
                <path class="office-flow" d="M710 232 H850 M1040 232 H1100"></path>
                <path class="office-flow approved-change-flow" d="M710 232 H780 V597 H850 M1040 597 H1280 M1470 597 H1550" marker-end="url(#office-arrow)"></path>
                <path class="office-flow data-link" d="M1550 515 H1240 V340 H1070 V285 H1040" marker-end="url(#office-arrow)"></path>
                <text class="office-policy-label" x="1160" y="320">BROKERED READ-ORIENTED FACTORY DATA</text>
                <text class="office-policy-label" x="1180" y="640">GOVERNED CHANGE TO FACTORY</text>
              </g>
            {:else}
              <g class="office-physical-layer">
                <rect class="office-floor-shell" x="20" y="110" width="1760" height="730"></rect>
                <rect class="office-room north-boundary-room" x="20" y="175" width="445" height="585"></rect>
                <rect class="office-room dmz-fabric-room" x="480" y="175" width="285" height="585"></rect>
                <rect class="office-room access-room" x="780" y="125" width="390" height="180"></rect>
                <rect class="office-room exchange-room" x="780" y="315" width="390" height="350"></rect>
                <rect class="office-room monitoring-room" x="780" y="675" width="390" height="155"></rect>
                <rect class="office-room south-boundary-room" x="1190" y="175" width="590" height="585"></rect>
                <text class="office-floor-label" x="50" y="145">SECURE LEVEL 3.5 FACILITY</text>

                <g class="facility-details">
                  <path class="office-exterior-wall" d="M20 175 H1780 V760 H20 Z"></path>
                  <path class="office-rack" d="M520 245 H585 V350 H520 Z M605 245 H670 V350 H605 Z"></path>
                  <path class="office-rack-slot" d="M532 265 H573 M532 286 H573 M532 307 H573 M617 265 H658 M617 286 H658 M617 307 H658"></path>
                  <path class="office-cable-tray" d="M235 225 H730 V805 H1180 M1180 225 H1540"></path>
                  <path class="office-security-door" d="M465 530 H500 M765 250 H800 M1170 530 H1205"></path>
                  <path class="office-badge-reader" d="M470 510 H482 V524 H470 Z M770 230 H782 V244 H770 Z M1175 510 H1187 V524 H1175 Z"></path>
                  <path class="office-window secure-window" d="M1280 175 H1390 M1430 175 H1540"></path>
                  <text class="office-detail-label" x="520" y="235">REDUNDANT SWITCH RACKS</text>
                  <text class="office-detail-label" x="1300" y="720">OT SECURITY VESTIBULE</text>
                </g>

                <text class="office-zone-label" x="50" y="210">IT-SIDE SECURITY BOUNDARY</text>
                <text class="office-zone-label" x="510" y="210">REDUNDANT DMZ FABRIC</text>
                <text class="office-zone-label" x="810" y="152">ACCESS SUBZONE</text>
                <text class="office-zone-label" x="810" y="347">EXCHANGE SUBZONE</text>
                <text class="office-zone-label" x="810" y="707">MONITORING SUBZONE</text>
                <text class="office-zone-label" x="1220" y="210">OT-SIDE SECURITY BOUNDARY / LEVEL 3</text>
                <path class="office-link dual-link" d="M125 452 H360 M360 452 H595"></path>
                <path class="office-link dual-link dmz-west-trunk" d="M720 212 V752 M595 452 H720 M720 212 H895 M720 422 H895 M720 567 H895 M720 752 H895"></path>
                <path class="office-link monitor-link" d="M895 752 H1065"></path>
                <path class="office-link dual-link dmz-east-trunk" d="M1160 212 V567 M895 212 H1160 M895 422 H1160 M895 567 H1160 M1160 452 H1380"></path>
                <path class="office-link dual-link" d="M1380 452 H1645"></path>
              </g>
              <g class="office-logical-layer">
                <rect class="office-zone it-source-zone" x="20" y="170" width="450" height="590"></rect>
                <rect class="office-zone access-zone" x="480" y="90" width="700" height="220"></rect>
                <rect class="office-zone exchange-zone" x="480" y="320" width="700" height="330"></rect>
                <rect class="office-zone monitoring-zone" x="480" y="660" width="700" height="180"></rect>
                <rect class="office-zone level-three-zone" x="1190" y="170" width="590" height="590"></rect>
                <text class="office-zone-label" x="50" y="205">CENTRAL OFFICE WAN · NAMED FLOWS</text>
                <text class="office-zone-label" x="510" y="122">ACCESS SUBZONE · SESSION TERMINATION</text>
                <text class="office-zone-label" x="510" y="352">EXCHANGE SUBZONE · BROKERED FLOWS</text>
                <text class="office-zone-label" x="510" y="692">MONITORING SUBZONE · OUT OF BAND</text>
                <text class="office-zone-label" x="1220" y="205">LEVEL 3 OT · SEPARATE AUTHORIZATION</text>
                <path class="office-flow" d="M125 452 H360 M360 452 H595"></path>
                <path class="office-flow dmz-west-trunk" d="M740 187 V547 M595 452 H740 M740 187 H895 M740 367 H895 M740 547 H895"></path>
                <path class="office-flow monitor-flow" d="M595 452 H740 V727 H865 M960 727 H1065"></path>
                <path class="office-flow dmz-east-trunk" d="M1160 187 V547 M895 187 H1160 M895 367 H1160 M895 547 H1160 M1160 452 H1380"></path>
                <path class="office-flow" d="M1380 452 H1645" marker-end="url(#office-arrow)"></path>
                <text class="office-policy-label" x="345" y="350">IDENTITY + DEVICE + REQUEST</text>
                <text class="office-policy-label" x="1375" y="350">INDEPENDENT OT POLICY</text>
              </g>
            {/if}
          </svg>

          {#each nodes as node (node.id)}
            {@const Icon = node.icon}
            {@const position = viewMode === "physical" ? node.physical : node.logical}
            <button
              type="button"
              class:selected={selectedId === node.id}
              class:physical-device-marker={viewMode === "physical"}
              class="lan-device office-device"
              style={`left: ${position.x}px; top: ${position.y}px; --node-accent: ${node.accent};`}
              aria-label={`Inspect ${node.label}, ${node.area}`}
              title={`Inspect ${node.label}`}
              onclick={(event) => selectNode(event, node)}
            >
              {#if viewMode === "physical"}
                <PhysicalDeviceMarker icon={Icon} label={node.label} />
              {:else}
                <span class="node-accent"></span>
                <span class="lan-device-header">
                  <span class="node-icon"><Icon size={19} strokeWidth={1.8} /></span>
                  <small>{node.area}</small>
                </span>
                <strong>{node.label}</strong>
                <span>{node.address}</span>
              {/if}
            </button>
          {/each}

          <div class="lan-key" aria-label={`${title} legend`}>
            {#if viewMode === "physical"}
              <span><Cable size={13} strokeWidth={1.8} /><i class="cable-key copper"></i>Physical link</span>
              {#if environment === "ot-dmz"}
                <span><i class="cable-key redundant"></i>Redundant path</span>
              {/if}
            {:else}
              {#if environment === "operations-intelligence"}
                <span><i class="cable-key data"></i>Brokered factory data</span>
                <span><i class="cable-key change"></i>Approved change workflow</span>
              {:else}
                <span><i class="cable-key copper"></i>Approved conduit</span>
                <span><ShieldCheck size={13} strokeWidth={1.8} />Policy boundary</span>
              {/if}
            {/if}
          </div>
        </section>
      </div>
    </div>

    {#if selectedNode}
      <aside class="lan-inspector" aria-label="Selected office node">
        <div class="lan-inspector-header">
          <div>
            <span>{selectedNode.area}</span>
            <h2>{selectedNode.label}</h2>
          </div>
          <button type="button" aria-label="Close details" title="Close" onclick={() => (selectedId = null)}>
            <X size={17} strokeWidth={1.9} />
          </button>
        </div>
        <p>{selectedNode.role}</p>
        <dl>
          <div>
            <dt>Addressing</dt>
            <dd>{selectedNode.address}</dd>
          </div>
          <div>
            <dt>Environment</dt>
            <dd>{title}</dd>
          </div>
        </dl>
        <div class="lan-port-list">
          <span>Architecture facts</span>
          {#each selectedNode.facts as fact}
            <div><Wifi size={14} strokeWidth={1.8} />{fact}</div>
          {/each}
        </div>
      </aside>
    {/if}
  </main>

  <footer class="statusbar">
    <span class="status-state"><i></i>{title} model</span>
    <span>{nodes.length} nodes / architecture baseline</span>
    <span>{viewMode === "physical" ? "Physical" : "Logical"} / {Math.round(zoom * 100)}%</span>
  </footer>
</div>
