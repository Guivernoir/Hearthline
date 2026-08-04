use axum::Json;
use axum::extract::{Path as RoutePath, State};
use axum::http::StatusCode;
use hearthline_config::{
    SCENARIO_SCHEMA_VERSION, ScenarioConnectionOverride, ScenarioFirewallHaOverride,
    ScenarioFirstHopOverride, ScenarioPacketConfig, ScenarioReport, ScenarioSummary,
    run_scenario_with_state_overrides,
};
use serde::{Deserialize, Serialize};

use crate::{ApiError, AppState};

#[derive(Serialize)]
pub(super) struct SimulationCatalog {
    pub schema_version: &'static str,
    pub scenarios: Vec<ScenarioSummary>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RunSimulationRequest {
    #[serde(default)]
    packet: Option<ScenarioPacketConfig>,
    #[serde(default)]
    connection_overrides: Option<Vec<ScenarioConnectionOverride>>,
    #[serde(default)]
    first_hop_overrides: Option<Vec<ScenarioFirstHopOverride>>,
    #[serde(default)]
    firewall_ha_overrides: Option<Vec<ScenarioFirewallHaOverride>>,
}

pub(super) async fn catalog(
    State(state): State<AppState>,
) -> Result<Json<SimulationCatalog>, ApiError> {
    let (appliances, connections) = state.paths.load()?;
    let scenarios = state.paths.load_scenarios(&appliances, &connections)?;
    Ok(Json(SimulationCatalog {
        schema_version: SCENARIO_SCHEMA_VERSION,
        scenarios: scenarios.summaries(),
    }))
}

pub(super) async fn run(
    RoutePath(id): RoutePath<String>,
    State(state): State<AppState>,
    Json(request): Json<RunSimulationRequest>,
) -> Result<Json<ScenarioReport>, ApiError> {
    if let Some(packet) = &request.packet {
        packet.validate().map_err(ApiError::validation)?;
    }
    let (appliances, connections) = state.paths.load()?;
    let scenarios = state.paths.load_scenarios(&appliances, &connections)?;
    let scenario = scenarios.get(&id).ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            format!("unknown simulation scenario {id}"),
        )
    })?;
    let report = run_scenario_with_state_overrides(
        &appliances,
        &connections,
        &scenario.config,
        request.packet,
        request.connection_overrides,
        request.first_hop_overrides,
        request.firewall_ha_overrides,
    )
    .map_err(ApiError::validation)?;
    crate::security::record_scenario_reports(&state, std::slice::from_ref(&report)).await;
    Ok(Json(report))
}
