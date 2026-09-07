//! Loopback API adapter, host contracts, and testable application router.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use contracts::HealthResponse;
use hearthline_config::{
    ConfigRepository, ConnectionRepository, FrontendApplianceCatalog, ScenarioRepository,
};
use hearthline_project::{CompiledProject, ModelTransactionStore, ProjectCompiler};
use hearthline_sim::PlantRuntimeStore;
use tokio::sync::{Mutex, RwLock};

pub mod contracts;

const DEFAULT_PORT: u16 = 3001;

#[derive(Clone)]
struct AppState {
    paths: Arc<ProjectPaths>,
    write_access: Arc<WriteAccessPolicy>,
    model_transactions: Arc<Mutex<ModelTransactionStore>>,
    compiled_project: Arc<RwLock<Arc<CompiledProject>>>,
    simulation_sessions: Arc<Mutex<model::PinnedSimulationSessions>>,
    workstation_sessions: Arc<Mutex<workstation::WorkstationSessionStore>>,
    plant_runtime: Arc<Mutex<PlantRuntimeStore>>,
    historian: Arc<Mutex<historian::HistorianStore>>,
    runtime_catalog: Arc<RwLock<RuntimeCatalog>>,
    security_events: Arc<Mutex<security::SecurityEventStore>>,
}

struct RuntimeCatalog {
    appliances: ConfigRepository,
    connections: ConnectionRepository,
    scenarios: ScenarioRepository,
}

impl RuntimeCatalog {
    fn from_project(project: &CompiledProject) -> Self {
        Self {
            appliances: project.appliances().clone(),
            connections: project.connections().clone(),
            scenarios: project.scenarios().clone(),
        }
    }
}

struct ProjectPaths {
    repository_root: PathBuf,
    config_root: PathBuf,
    appliance_root: PathBuf,
    connection_root: PathBuf,
    scenario_root: PathBuf,
}

impl ProjectPaths {
    fn from_project_root(root: &Path) -> Self {
        Self {
            repository_root: root.to_path_buf(),
            config_root: root.join("project/config"),
            appliance_root: root.join("project/config/appliances"),
            connection_root: root.join("project/config/connections"),
            scenario_root: root.join("project/config/scenarios"),
        }
    }

    fn load(&self) -> Result<(ConfigRepository, ConnectionRepository), ApiError> {
        let appliances = ConfigRepository::load(&self.appliance_root).map_err(ApiError::project)?;
        let connections = ConnectionRepository::load(&self.connection_root, &appliances)
            .map_err(ApiError::project)?;
        Ok((appliances, connections))
    }

    fn load_scenarios(
        &self,
        appliances: &ConfigRepository,
        connections: &ConnectionRepository,
    ) -> Result<ScenarioRepository, ApiError> {
        ScenarioRepository::load(&self.scenario_root, appliances, connections)
            .map_err(ApiError::project)
    }
}

struct WriteAccessPolicy {
    bearer_token: Option<String>,
}

impl WriteAccessPolicy {
    fn require(&self, headers: &HeaderMap) -> Result<(), ApiError> {
        let Some(expected) = &self.bearer_token else {
            return Ok(());
        };
        let supplied = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "));
        if supplied == Some(expected.as_str()) {
            Ok(())
        } else {
            Err(ApiError::new(
                StatusCode::UNAUTHORIZED,
                "model writes require the configured bearer token",
            ))
        }
    }
}

#[derive(serde::Serialize)]
struct ErrorResponse {
    error: String,
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn project(error: impl std::fmt::Display) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("project configuration is invalid: {error}"),
        )
    }

    fn validation(error: impl std::fmt::Display) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, error.to_string())
    }

    fn transaction(error: impl std::fmt::Display) -> Self {
        let detail = error.to_string();
        let status = if detail.contains("stale model revision") {
            StatusCode::CONFLICT
        } else if detail.contains("unknown model draft") {
            StatusCode::NOT_FOUND
        } else if detail.contains("validation failed") {
            StatusCode::UNPROCESSABLE_ENTITY
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        Self::new(status, detail)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse {
                error: self.message,
            }),
        )
            .into_response()
    }
}

/// Builds the complete HTTP adapter without binding a socket.
pub fn router_for_repository(
    project_root: impl AsRef<Path>,
    bearer_token: Option<String>,
) -> Result<Router, Box<dyn std::error::Error>> {
    let state = state_for_repository(project_root.as_ref(), bearer_token)?;
    Ok(routes(state))
}

/// Builds the HTTP adapter with the deterministic plant clock used by the server.
///
/// This constructor is intended for end-to-end adapter tests and embedded loopback hosts.
pub fn router_for_repository_with_process_clock(
    project_root: impl AsRef<Path>,
    bearer_token: Option<String>,
) -> Result<Router, Box<dyn std::error::Error>> {
    let state = state_for_repository(project_root.as_ref(), bearer_token)?;
    start_process_clock(state.clone());
    Ok(routes(state))
}

/// Runs the standalone API with loopback-only writes unless authentication is configured.
pub async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()?;
    let bind_ip = std::env::var("HEARTHLINE_API_BIND")
        .unwrap_or_else(|_| Ipv4Addr::LOCALHOST.to_string())
        .parse::<IpAddr>()?;
    let bearer_token = std::env::var("HEARTHLINE_API_AUTH_TOKEN")
        .ok()
        .filter(|token| !token.trim().is_empty());
    require_safe_write_binding(bind_ip, bearer_token.as_deref())?;

    let state = state_for_repository(&project_root, bearer_token)?;
    start_process_clock(state.clone());
    let app = routes(state);
    let port = std::env::var("HEARTHLINE_API_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let address = SocketAddr::from((bind_ip, port));
    let listener = tokio::net::TcpListener::bind(address).await?;
    println!("Hearthline API listening on http://{address}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

pub fn require_safe_write_binding(
    bind_ip: IpAddr,
    bearer_token: Option<&str>,
) -> Result<(), &'static str> {
    if bind_ip.is_loopback() || bearer_token.is_some_and(|token| !token.trim().is_empty()) {
        Ok(())
    } else {
        Err("refusing non-loopback API binding without HEARTHLINE_API_AUTH_TOKEN")
    }
}

fn state_for_repository(
    project_root: &Path,
    bearer_token: Option<String>,
) -> Result<AppState, Box<dyn std::error::Error>> {
    let paths = ProjectPaths::from_project_root(project_root);
    let model_transactions = ModelTransactionStore::new(&paths.repository_root)?;
    let compiled = Arc::new(ProjectCompiler::new(&paths.config_root).compile_locked()?);
    let runtime_catalog = RuntimeCatalog::from_project(&compiled);
    Ok(AppState {
        paths: Arc::new(paths),
        write_access: Arc::new(WriteAccessPolicy { bearer_token }),
        model_transactions: Arc::new(Mutex::new(model_transactions)),
        compiled_project: Arc::new(RwLock::new(compiled)),
        simulation_sessions: Arc::new(Mutex::new(model::PinnedSimulationSessions::default())),
        workstation_sessions: Arc::new(Mutex::new(workstation::WorkstationSessionStore::default())),
        plant_runtime: Arc::new(Mutex::new(PlantRuntimeStore::default())),
        historian: Arc::new(Mutex::new(historian::HistorianStore::default())),
        runtime_catalog: Arc::new(RwLock::new(runtime_catalog)),
        security_events: Arc::new(Mutex::new(security::SecurityEventStore::default())),
    })
}

fn routes(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/model", get(model::summary))
        .route("/api/model/documents", get(model::documents))
        .route("/api/model/drafts", post(model::create_draft))
        .route("/api/model/drafts/{id}/documents", put(model::update_draft))
        .route("/api/model/drafts/{id}/preview", post(model::preview_draft))
        .route("/api/model/drafts/{id}/commit", post(model::commit_draft))
        .route("/api/model/drafts/{id}", delete(model::discard_draft))
        .route("/api/model/sessions", post(model::create_session))
        .route("/api/model/sessions/{id}", get(model::session))
        .route(
            "/api/model/sessions/{id}/restart",
            post(model::restart_session),
        )
        .route("/api/config/catalog", get(catalog))
        .route("/api/simulations", get(simulation::catalog))
        .route("/api/simulations/{id}/run", post(simulation::run))
        .route("/api/hmis/{id}", get(hmi::profile))
        .route("/api/hmis/{id}/program", get(hmi::control_program))
        .route("/api/hmis/{id}/actions", post(hmi::action))
        .route("/api/hmis/{id}/historian", get(historian::status))
        .route("/api/hmis/{id}/telemetry", post(historian::publish))
        .route("/api/workstations/{id}", get(workstation::profile))
        .route("/api/workstations/{id}/actions", post(workstation::action))
        .route("/api/security/consoles/{id}", get(security::console))
        .route(
            "/api/security/events/{id}/acknowledge",
            post(security::acknowledge),
        )
        .route("/api/security/consoles/{id}/clear", post(security::clear))
        .with_state(state)
}

fn start_process_clock(state: AppState) {
    tokio::spawn(async move {
        const TICK_MS: u64 = 250;
        let mut clock = tokio::time::interval(std::time::Duration::from_millis(TICK_MS));
        clock.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            clock.tick().await;
            state.workstation_sessions.lock().await.tick(TICK_MS);
            let catalog = state.runtime_catalog.read().await;
            let snapshot = {
                let mut sessions = state.plant_runtime.lock().await;
                sessions.tick(TICK_MS);
                sessions.profile(&catalog.appliances, historian::FORMING_SCADA_ID)
            };
            if let Ok(snapshot) = snapshot {
                state.historian.lock().await.tick(
                    TICK_MS,
                    &snapshot,
                    &catalog.appliances,
                    &catalog.connections,
                    &catalog.scenarios,
                );
            }
        }
    });
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        write_access: true,
        model_revision: state.compiled_project.read().await.digest().into(),
    })
}

async fn catalog(
    State(state): State<AppState>,
) -> Result<Json<FrontendApplianceCatalog>, ApiError> {
    let catalog = state.runtime_catalog.read().await;
    Ok(Json(
        catalog.appliances.frontend_catalog(&catalog.connections),
    ))
}

mod historian;
mod hmi;
mod model;
mod security;
mod simulation;
mod workstation;
