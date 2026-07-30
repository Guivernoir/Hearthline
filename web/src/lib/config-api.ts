import type { ApplianceCatalog } from "./appliance-config";

interface HealthResponse {
  status: string;
  version: string;
  writeAccess: boolean;
}

interface ErrorResponse {
  error?: string;
}

export async function configurationApiAvailable() {
  try {
    const response = await fetch("/api/health", {
      headers: { Accept: "application/json" },
    });
    if (!response.ok) return false;
    const health = (await response.json()) as HealthResponse;
    return health.status === "ok" && health.writeAccess;
  } catch {
    return false;
  }
}

export function saveAppliance(
  id: string,
  sourceYaml: string,
  expectedRevision: string,
) {
  return saveConfiguration(
    `/api/config/appliances/${encodeURIComponent(id)}`,
    sourceYaml,
    expectedRevision,
  );
}

export function saveConnection(
  id: string,
  sourceYaml: string,
  expectedRevision: string,
) {
  return saveConfiguration(
    `/api/config/connections/${encodeURIComponent(id)}`,
    sourceYaml,
    expectedRevision,
  );
}

async function saveConfiguration(
  endpoint: string,
  sourceYaml: string,
  expectedRevision: string,
): Promise<ApplianceCatalog> {
  const response = await fetch(endpoint, {
    method: "PUT",
    headers: {
      Accept: "application/json",
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ sourceYaml, expectedRevision }),
  });
  if (!response.ok) {
    let message = `Configuration save failed (${response.status})`;
    try {
      const error = (await response.json()) as ErrorResponse;
      if (error.error) message = error.error;
    } catch {
      // Preserve the status-based message when the server did not return JSON.
    }
    throw new Error(message);
  }
  return (await response.json()) as ApplianceCatalog;
}
