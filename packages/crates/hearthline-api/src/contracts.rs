use std::path::PathBuf;

use hearthline_project::{
    BlueprintDefinition, BlueprintInstance, DraftChange, DraftId, DraftPreview,
};
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

pub const MODEL_API_CONTRACT_VERSION: &str = "0.1.0";

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub write_access: bool,
    pub model_revision: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelSummary {
    pub schema_version: String,
    pub revision: String,
    pub compiler_version: String,
    pub appliance_count: usize,
    pub connection_count: usize,
    pub scenario_count: usize,
    pub cell_count: usize,
    pub blueprint_instance_count: usize,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelDocument {
    pub kind: String,
    pub path: String,
    pub revision: String,
    pub source: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelDocumentCatalog {
    pub model_revision: String,
    pub documents: Vec<ModelDocument>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftCreated {
    pub id: DraftId,
    pub base_revision: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftCommitRequest {
    pub expected_revision: String,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftCommitResponse {
    pub draft: DraftId,
    pub previous_revision: String,
    pub revision: String,
    pub changed_paths: Vec<String>,
    pub stale_session_count: usize,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftUpdateResponse {
    pub draft: DraftId,
    pub accepted: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SimulationSessionResponse {
    pub id: u64,
    pub model_revision: String,
    pub status: String,
    pub cell_count: usize,
    pub conduit_count: usize,
}

pub fn generated_contract_files() -> Result<Vec<(PathBuf, String)>, serde_json::Error> {
    let files = vec![
        (
            PathBuf::from("project/contracts/openapi.json"),
            pretty(&openapi_document())?,
        ),
        (
            PathBuf::from("project/contracts/schemas/blueprint-definition.schema.json"),
            pretty(&schema_value::<BlueprintDefinition>())?,
        ),
        (
            PathBuf::from("project/contracts/schemas/blueprint-instance.schema.json"),
            pretty(&schema_value::<BlueprintInstance>())?,
        ),
        (
            PathBuf::from("project/contracts/schemas/draft-change.schema.json"),
            pretty(&schema_value::<DraftChange>())?,
        ),
        (
            PathBuf::from("project/contracts/schemas/draft-preview.schema.json"),
            pretty(&schema_value::<DraftPreview>())?,
        ),
        (
            PathBuf::from("project/contracts/schemas/model-document-catalog.schema.json"),
            pretty(&schema_value::<ModelDocumentCatalog>())?,
        ),
        (
            PathBuf::from("packages/web/src/generated/model-api.ts"),
            typescript_source().into(),
        ),
    ];
    Ok(files)
}

fn openapi_document() -> Value {
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "Hearthline local model API",
            "version": MODEL_API_CONTRACT_VERSION,
        },
        "servers": [{ "url": "http://127.0.0.1:3001" }],
        "paths": {
            "/api/health": {
                "get": operation("Read API and model health", "readHealth", None, Some("HealthResponse"), false)
            },
            "/api/model": {
                "get": operation("Read immutable model revision", "readModel", None, Some("ModelSummary"), false)
            },
            "/api/model/documents": {
                "get": operation("Read locked editable model documents", "readModelDocuments", None, Some("ModelDocumentCatalog"), false)
            },
            "/api/model/drafts": {
                "post": operation("Create revisioned model draft", "createModelDraft", None, Some("DraftCreated"), false)
            },
            "/api/model/drafts/{id}/documents": {
                "put": operation("Apply document to draft overlay", "updateModelDraft", Some("DraftChange"), Some("DraftUpdateResponse"), true)
            },
            "/api/model/drafts/{id}/preview": {
                "post": operation("Compile complete draft overlay", "previewModelDraft", None, Some("DraftPreview"), true)
            },
            "/api/model/drafts/{id}/commit": {
                "post": operation("Atomically commit validated draft", "commitModelDraft", Some("DraftCommitRequest"), Some("DraftCommitResponse"), true)
            },
            "/api/model/drafts/{id}": {
                "delete": operation("Discard a model draft", "discardModelDraft", None, None, true)
            },
            "/api/model/sessions": {
                "post": operation("Create model-pinned simulation session", "createSimulationSession", None, Some("SimulationSessionResponse"), false)
            },
            "/api/model/sessions/{id}": {
                "get": operation("Read current or stale session status", "readSimulationSession", None, Some("SimulationSessionResponse"), true)
            },
            "/api/model/sessions/{id}/restart": {
                "post": operation("Restart a session on the current model", "restartSimulationSession", None, Some("SimulationSessionResponse"), true)
            },
        },
        "components": {
            "schemas": openapi_schemas()
        }
    })
}

fn operation(
    summary: &str,
    operation_id: &str,
    request: Option<&str>,
    response: Option<&str>,
    has_id: bool,
) -> Value {
    let status = if response.is_some() { "200" } else { "204" };
    let mut operation = json!({
        "summary": summary,
        "operationId": operation_id,
        "responses": {
            (status): {
                "description": "Successful operation"
            }
        }
    });
    if let Some(response) = response {
        operation["responses"][status]["content"] = json!({
            "application/json": {
                "schema": { "$ref": format!("#/components/schemas/{response}") }
            }
        });
    }
    if let Some(request) = request {
        operation["requestBody"] = json!({
            "required": true,
            "content": {
                "application/json": {
                    "schema": { "$ref": format!("#/components/schemas/{request}") }
                }
            }
        });
    }
    if has_id {
        operation["parameters"] = json!([{
            "name": "id",
            "in": "path",
            "required": true,
            "schema": {
                "type": "integer",
                "format": "int64",
                "minimum": 0
            }
        }]);
    }
    operation
}

fn openapi_schemas() -> Value {
    let mut schemas = Map::new();
    add_openapi_schema::<BlueprintDefinition>(&mut schemas, "BlueprintDefinition");
    add_openapi_schema::<BlueprintInstance>(&mut schemas, "BlueprintInstance");
    add_openapi_schema::<DraftChange>(&mut schemas, "DraftChange");
    add_openapi_schema::<DraftPreview>(&mut schemas, "DraftPreview");
    add_openapi_schema::<DraftCreated>(&mut schemas, "DraftCreated");
    add_openapi_schema::<DraftCommitRequest>(&mut schemas, "DraftCommitRequest");
    add_openapi_schema::<DraftCommitResponse>(&mut schemas, "DraftCommitResponse");
    add_openapi_schema::<DraftUpdateResponse>(&mut schemas, "DraftUpdateResponse");
    add_openapi_schema::<HealthResponse>(&mut schemas, "HealthResponse");
    add_openapi_schema::<ModelSummary>(&mut schemas, "ModelSummary");
    add_openapi_schema::<ModelDocument>(&mut schemas, "ModelDocument");
    add_openapi_schema::<ModelDocumentCatalog>(&mut schemas, "ModelDocumentCatalog");
    add_openapi_schema::<SimulationSessionResponse>(&mut schemas, "SimulationSessionResponse");
    Value::Object(schemas)
}

fn add_openapi_schema<T: JsonSchema>(schemas: &mut Map<String, Value>, name: &str) {
    let mut root = schema_value::<T>();
    let object = root
        .as_object_mut()
        .expect("generated JSON Schema root is an object");
    object.remove("$schema");
    object.remove("title");
    let definitions = object
        .remove("$defs")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    rewrite_schema_references(&mut root);
    insert_identical_schema(schemas, name, root);
    for (definition, mut schema) in definitions {
        rewrite_schema_references(&mut schema);
        insert_identical_schema(schemas, &definition, schema);
    }
}

fn insert_identical_schema(schemas: &mut Map<String, Value>, name: &str, schema: Value) {
    if let Some(existing) = schemas.get(name) {
        assert_eq!(
            existing, &schema,
            "conflicting generated schema named {name}"
        );
    } else {
        schemas.insert(name.into(), schema);
    }
}

fn rewrite_schema_references(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get_mut("$ref")
                && let Some(name) = reference.strip_prefix("#/$defs/")
            {
                *reference = format!("#/components/schemas/{name}");
            }
            for nested in object.values_mut() {
                rewrite_schema_references(nested);
            }
        }
        Value::Array(items) => {
            for item in items {
                rewrite_schema_references(item);
            }
        }
        _ => {}
    }
}

fn schema_value<T: JsonSchema>() -> Value {
    serde_json::to_value(schema_for!(T)).expect("JSON Schema serializes")
}

fn pretty(value: &Value) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(value).map(|source| format!("{source}\n"))
}

fn typescript_source() -> &'static str {
    r#"// Generated by `cargo xtask contracts --write`; do not edit.

export interface ModelSummary {
  schemaVersion: string;
  revision: string;
  compilerVersion: string;
  applianceCount: number;
  connectionCount: number;
  scenarioCount: number;
  cellCount: number;
  blueprintInstanceCount: number;
}

export type ModelDocumentKind =
  | "blueprint"
  | "instance"
  | "appliance"
  | "connection"
  | "scenario";

export interface ModelDocument {
  kind: ModelDocumentKind;
  path: string;
  revision: string;
  source: string;
}

export interface ModelDocumentCatalog {
  modelRevision: string;
  documents: ModelDocument[];
}

export interface DraftCreated {
  id: number;
  baseRevision: string;
}

export interface DraftChange {
  path: string;
  source: string | null;
}

export interface DraftDiagnostic {
  severity: string;
  path: string | null;
  line: number | null;
  message: string;
}

export interface CapacityAssessment {
  resource: string;
  scope: string;
  demand: number;
  reviewedLimit: number;
  structuralLimit: number | null;
  reservePercent: number;
  overflow: string;
  status: "accepted" | "review-required" | "rejected";
}

export interface CapacityDelta {
  resource: string;
  scope: string;
  previousDemand: number | null;
  candidateDemand: number | null;
  candidateStatus: "accepted" | "review-required" | "rejected" | null;
}

export interface SourceDigest {
  path: string;
  kind: string;
  sha256: string;
}

export interface DraftPreview {
  draft: number;
  baseRevision: string;
  candidateRevision: string | null;
  diagnostics: DraftDiagnostic[];
  addedObjects: string[];
  removedObjects: string[];
  modifiedObjects: string[];
  capacity: { assessments: CapacityAssessment[] } | null;
  capacityDeltas: CapacityDelta[];
  affectedScenarios: string[];
  previewCatalogs: SourceDigest[];
}

export interface DraftCommitResponse {
  draft: number;
  previousRevision: string;
  revision: string;
  changedPaths: string[];
  staleSessionCount: number;
}

export interface SimulationSessionResponse {
  id: number;
  modelRevision: string;
  status: "current" | "stale" | "stopped";
  cellCount: number;
  conduitCount: number;
}

export interface DraftCommitRequest {
  expectedRevision: string;
  reason: string;
}

export class ModelApiError extends Error {
  constructor(
    public readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = "ModelApiError";
  }
}

export class ModelApiClient {
  constructor(private readonly baseUrl = "") {}

  health() {
    return this.request<{ status: string; version: string; writeAccess: boolean; modelRevision: string }>("/api/health");
  }

  model() {
    return this.request<ModelSummary>("/api/model");
  }

  documents() {
    return this.request<ModelDocumentCatalog>("/api/model/documents");
  }

  createDraft() {
    return this.request<DraftCreated>("/api/model/drafts", { method: "POST" });
  }

  updateDraft(id: number, change: DraftChange) {
    return this.request<{ draft: number; accepted: boolean }>(this.draftPath(id, "/documents"), {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(change),
    });
  }

  previewDraft(id: number) {
    return this.request<DraftPreview>(this.draftPath(id, "/preview"), { method: "POST" });
  }

  commitDraft(id: number, request: DraftCommitRequest) {
    return this.request<DraftCommitResponse>(this.draftPath(id, "/commit"), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });
  }

  discardDraft(id: number) {
    return this.request<void>(this.draftPath(id), { method: "DELETE" });
  }

  createSession() {
    return this.request<SimulationSessionResponse>("/api/model/sessions", { method: "POST" });
  }

  session(id: number) {
    return this.request<SimulationSessionResponse>(this.sessionPath(id));
  }

  restartSession(id: number) {
    return this.request<SimulationSessionResponse>(`${this.sessionPath(id)}/restart`, {
      method: "POST",
    });
  }

  private draftPath(id: number, suffix = "") {
    return `/api/model/drafts/${encodeURIComponent(String(id))}${suffix}`;
  }

  private sessionPath(id: number) {
    return `/api/model/sessions/${encodeURIComponent(String(id))}`;
  }

  private async request<T>(path: string, init?: RequestInit): Promise<T> {
    const response = await fetch(`${this.baseUrl}${path}`, {
      ...init,
      headers: { Accept: "application/json", ...init?.headers },
    });
    if (!response.ok) {
      let message = `Model operation failed (${response.status})`;
      try {
        const body = (await response.json()) as { error?: string };
        if (body.error) message = body.error;
      } catch {
        // Retain the status-based message when the response has no JSON body.
      }
      throw new ModelApiError(response.status, message);
    }
    if (response.status === 204) return undefined as T;
    return (await response.json()) as T;
  }
}
"#
}
