import applianceConfigData from "../../generated/appliance-configs.json";
import type { ViewMode } from "../shared/types";

export interface FrontendAppliance {
  id: string;
  label: string;
  kind: string;
  behaviorFamily: string;
  site: string;
  environment: string;
  zone: string;
  role: string;
  summary: string;
  lifecycle: "design" | "configured" | "simulated";
  tags: string[];
  sourcePath: string;
  sourceYaml: string;
  revision: string;
  addresses: string[];
  interfaceCount: number;
  interfaces: FrontendInterface[];
  services: string[];
  behaviorFacts: string[];
  spanningTree: FrontendSpanningTree | null;
  linkAggregation: FrontendLinkAggregation | null;
  multiChassis: FrontendMultiChassis | null;
  firewallHa: FrontendFirewallHa | null;
}

export interface FrontendSpanningTree {
  protocol: "rapid-pvst";
  bridgePriority: number;
  bridgeMac: string;
}

export interface FrontendLinkAggregation {
  systemMac: string;
  groups: FrontendLinkAggregationGroup[];
}

export interface FrontendLinkAggregationGroup {
  id: string;
  logicalId: string;
  protocol: "lacp";
  mode: "active" | "passive";
  minimumActiveMembers: number;
  members: string[];
}

export interface FrontendMultiChassis {
  domain: string;
  peer: string;
  peerLink: string;
  role: "primary" | "secondary";
}

export interface FrontendFirewallHa {
  domain: string;
  peer: string;
  role: "active" | "standby";
  syncInterface: string;
  monitoredInterfaces: string[];
  sessionSync: boolean;
  heartbeatIntervalMs: number;
  failureHoldMs: number;
}

export interface FrontendInterface {
  id: string;
  hardware: string;
  mode: string;
  administrativeState: "up" | "down";
  initialOperationalState: "up" | "down";
  speedMbps: number;
  duplex: "auto" | "full" | "half";
  mtu: number;
  addresses: string[];
  vlans: number[];
  supportedMedia: string[];
  firstHop: FrontendFirstHop | null;
}

export interface FrontendFirstHop {
  protocol: "vrrp";
  group: number;
  virtualIp: string;
  virtualMac: string;
  priority: number;
  preempt: boolean;
  initialRole: "active" | "standby";
}

export interface FrontendConnectionEndpoint {
  appliance: string;
  interface: string;
  hardware: string;
  administrativeState: "up" | "down";
  initialOperationalState: "up" | "down";
  speedMbps: number;
  duplex: "auto" | "full" | "half";
  mtu: number;
}

export interface FrontendConnection {
  id: string;
  label: string;
  lifecycle: "design" | "configured" | "simulated";
  transport: string;
  medium: string;
  mediumDetail: string;
  endpointA: FrontendConnectionEndpoint;
  endpointB: FrontendConnectionEndpoint;
  capacityMbps: number;
  effectiveMtu: number;
  latencyMs: number;
  physicalDelayUs: number;
  lossEvery: number | null;
  negotiatedDuplex: "full" | "half";
  direction: "bidirectional" | "a-to-b" | "b-to-a";
  configuredOperational: boolean;
  initialOperational: boolean;
  physicalFacts: string[];
  tags: string[];
  sourcePath: string;
  sourceYaml: string;
  revision: string;
}

export interface ApplianceCatalog {
  schemaVersion: string;
  applianceSchemaVersion: string;
  generationStatus: "generated";
  generatedBy: string;
  applianceSourceRoot: string;
  connectionSourceRoot: string;
  appliances: FrontendAppliance[];
  nodeIndex: Record<string, string[]>;
  connections: FrontendConnection[];
  applianceConnectionIndex: Record<string, string[]>;
}

export const SUPPORTED_APPLIANCE_CATALOG_SCHEMA = "0.8.0";
export const SUPPORTED_APPLIANCE_SCHEMA = "0.9.0";
export const SUPPORTED_CONNECTION_SCHEMA = "0.2.0";

let activeCatalog = applianceConfigData as ApplianceCatalog;

if (activeCatalog.schemaVersion !== SUPPORTED_APPLIANCE_CATALOG_SCHEMA) {
  throw new Error(
    `Unsupported appliance catalog schema ${activeCatalog.schemaVersion}; expected ${SUPPORTED_APPLIANCE_CATALOG_SCHEMA}`,
  );
}

if (activeCatalog.applianceSchemaVersion !== SUPPORTED_APPLIANCE_SCHEMA) {
  throw new Error(
    `Unsupported appliance YAML schema ${activeCatalog.applianceSchemaVersion}; expected ${SUPPORTED_APPLIANCE_SCHEMA}`,
  );
}

if (activeCatalog.generationStatus !== "generated") {
  throw new Error(
    `Appliance catalog status ${activeCatalog.generationStatus}; expected generated`,
  );
}

let appliancesById = indexById(activeCatalog.appliances);
let connectionsById = indexById(activeCatalog.connections);

export function installCatalog(catalog: ApplianceCatalog) {
  if (catalog.schemaVersion !== SUPPORTED_APPLIANCE_CATALOG_SCHEMA) {
    throw new Error(
      `Unsupported appliance catalog schema ${catalog.schemaVersion}; expected ${SUPPORTED_APPLIANCE_CATALOG_SCHEMA}`,
    );
  }
  if (catalog.applianceSchemaVersion !== SUPPORTED_APPLIANCE_SCHEMA) {
    throw new Error(
      `Unsupported appliance YAML schema ${catalog.applianceSchemaVersion}; expected ${SUPPORTED_APPLIANCE_SCHEMA}`,
    );
  }
  activeCatalog = catalog;
  appliancesById = indexById(catalog.appliances);
  connectionsById = indexById(catalog.connections);
}

export function getApplianceCatalog() {
  return activeCatalog;
}

export function findAppliance(id: string) {
  return appliancesById.get(id) ?? null;
}

export function findConnection(id: string) {
  return connectionsById.get(id) ?? null;
}

export function isInteractiveWorkstation(id: string) {
  const appliance = findAppliance(id);
  return appliance !== null &&
    ["workstation", "privileged-workstation", "engineering-workstation"].includes(
      appliance.kind,
    ) &&
    appliance.tags.includes("interactive");
}

export function isInteractiveHmi(id: string) {
  const appliance = findAppliance(id);
  return appliance !== null &&
    appliance.kind === "hmi" &&
    appliance.tags.includes("interactive");
}

export function isInteractiveSecurityConsole(id: string) {
  const appliance = findAppliance(id);
  return appliance !== null &&
    appliance.kind === "operations-console" &&
    appliance.tags.includes("interactive");
}

export function findConnectionsForAppliance(id: string) {
  return (activeCatalog.applianceConnectionIndex[id] ?? [])
    .map((connectionId) => findConnection(connectionId))
    .filter(
      (connection): connection is FrontendConnection => connection !== null,
    );
}

export function findAppliancesForNode(
  view: string,
  node: string,
  mode: ViewMode,
) {
  const ids = [
    ...(activeCatalog.nodeIndex[`${view}:${node}:any`] ?? []),
    ...(activeCatalog.nodeIndex[`${view}:${node}:${mode}`] ?? []),
  ];

  return [...new Set(ids)]
    .map((id) => findAppliance(id))
    .filter((appliance): appliance is FrontendAppliance => appliance !== null);
}

function indexById<T extends { id: string }>(items: T[]) {
  return new Map(items.map((item) => [item.id, item]));
}
