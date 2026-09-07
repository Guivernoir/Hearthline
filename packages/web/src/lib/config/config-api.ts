import type { ApplianceCatalog } from "./appliance-config";
import { ModelApiClient, ModelApiError } from "../../generated/model-api";
import type { DraftCreated } from "../../generated/model-api";
export type {
  CapacityAssessment,
  CapacityDelta,
  DraftDiagnostic,
  DraftPreview,
} from "../../generated/model-api";

interface ErrorResponse {
  error?: string;
}

const modelApi = new ModelApiClient();

export async function configurationApiAvailable() {
  try {
    const health = await modelApi.health();
    return health.status === "ok" && health.writeAccess;
  } catch {
    return false;
  }
}

export function saveAppliance(sourcePath: string, sourceYaml: string) {
  return saveConfiguration(sourcePath, sourceYaml, "Update appliance configuration");
}

export function saveConnection(sourcePath: string, sourceYaml: string) {
  return saveConfiguration(sourcePath, sourceYaml, "Update connection configuration");
}

export async function createModelDraft() {
  return modelApi.createDraft();
}

export function loadModelDocuments() {
  return modelApi.documents();
}

export async function updateModelDraft(
  draft: number,
  path: string,
  source: string | null,
) {
  await modelApi.updateDraft(draft, { path: normalizeSourcePath(path), source });
}

export function previewModelDraft(draft: number) {
  return modelApi.previewDraft(draft);
}

export async function commitModelDraft(draft: DraftCreated, reason: string) {
  await modelApi.commitDraft(draft.id, {
    expectedRevision: draft.baseRevision,
    reason,
  });
}

export async function discardModelDraft(draft: number) {
  try {
    await modelApi.discardDraft(draft);
  } catch (error) {
    if (!(error instanceof ModelApiError) || error.status !== 404) throw error;
  }
}

async function saveConfiguration(
  sourcePath: string,
  sourceYaml: string,
  reason: string,
): Promise<ApplianceCatalog> {
  const draft = await createModelDraft();
  let committed = false;
  try {
    await updateModelDraft(draft.id, sourcePath, sourceYaml);
    const preview = await previewModelDraft(draft.id);
    const errors = preview.diagnostics.filter((item) => item.severity === "error");
    if (errors.length > 0 || !preview.candidateRevision) {
      throw new Error(
        errors.map((item) => item.message).join("\n") || "Model compilation failed",
      );
    }
    await commitModelDraft(draft, reason);
    committed = true;
    return requestJson<ApplianceCatalog>("/api/config/catalog");
  } finally {
    if (!committed) await discardModelDraft(draft.id).catch(() => undefined);
  }
}

function normalizeSourcePath(path: string) {
  const normalized = path.replaceAll("\\", "/").replace(/^\/+/, "");
  return normalized.startsWith("project/") ? normalized : `project/${normalized}`;
}

async function requestJson<T>(endpoint: string, init?: RequestInit): Promise<T> {
  const response = await fetch(endpoint, {
    ...init,
    headers: { Accept: "application/json", ...init?.headers },
  });
  if (!response.ok) await throwResponseError(response);
  if (response.status === 204) return undefined as T;
  return (await response.json()) as T;
}

async function throwResponseError(response: Response): Promise<never> {
  let message = `Model operation failed (${response.status})`;
  try {
    const error = (await response.json()) as ErrorResponse;
    if (error.error) message = error.error;
  } catch {
    // Preserve the status-based message when the server did not return JSON.
  }
  throw new Error(message);
}
