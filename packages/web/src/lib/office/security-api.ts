import type { ScenarioSecurityEvent } from "../simulation/simulation-api";

export interface SecurityEventRecord {
  id: number;
  receivedSequence: number;
  acknowledged: boolean;
  event: ScenarioSecurityEvent;
}

export interface SecurityConsoleSession {
  schemaVersion: string;
  consoleId: string;
  sequence: number;
  activeCount: number;
  acknowledgedCount: number;
  events: SecurityEventRecord[];
}

interface ErrorResponse {
  error?: string;
}

export function loadSecurityConsole(
  id: string,
): Promise<SecurityConsoleSession> {
  return requestJson(`/api/security/consoles/${encodeURIComponent(id)}`);
}

export function acknowledgeSecurityEvent(
  id: number,
): Promise<SecurityEventRecord> {
  return requestJson(
    `/api/security/events/${encodeURIComponent(String(id))}/acknowledge`,
    { method: "POST" },
  );
}

export function clearSecurityConsole(
  id: string,
): Promise<SecurityConsoleSession> {
  return requestJson(
    `/api/security/consoles/${encodeURIComponent(id)}/clear`,
    { method: "POST" },
  );
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
    let message = `Security console request failed (${response.status})`;
    try {
      const body = (await response.json()) as ErrorResponse;
      if (body.error) message = body.error;
    } catch {
      // Preserve the status-based message when the API did not return JSON.
    }
    throw new Error(message);
  }
  return (await response.json()) as T;
}
