export interface HmiSnapshot {
  schemaVersion: string;
  id: string;
  label: string;
  environment: string;
  zone: string;
  role: string;
  controller: string;
  remoteIo: string;
  permissions: string[];
  sequence: number;
  signals: HmiSignal[];
  actuators: HmiActuator[];
  safety: HmiSafety[];
  alarms: HmiAlarm[];
  audit: HmiAuditEntry[];
}

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

export type HmiAction =
  | { kind: "command"; tag: string; value: string }
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
