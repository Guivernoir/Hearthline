import processViewData from "../generated/process-view.json";

export type ProcessNodeKind = "boundary" | "platform" | "process";

export type ProcessIconKey =
  | "boxes"
  | "check-circle"
  | "cpu"
  | "database"
  | "droplets"
  | "eye"
  | "factory"
  | "fan"
  | "file-check"
  | "flame"
  | "gauge"
  | "globe"
  | "monitor"
  | "network"
  | "package"
  | "paintbrush"
  | "remote-io"
  | "router"
  | "scan"
  | "server"
  | "shield"
  | "snowflake"
  | "thermometer"
  | "truck"
  | "valve"
  | "wind";

export interface ProcessPosition {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface ProcessSupportNode {
  id: string;
  label: string;
  subtitle: string;
  zone: string;
  accent: string;
  icon: ProcessIconKey;
  tags: string[];
  detail: string;
  position: ProcessPosition;
  kind: Exclude<ProcessNodeKind, "process">;
}

export interface ProcessEquipment {
  id: string;
  label: string;
  kind: string;
  role: string;
  icon: ProcessIconKey;
  accent: string;
  slot:
    | "switch"
    | "controller"
    | "hmi"
    | "remote-io"
    | "sensor-a"
    | "sensor-b"
    | "actuator-a"
    | "actuator-b"
    | "safety";
  linkKind: "ethernet" | "io" | "safety-status";
  upstream: string | null;
  physicalUpstream?: string | null;
  configRef: string;
  facts: string[];
  physical?: {
    label: string;
    kind: string;
    role: string;
    icon: ProcessIconKey;
    configRefs: string[];
    facts: string[];
  };
}

export interface ProcessArea {
  id: string;
  routeKey: string;
  label: string;
  subtitle: string;
  zone: string;
  accent: string;
  icon: ProcessIconKey;
  tags: string[];
  detail: string;
  position: ProcessPosition;
  equipment: ProcessEquipment[];
}

export interface ProcessViewModel {
  schemaVersion: string;
  generationStatus: "bootstrap" | "generated";
  generatedBy: string;
  sourceRoot: string;
  supportNodes: ProcessSupportNode[];
  areas: ProcessArea[];
}

export const SUPPORTED_PROCESS_VIEW_SCHEMA = "0.2.0";

const candidateProcessView = processViewData as ProcessViewModel;

if (candidateProcessView.schemaVersion !== SUPPORTED_PROCESS_VIEW_SCHEMA) {
  throw new Error(
    `Unsupported process view schema ${candidateProcessView.schemaVersion}; expected ${SUPPORTED_PROCESS_VIEW_SCHEMA}`,
  );
}

export const processView = candidateProcessView;

export function findProcessArea(routeKey: string) {
  return processView.areas.find((area) => area.routeKey === routeKey) ?? null;
}
