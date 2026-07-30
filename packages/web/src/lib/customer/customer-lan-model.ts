import type { Component } from "svelte";
import { Monitor, Network, Router } from "@lucide/svelte";

interface DevicePosition {
  x: number;
  y: number;
}

export interface CustomerDevice {
  id: string;
  label: string;
  role: string;
  area: string;
  address: string;
  ports: string[];
  accent: string;
  icon: Component<any>;
  physical: DevicePosition;
  logical: DevicePosition;
}

export const customerDevices: CustomerDevice[] = [
  {
    id: "pc-01",
    label: "Customer PC-01",
    role: "Customer workstation",
    area: "Home office",
    address: "192.168.0.2/24",
    ports: ["FastEthernet0 → SW-01 Fa0/1", "Gateway 192.168.0.1"],
    accent: "#3567a6",
    icon: Monitor,
    physical: { x: 145, y: 270 },
    logical: { x: 80, y: 240 },
  },
  {
    id: "pc-02",
    label: "Customer PC-02",
    role: "Customer workstation",
    area: "Living area",
    address: "192.168.0.3/24",
    ports: ["FastEthernet0 → SW-01 Fa0/2", "Gateway 192.168.0.1"],
    accent: "#3567a6",
    icon: Monitor,
    physical: { x: 145, y: 510 },
    logical: { x: 80, y: 490 },
  },
  {
    id: "sw-01",
    label: "Customer SW-01",
    role: "Layer 2 access switch",
    area: "Network cabinet",
    address: "VLAN 1 / Layer 2",
    ports: ["Fa0/1 PC-01", "Fa0/2 PC-02", "Gi0/1 RTR-01"],
    accent: "#267168",
    icon: Network,
    physical: { x: 500, y: 380 },
    logical: { x: 400, y: 365 },
  },
  {
    id: "rtr-01",
    label: "Customer RTR-01",
    role: "Default gateway and PAT edge",
    area: "Network cabinet",
    address: "192.168.0.1/24 · 203.0.113.2/24",
    ports: ["Gi0/0 SW-01 Gi0/1", "Gi0/1 INET-CPE-01 customer port"],
    accent: "#267168",
    icon: Router,
    physical: { x: 850, y: 380 },
    logical: { x: 780, y: 365 },
  },
];
