use std::ffi::OsString;
use std::fs;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Path as RoutePath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use hearthline_config::{
    ConfigRepository, ConnectionRepository, FrontendApplianceCatalog, HmiSessionStore,
    ScenarioRepository,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};

const DEFAULT_PORT: u16 = 3001;

#[derive(Clone)]
struct AppState {
    paths: Arc<ProjectPaths>,
    write_lock: Arc<Mutex<()>>,
    workstation_sessions: Arc<Mutex<workstation::WorkstationSessionStore>>,
    hmi_sessions: Arc<Mutex<HmiSessionStore>>,
    historian: Arc<Mutex<historian::HistorianStore>>,
    runtime_catalog: Arc<RwLock<RuntimeCatalog>>,
    security_events: Arc<Mutex<security::SecurityEventStore>>,
}

struct RuntimeCatalog {
    appliances: ConfigRepository,
    connections: ConnectionRepository,
    scenarios: ScenarioRepository,
}

struct ProjectPaths {
    appliance_root: PathBuf,
    connection_root: PathBuf,
    scenario_root: PathBuf,
    generated_catalog: PathBuf,
}

impl ProjectPaths {
    fn from_project_root(root: &Path) -> Self {
        Self {
            appliance_root: root.join("project/config/appliances"),
            connection_root: root.join("project/config/connections"),
            scenario_root: root.join("project/config/scenarios"),
            generated_catalog: root.join("packages/web/src/generated/appliance-configs.json"),
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
    write_access: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateRequest {
    source_yaml: String,
    expected_revision: String,
}

#[derive(Serialize)]
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

    fn io(action: &str, error: impl std::fmt::Display) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("{action}: {error}"),
        )
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()?;
    let paths = ProjectPaths::from_project_root(&project_root);
    let (appliances, connections) = paths.load().map_err(|error| error.message)?;
    let scenarios = paths
        .load_scenarios(&appliances, &connections)
        .map_err(|error| error.message)?;

    let state = AppState {
        paths: Arc::new(paths),
        write_lock: Arc::new(Mutex::new(())),
        workstation_sessions: Arc::new(Mutex::new(workstation::WorkstationSessionStore::default())),
        hmi_sessions: Arc::new(Mutex::new(HmiSessionStore::default())),
        historian: Arc::new(Mutex::new(historian::HistorianStore::default())),
        runtime_catalog: Arc::new(RwLock::new(RuntimeCatalog {
            appliances,
            connections,
            scenarios,
        })),
        security_events: Arc::new(Mutex::new(security::SecurityEventStore::default())),
    };
    start_process_clock(state.clone());
    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/config/catalog", get(catalog))
        .route("/api/config/appliances/{id}", put(update_appliance))
        .route("/api/config/connections/{id}", put(update_connection))
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
        .with_state(state);

    let port = std::env::var("HEARTHLINE_API_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let listener = tokio::net::TcpListener::bind(address).await?;
    println!("Hearthline local API listening on http://{address}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
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
                let mut sessions = state.hmi_sessions.lock().await;
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

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        write_access: true,
    })
}

async fn catalog(
    State(state): State<AppState>,
) -> Result<Json<FrontendApplianceCatalog>, ApiError> {
    let (appliances, connections) = state.paths.load()?;
    Ok(Json(appliances.frontend_catalog(&connections)))
}

async fn update_appliance(
    RoutePath(id): RoutePath<String>,
    State(state): State<AppState>,
    Json(request): Json<UpdateRequest>,
) -> Result<Json<FrontendApplianceCatalog>, ApiError> {
    let _guard = state.write_lock.lock().await;
    let (appliances, _connections) = state.paths.load()?;
    let current = appliances.get(&id).ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            format!("unknown appliance configuration {id}"),
        )
    })?;
    require_revision(&request.expected_revision, &current.revision())?;

    let candidate_appliances = ConfigRepository::load_with_override(
        &state.paths.appliance_root,
        Some((&current.source_file, &request.source_yaml)),
    )
    .map_err(ApiError::validation)?;
    let candidate_connections =
        ConnectionRepository::load(&state.paths.connection_root, &candidate_appliances)
            .map_err(ApiError::validation)?;
    let catalog = candidate_appliances.frontend_catalog(&candidate_connections);
    commit_configuration(
        &current.source_file,
        &request.source_yaml,
        &current.source_yaml,
        &state.paths.generated_catalog,
        &catalog,
    )?;
    state.hmi_sessions.lock().await.clear();
    state.workstation_sessions.lock().await.clear();
    refresh_runtime(&state).await?;
    state.historian.lock().await.clear();
    state.security_events.lock().await.clear();
    Ok(Json(catalog))
}

async fn update_connection(
    RoutePath(id): RoutePath<String>,
    State(state): State<AppState>,
    Json(request): Json<UpdateRequest>,
) -> Result<Json<FrontendApplianceCatalog>, ApiError> {
    let _guard = state.write_lock.lock().await;
    let (appliances, connections) = state.paths.load()?;
    let current = connections.get(&id).ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            format!("unknown connection configuration {id}"),
        )
    })?;
    require_revision(&request.expected_revision, &current.revision())?;

    let candidate_connections = ConnectionRepository::load_with_override(
        &state.paths.connection_root,
        &appliances,
        Some((&current.source_file, &request.source_yaml)),
    )
    .map_err(ApiError::validation)?;
    let catalog = appliances.frontend_catalog(&candidate_connections);
    commit_configuration(
        &current.source_file,
        &request.source_yaml,
        &current.source_yaml,
        &state.paths.generated_catalog,
        &catalog,
    )?;
    state.hmi_sessions.lock().await.clear();
    state.workstation_sessions.lock().await.clear();
    refresh_runtime(&state).await?;
    state.historian.lock().await.clear();
    state.security_events.lock().await.clear();
    Ok(Json(catalog))
}

async fn refresh_runtime(state: &AppState) -> Result<(), ApiError> {
    let (appliances, connections) = state.paths.load()?;
    let scenarios = state.paths.load_scenarios(&appliances, &connections)?;
    *state.runtime_catalog.write().await = RuntimeCatalog {
        appliances,
        connections,
        scenarios,
    };
    Ok(())
}

fn require_revision(expected: &str, current: &str) -> Result<(), ApiError> {
    if expected == current {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::CONFLICT,
            "configuration changed after it was opened; reload before saving",
        ))
    }
}

fn commit_configuration(
    source_path: &Path,
    source_yaml: &str,
    previous_source: &str,
    catalog_path: &Path,
    catalog: &FrontendApplianceCatalog,
) -> Result<(), ApiError> {
    let catalog_json = serde_json::to_string_pretty(catalog)
        .map_err(|error| ApiError::io("cannot serialize generated catalog", error))?
        + "\n";
    let source_temporary = temporary_path(source_path);
    let catalog_temporary = temporary_path(catalog_path);

    fs::write(&source_temporary, source_yaml)
        .map_err(|error| ApiError::io("cannot stage configuration", error))?;
    if let Err(error) = fs::write(&catalog_temporary, catalog_json) {
        let _ = fs::remove_file(&source_temporary);
        return Err(ApiError::io("cannot stage generated catalog", error));
    }
    if let Err(error) = fs::rename(&source_temporary, source_path) {
        let _ = fs::remove_file(&catalog_temporary);
        return Err(ApiError::io("cannot commit configuration", error));
    }
    if let Err(error) = fs::rename(&catalog_temporary, catalog_path) {
        let _ = fs::write(source_path, previous_source);
        return Err(ApiError::io(
            "cannot commit generated catalog; configuration was restored",
            error,
        ));
    }
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name: OsString = path.as_os_str().to_owned();
    name.push(".hearthline.tmp");
    PathBuf::from(name)
}
mod historian;
mod hmi;
mod security;
mod simulation;
mod workstation;
