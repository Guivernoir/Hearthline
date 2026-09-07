use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::sync::Arc;

use crate::contracts::{
    DraftCommitRequest, DraftCommitResponse, DraftCreated, DraftUpdateResponse, ModelDocument,
    ModelDocumentCatalog, ModelSummary, SimulationSessionResponse,
};
use axum::Json;
use axum::extract::{Path as RoutePath, State};
use axum::http::{HeaderMap, StatusCode};
use hearthline_project::{DraftChange, DraftId, DraftPreview, ProjectCompiler};
use hearthline_sim::{SessionCapacityPolicy, SimulationSession, SimulationSessionStatus};

use crate::{ApiError, AppState};

#[derive(Default)]
pub(super) struct PinnedSimulationSessions {
    next_id: u64,
    sessions: BTreeMap<u64, SimulationSession>,
}

impl PinnedSimulationSessions {
    fn create(
        &mut self,
        project: Arc<hearthline_project::CompiledProject>,
    ) -> Result<u64, ApiError> {
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).ok_or_else(|| {
            ApiError::new(
                StatusCode::INSUFFICIENT_STORAGE,
                "simulation session ID space exhausted",
            )
        })?;
        let session = SimulationSession::build(project, SessionCapacityPolicy::reviewed_default())
            .map_err(ApiError::validation)?;
        self.sessions.insert(id, session);
        Ok(id)
    }

    fn response(
        &self,
        id: u64,
        current_revision: &str,
    ) -> Result<SimulationSessionResponse, ApiError> {
        let session = self.sessions.get(&id).ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                format!("unknown simulation session {id}"),
            )
        })?;
        Ok(session_response(id, session, current_revision))
    }

    fn stale_count(&self, current_revision: &str) -> usize {
        self.sessions
            .values()
            .filter(|session| session.status(current_revision) == SimulationSessionStatus::Stale)
            .count()
    }
}

pub(super) async fn summary(State(state): State<AppState>) -> Json<ModelSummary> {
    let project = state.compiled_project.read().await;
    Json(ModelSummary {
        schema_version: project.schema_version().into(),
        revision: project.digest().into(),
        compiler_version: env!("CARGO_PKG_VERSION").into(),
        appliance_count: project.appliances().len(),
        connection_count: project.connections().len(),
        scenario_count: project.scenarios().len(),
        cell_count: project.runtime_plan().partitions.len() + project.expanded_blueprints().len(),
        blueprint_instance_count: project.expanded_blueprints().len(),
    })
}

pub(super) async fn documents(
    State(state): State<AppState>,
) -> Result<Json<ModelDocumentCatalog>, ApiError> {
    const EDITABLE_KINDS: [&str; 5] = [
        "appliance",
        "blueprint",
        "connection",
        "instance",
        "scenario",
    ];
    let editable = BTreeSet::from(EDITABLE_KINDS);
    let project = state.compiled_project.read().await;
    let root = project.root();
    let mut documents = project
        .model_lock()
        .sources
        .iter()
        .filter(|source| editable.contains(source.kind.as_str()))
        .map(|source| {
            let path = root.join(&source.path);
            let canonical = fs::canonicalize(&path).map_err(ApiError::project)?;
            if !canonical.starts_with(root) {
                return Err(ApiError::new(
                    StatusCode::FORBIDDEN,
                    format!("locked source {} escapes the model root", source.path),
                ));
            }
            let body = fs::read_to_string(&canonical).map_err(ApiError::project)?;
            Ok(ModelDocument {
                kind: source.kind.clone(),
                path: format!("project/config/{}", source.path),
                revision: source.sha256.clone(),
                source: body,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    documents.sort_by(|left, right| {
        (left.kind.as_str(), left.path.as_str()).cmp(&(right.kind.as_str(), right.path.as_str()))
    });
    Ok(Json(ModelDocumentCatalog {
        model_revision: project.digest().into(),
        documents,
    }))
}

pub(super) async fn create_draft(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DraftCreated>, ApiError> {
    state.write_access.require(&headers)?;
    let draft = state
        .model_transactions
        .lock()
        .await
        .create()
        .map_err(ApiError::transaction)?;
    Ok(Json(DraftCreated {
        id: draft.id,
        base_revision: draft.base_revision,
    }))
}

pub(super) async fn update_draft(
    RoutePath(id): RoutePath<u64>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(change): Json<DraftChange>,
) -> Result<Json<DraftUpdateResponse>, ApiError> {
    state.write_access.require(&headers)?;
    state
        .model_transactions
        .lock()
        .await
        .update(DraftId(id), change)
        .map_err(ApiError::transaction)?;
    Ok(Json(DraftUpdateResponse {
        draft: DraftId(id),
        accepted: true,
    }))
}

pub(super) async fn preview_draft(
    RoutePath(id): RoutePath<u64>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DraftPreview>, ApiError> {
    state.write_access.require(&headers)?;
    state
        .model_transactions
        .lock()
        .await
        .preview(DraftId(id))
        .map(Json)
        .map_err(ApiError::transaction)
}

pub(super) async fn commit_draft(
    RoutePath(id): RoutePath<u64>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DraftCommitRequest>,
) -> Result<Json<DraftCommitResponse>, ApiError> {
    state.write_access.require(&headers)?;
    let commit = state
        .model_transactions
        .lock()
        .await
        .commit(DraftId(id), &request.expected_revision, &request.reason)
        .map_err(ApiError::transaction)?;
    let compiled = Arc::new(
        ProjectCompiler::new(&state.paths.config_root)
            .compile_locked()
            .map_err(ApiError::project)?,
    );
    let revision = compiled.digest().to_owned();
    *state.runtime_catalog.write().await = crate::RuntimeCatalog::from_project(&compiled);
    *state.compiled_project.write().await = compiled;
    let stale_session_count = state
        .simulation_sessions
        .lock()
        .await
        .stale_count(&revision);
    Ok(Json(DraftCommitResponse {
        draft: commit.draft,
        previous_revision: commit.previous_revision,
        revision: commit.revision,
        changed_paths: commit
            .changed_paths
            .into_iter()
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .collect(),
        stale_session_count,
    }))
}

pub(super) async fn discard_draft(
    RoutePath(id): RoutePath<u64>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    state.write_access.require(&headers)?;
    if state.model_transactions.lock().await.discard(DraftId(id)) {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::new(
            StatusCode::NOT_FOUND,
            format!("unknown model draft {id}"),
        ))
    }
}

pub(super) async fn create_session(
    State(state): State<AppState>,
) -> Result<Json<SimulationSessionResponse>, ApiError> {
    let project = state.compiled_project.read().await.clone();
    let revision = project.digest().to_owned();
    let mut sessions = state.simulation_sessions.lock().await;
    let id = sessions.create(project)?;
    sessions.response(id, &revision).map(Json)
}

pub(super) async fn session(
    RoutePath(id): RoutePath<u64>,
    State(state): State<AppState>,
) -> Result<Json<SimulationSessionResponse>, ApiError> {
    let revision = state.compiled_project.read().await.digest().to_owned();
    state
        .simulation_sessions
        .lock()
        .await
        .response(id, &revision)
        .map(Json)
}

pub(super) async fn restart_session(
    RoutePath(id): RoutePath<u64>,
    State(state): State<AppState>,
) -> Result<Json<SimulationSessionResponse>, ApiError> {
    let project = state.compiled_project.read().await.clone();
    let revision = project.digest().to_owned();
    let mut sessions = state.simulation_sessions.lock().await;
    if sessions.sessions.remove(&id).is_none() {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            format!("unknown simulation session {id}"),
        ));
    }
    let session = SimulationSession::build(project, SessionCapacityPolicy::reviewed_default())
        .map_err(ApiError::validation)?;
    sessions.sessions.insert(id, session);
    sessions.response(id, &revision).map(Json)
}

fn session_response(
    id: u64,
    session: &SimulationSession,
    current_revision: &str,
) -> SimulationSessionResponse {
    let status = match session.status(current_revision) {
        SimulationSessionStatus::Current => "current",
        SimulationSessionStatus::Stale => "stale",
        SimulationSessionStatus::Stopped => "stopped",
    };
    SimulationSessionResponse {
        id,
        model_revision: session.revision().digest.clone(),
        status: status.into(),
        cell_count: session.scheduler().cells().len(),
        conduit_count: session.scheduler().conduits().len(),
    }
}
