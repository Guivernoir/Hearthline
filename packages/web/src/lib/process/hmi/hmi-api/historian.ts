import type { ScenarioReport } from "../../../simulation/simulation-api";

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
