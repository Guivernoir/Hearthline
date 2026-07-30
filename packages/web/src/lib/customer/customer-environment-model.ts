import type { Component } from "svelte";
import {
  Cloud,
  Database,
  Globe2,
  RadioTower,
  Router,
  Server,
  ShieldCheck,
  Wifi,
} from "@lucide/svelte";

export type CustomerEnvironment = "edge" | "public-web-path";

interface NodePosition {
  x: number;
  y: number;
}

export interface EnvironmentNode {
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

export const edgeNodes: EnvironmentNode[] = [
  {
    id: "customer-lan",
    label: "Customer LAN",
    role: "Adjacent private network terminating at the router inside interface",
    area: "Inside network",
    address: "VLAN 1 / 192.168.0.0/24",
    facts: ["Detailed in the Customer LAN view", "Handoff terminates at RTR-01 Gi0/0"],
    accent: "#3567a6",
    icon: Wifi,
    physical: { x: 110, y: 400 },
    logical: { x: 110, y: 400 },
  },
  {
    id: "customer-router",
    label: "Customer RTR-01",
    role: "Default gateway, routing, and PAT boundary",
    area: "Customer edge",
    address: "192.168.0.1/24 · 203.0.113.2/24",
    facts: ["Gi0/0 NAT inside", "Gi0/1 NAT outside", "Default route → 203.0.113.1"],
    accent: "#267168",
    icon: Router,
    physical: { x: 450, y: 400 },
    logical: { x: 450, y: 400 },
  },
  {
    id: "customer-modem",
    label: "Customer INET-CPE-01",
    role: "Provider access CPE and media termination",
    area: "Service demarcation",
    address: "No Layer 3 address",
    facts: ["Customer port → RTR-01 Gi0/1", "Access port → WAN-01"],
    accent: "#7a6546",
    icon: RadioTower,
    physical: { x: 790, y: 400 },
    logical: { x: 790, y: 400 },
  },
  {
    id: "wan-01",
    label: "WAN-01",
    role: "Customer-facing provider access circuit",
    area: "Access network",
    address: "203.0.113.0/24 service handoff",
    facts: ["Access technology is provider-specific", "No customer routing function"],
    accent: "#6a5c91",
    icon: Cloud,
    physical: { x: 1130, y: 400 },
    logical: { x: 1130, y: 400 },
  },
  {
    id: "isp-router",
    label: "ISP EDGE-RTR-01 / 02",
    role: "Redundant provider gateway role",
    area: "Provider edge",
    address: "203.0.113.1/24",
    facts: ["Customer-facing gateway VIP", "Provider core is abstracted from this view"],
    accent: "#b65034",
    icon: Globe2,
    physical: { x: 1470, y: 400 },
    logical: { x: 1470, y: 400 },
  },
];

export const publicPathNodes: EnvironmentNode[] = [
  {
    id: "customer-router",
    label: "Customer RTR-01",
    role: "Originating customer edge and PAT source",
    area: "Customer premises",
    address: "203.0.113.2/24",
    facts: ["Private source translated with PAT", "Default route → 203.0.113.1"],
    accent: "#3567a6",
    icon: Router,
    physical: { x: 70, y: 405 },
    logical: { x: 45, y: 405 },
  },
  {
    id: "wan-01",
    label: "WAN-01",
    role: "Customer-facing access network",
    area: "Customer WAN",
    address: "203.0.113.0/24",
    facts: ["Provider access service", "Customer to provider edge"],
    accent: "#6a5c91",
    icon: Cloud,
    physical: { x: 300, y: 405 },
    logical: { x: 285, y: 405 },
  },
  {
    id: "isp-router",
    label: "ISP EDGE-RTR-01 / 02",
    role: "Redundant provider edge and service routing role",
    area: "ISP core",
    address: "203.0.113.1 · 198.51.100.1 · 192.0.2.1",
    facts: ["Customer gateway VIP", "Provider service routing", "Business-facing provider gateway"],
    accent: "#267168",
    icon: Globe2,
    physical: { x: 560, y: 405 },
    logical: { x: 525, y: 405 },
  },
  {
    id: "isp-dns",
    label: "ISP-DNS-01 / 02",
    role: "Redundant public authoritative DNS role",
    area: "ISP services",
    address: "198.51.100.50/24",
    facts: ["www.business.example → 192.0.2.10", "Authoritative service redundancy", "DNSSEC is part of the target design"],
    accent: "#426d9d",
    icon: Database,
    physical: { x: 560, y: 555 },
    logical: { x: 525, y: 650 },
  },
  {
    id: "wan-02",
    label: "WAN-02A / 02B",
    role: "Diverse business-facing provider circuits",
    area: "Business WAN",
    address: "192.0.2.0/24",
    facts: ["Independent carrier paths are required", "Shared documentation prefix and gateway role"],
    accent: "#6a5c91",
    icon: Cloud,
    physical: { x: 820, y: 405 },
    logical: { x: 795, y: 405 },
  },
  {
    id: "business-edge",
    label: "Business EDGE-RTR-01 / 02",
    role: "Redundant enterprise edge and static NAT role",
    area: "Business perimeter",
    address: "192.0.2.2/24 · 10.255.0.1/30",
    facts: ["192.0.2.10 ↔ 172.16.10.2", "DMZ route → 10.255.0.2"],
    accent: "#b65034",
    icon: Router,
    physical: { x: 1080, y: 405 },
    logical: { x: 1065, y: 405 },
  },
  {
    id: "business-firewall",
    label: "Business FRW-01A / 01B",
    role: "High-availability perimeter policy enforcement",
    area: "DMZ boundary",
    address: "10.255.0.2/30 · 172.16.10.1/24",
    facts: ["Permit TCP/443", "TCP/80 may redirect to HTTPS only", "Unmatched inbound traffic denied"],
    accent: "#9e3f2f",
    icon: ShieldCheck,
    physical: { x: 1325, y: 405 },
    logical: { x: 1335, y: 405 },
  },
  {
    id: "business-web",
    label: "Business WEB-GW-01 / 02",
    role: "Public reverse proxy, TLS termination, and web application firewall",
    area: "Public DMZ",
    address: "172.16.10.2/24 · public 192.0.2.10",
    facts: [
      "HTTPS is the public service",
      "HTTP is redirect-only when enabled",
      "Only named internal application dependencies are proxied",
    ],
    accent: "#51704c",
    icon: Server,
    physical: { x: 1570, y: 405 },
    logical: { x: 1575, y: 405 },
  },
];
