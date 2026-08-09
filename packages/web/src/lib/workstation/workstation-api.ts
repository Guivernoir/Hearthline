import type {
  ScenarioHttpResponse,
  ScenarioReport,
} from "../simulation/simulation-api";

export interface WorkstationInterface {
  id: string;
  hardware: string;
  macAddress: string | null;
  addresses: string[];
  administrativeState: string;
  operationalState: string;
  speedMbps: number;
  mtu: number;
}

export interface WorkstationProfile {
  schemaVersion: string;
  id: string;
  label: string;
  kind: string;
  site: string;
  environment: string;
  zone: string;
  role: string;
  hostname: string;
  browserHome: string | null;
  defaultGateway: string | null;
  dnsServers: string[];
  interfaces: WorkstationInterface[];
  applications: string[];
}

export type WorkstationAction =
  | { kind: "terminal"; command: string }
  | { kind: "browser"; url: string }
  | { kind: "inspect"; appliance: string; command: string };

export interface BrowserNavigation {
  url: string;
  method: string;
  requestBodyBytes: number;
  host: string;
  path: string;
  resolvedAddress: string | null;
  resolutionSource: "dns-query" | "client-cache" | "literal-address";
  gateway: string | null;
  forwardedTo: string | null;
  response: ScenarioHttpResponse | null;
  outcome: "responded" | "denied" | "failed" | "name-resolution-failed";
}

export interface WorkstationArpEntry {
  address: string;
  macAddress: string;
  interface: string;
  remainingTtlMs: number;
}

export interface RuntimeMacEntry {
  vlan: number;
  macAddress: string;
  interface: string;
  remainingTtlMs: number;
}

export interface RuntimeNeighborEntry {
  address: string;
  macAddress: string;
  interface: string;
  state: string;
  remainingTtlMs: number;
}

export interface RuntimePatEntry {
  protocol: string;
  internalAddress: string;
  internalToken: number;
  externalAddress: string;
  externalToken: number;
  remoteAddress: string;
  remotePort: number | null;
  remainingTtlMs: number;
}

export interface RuntimeFirewallSessionEntry {
  protocol: string;
  sourceAddress: string;
  sourcePort: number | null;
  destinationAddress: string;
  destinationPort: number | null;
  remainingTtlMs: number;
}

export interface RuntimeDeviceSnapshot {
  id: string;
  kind: string;
  supportsMacTable: boolean;
  supportsNeighbors: boolean;
  supportsPat: boolean;
  supportsFirewallSessions: boolean;
  macTable: RuntimeMacEntry[];
  neighbors: RuntimeNeighborEntry[];
  patTranslations: RuntimePatEntry[];
  firewallSessions: RuntimeFirewallSessionEntry[];
}

export interface WorkstationNetworkState {
  active: boolean;
  simulatedTimeMs: number;
  arpEntries: WorkstationArpEntry[];
  patTranslations: number;
  devices: RuntimeDeviceSnapshot[];
}

export interface WorkstationActionReport {
  schemaVersion: string;
  workstationId: string;
  action: "terminal" | "browser" | "inspect";
  status: "completed" | "succeeded" | "denied" | "failed" | "unsupported";
  title: string;
  output: string[];
  clearOutput: boolean;
  browser: BrowserNavigation | null;
  simulations: ScenarioReport[];
  networkState: WorkstationNetworkState;
}

interface ErrorResponse {
  error?: string;
}

export function loadWorkstationProfile(id: string): Promise<WorkstationProfile> {
  return requestJson(`/api/workstations/${encodeURIComponent(id)}`);
}

export function runWorkstationAction(
  id: string,
  action: WorkstationAction,
): Promise<WorkstationActionReport> {
  return requestJson(`/api/workstations/${encodeURIComponent(id)}/actions`, {
    method: "POST",
    headers: {
      Accept: "application/json",
      "Content-Type": "application/json",
    },
    body: JSON.stringify(action),
  });
}

async function requestJson<T>(
  endpoint: string,
  init?: RequestInit,
): Promise<T> {
  const response = await fetch(endpoint, {
    headers: { Accept: "application/json" },
    ...init,
  });
  if (!response.ok) {
    let message = `Workstation request failed (${response.status})`;
    try {
      const body = (await response.json()) as ErrorResponse;
      if (body.error) message = body.error;
    } catch {
      // Keep the status-based message when the API did not return JSON.
    }
    throw new Error(message);
  }
  return (await response.json()) as T;
}
