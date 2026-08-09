export interface HmiSnapshot {
  schemaVersion: string;
  id: string;
  label: string;
  environment: string;
  zone: string;
  role: string;
  interfaceKind: "hmi" | "scada-workstation";
  controller: string;
  remoteIo: string;
  permissions: string[];
  sequence: number;
  controlProgram: HmiControlProgramState | null;
  process: HmiProcessState | null;
  signals: HmiSignal[];
  actuators: HmiActuator[];
  safety: HmiSafety[];
  alarms: HmiAlarm[];
  audit: HmiAuditEntry[];
}

export interface HmiControlProgramState {
  language: "structured-text";
  program: string;
  task: string;
  sourcePath: string;
  bindingPath: string;
  revision: string;
  currentStep: number;
  scanIntervalMs: number;
  watchdogMs: number;
}

export interface HmiControlProgramDocument {
  schemaVersion: string;
  controller: string;
  language: "structured-text";
  program: string;
  task: string;
  sourcePath: string;
  bindingPath: string;
  revision: string;
  source: string;
  bindingYaml: string;
}

export interface HmiProcessState {
  model: string;
  phase: string;
  running: boolean;
  phaseElapsedMs: number;
  scanCount: number;
  cycleCount: number;
  fault: HmiProcessFault | null;
  phases: HmiProcessPhase[];
}

export interface HmiProcessPhase {
  key: string;
  label: string;
}

export type HmiProcessFault =
  | "slip-supply-loss"
  | "compressed-air-loss"
  | "mould-overpressure"
  | "vacuum-loss"
  | "robot-pickup-failure";

export interface HmiSignal {
  componentId: string;
  label: string;
  tag: string;
  unit: string;
  minimum: number;
  maximum: number;
  value: number;
  qualityGood: boolean;
  timestampMs: number;
}

export interface HmiActuator {
  componentId: string;
  label: string;
  commandTag: string;
  feedbackTag: string | null;
  safeState: string;
  states: string[];
  currentState: string;
}

export interface HmiSafety {
  componentId: string;
  label: string;
  permissives: HmiPermissive[];
  tripLatched: boolean;
}

export interface HmiPermissive {
  tag: string;
  satisfied: boolean;
}

export interface HmiAlarm {
  id: string;
  code: string;
  source: string;
  message: string;
  severity: "warning" | "trip";
  active: boolean;
  acknowledged: boolean;
  sequence: number;
}

export interface HmiAuditEntry {
  sequence: number;
  action: string;
  target: string;
  result: string;
}

export interface HmiTraceEntry {
  sequence: number;
  component: string;
  stage: string;
  detail: string;
}

export interface HistorianRecord {
  source: string;
  sequence: number;
  capturedAtMs: number;
  phase: string;
  cycle: number;
  payload: string;
  wireLengthBytes: number;
}

export interface HistorianTierStatus {
  applianceId: string;
  storedRecords: number;
  capacity: number;
  latest: HistorianRecord | null;
}

export interface HistorianStatus {
  schemaVersion: string;
  sampleIntervalMs: number;
  local: HistorianTierStatus;
  replica: HistorianTierStatus;
  pendingRecords: number;
  droppedUnreplicated: number;
  replicationAttempts: number;
  lastError: string | null;
  lastCollection: ScenarioReport | null;
  lastReplication: ScenarioReport | null;
  lastPublication: ScenarioReport | null;
}

export type HmiAction =
  | { kind: "command"; tag: string; value: string }
  | { kind: "start-process" }
  | { kind: "reset-process" }
  | { kind: "set-process-fault"; fault: HmiProcessFault; active: boolean }
  | { kind: "reset-safety"; safetyId: string }
  | { kind: "acknowledge-alarm"; alarmId: string };

export interface HmiActionReport {
  schemaVersion: string;
  status: "applied" | "completed" | "denied";
  message: string;
  trace: HmiTraceEntry[];
  snapshot: HmiSnapshot;
}

interface ErrorResponse {
  error?: string;
}

export function loadHmiSnapshot(id: string): Promise<HmiSnapshot> {
  return requestJson(`/api/hmis/${encodeURIComponent(id)}`);
}

export function loadHmiControlProgram(
  id: string,
): Promise<HmiControlProgramDocument> {
  return requestJson(`/api/hmis/${encodeURIComponent(id)}/program`);
}

export function runHmiAction(
  id: string,
  action: HmiAction,
): Promise<HmiActionReport> {
  return requestJson(`/api/hmis/${encodeURIComponent(id)}/actions`, {
    method: "POST",
    headers: {
      Accept: "application/json",
      "Content-Type": "application/json",
    },
    body: JSON.stringify(action),
  });
}

export function publishHmiTelemetry(id: string): Promise<ScenarioReport> {
  return requestJson(`/api/hmis/${encodeURIComponent(id)}/telemetry`, {
    method: "POST",
    headers: { Accept: "application/json" },
  });
}

export function loadHistorianStatus(id: string): Promise<HistorianStatus> {
  return requestJson(`/api/hmis/${encodeURIComponent(id)}/historian`);
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
    let message = `HMI request failed (${response.status})`;
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
import type { ScenarioReport } from "../../simulation/simulation-api";
