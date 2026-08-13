import type { ScenarioReport } from "../../../simulation/simulation-api";
import type { HmiAction, HmiActionReport } from "./actions";
import type { HmiControlProgramDocument, HmiSnapshot } from "./core";
import type { HistorianStatus } from "./historian";

export * from "./actions";
export * from "./core";
export * from "./historian";
export * from "./robot";
export * from "./supervisory";

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
