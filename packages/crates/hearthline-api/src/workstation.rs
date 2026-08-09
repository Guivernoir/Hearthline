use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{Path as RoutePath, State};
use axum::http::StatusCode;
use hearthline_config::{
    WorkstationAction, WorkstationActionReport, WorkstationProfile, WorkstationSession,
    run_workstation_action_with_session, workstation_profile,
};

use crate::{ApiError, AppState};

#[derive(Default)]
pub(super) struct WorkstationSessionStore {
    sessions: BTreeMap<String, WorkstationSession>,
}

impl WorkstationSessionStore {
    pub(super) fn tick(&mut self, elapsed_ms: u64) {
        for session in self.sessions.values_mut() {
            session.tick(elapsed_ms);
        }
    }

    pub(super) fn clear(&mut self) {
        self.sessions.clear();
    }
}

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
    let report = {
        let mut sessions = state.workstation_sessions.lock().await;
        let session = sessions.sessions.entry(id.clone()).or_default();
        run_workstation_action_with_session(
            &appliances,
            &connections,
            &scenarios,
            &id,
            action,
            session,
        )
        .map_err(ApiError::validation)?
    };
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
