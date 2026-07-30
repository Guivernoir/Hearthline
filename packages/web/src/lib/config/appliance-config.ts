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
  generationStatus: "generated";
  generatedBy: string;
  applianceSourceRoot: string;
  connectionSourceRoot: string;
  appliances: FrontendAppliance[];
  nodeIndex: Record<string, string[]>;
  connections: FrontendConnection[];
  applianceConnectionIndex: Record<string, string[]>;
}

export const SUPPORTED_APPLIANCE_CATALOG_SCHEMA = "0.3.0";
export const SUPPORTED_CONNECTION_SCHEMA = "0.2.0";

let activeCatalog = applianceConfigData as ApplianceCatalog;

if (activeCatalog.schemaVersion !== SUPPORTED_APPLIANCE_CATALOG_SCHEMA) {
  throw new Error(
    `Unsupported appliance catalog schema ${activeCatalog.schemaVersion}; expected ${SUPPORTED_APPLIANCE_CATALOG_SCHEMA}`,
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
