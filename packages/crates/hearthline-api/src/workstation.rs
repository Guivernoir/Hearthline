use axum::Json;
use axum::extract::{Path as RoutePath, State};
use axum::http::StatusCode;
use hearthline_config::{
    WorkstationAction, WorkstationActionReport, WorkstationProfile, run_workstation_action,
    workstation_profile,
};

use crate::{ApiError, AppState};

pub(super) async fn profile(
    RoutePath(id): RoutePath<String>,
    State(state): State<AppState>,
) -> Result<Json<WorkstationProfile>, ApiError> {
    let (appliances, connections) = state.paths.load()?;
    require_appliance(&appliances, &id)?;
    let scenarios = state.paths.load_scenarios(&appliances, &connections)?;
    workstation_profile(&appliances, &scenarios, &id)
        .map(Json)
        .map_err(ApiError::validation)
}

pub(super) async fn action(
    RoutePath(id): RoutePath<String>,
    State(state): State<AppState>,
    Json(action): Json<WorkstationAction>,
) -> Result<Json<WorkstationActionReport>, ApiError> {
    let (appliances, connections) = state.paths.load()?;
    require_appliance(&appliances, &id)?;
    let scenarios = state.paths.load_scenarios(&appliances, &connections)?;
    let report = run_workstation_action(&appliances, &connections, &scenarios, &id, action)
        .map_err(ApiError::validation)?;
    crate::security::record_scenario_reports(&state, &report.simulations).await;
    Ok(Json(report))
}

fn require_appliance(
    appliances: &hearthline_config::ConfigRepository,
    id: &str,
) -> Result<(), ApiError> {
    if appliances.get(id).is_some() {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::NOT_FOUND,
            format!("unknown appliance {id}"),
        ))
    }
}
