use axum::Json;
use axum::extract::{Path as RoutePath, State};
use axum::http::StatusCode;
use hearthline_config::{HmiAction, HmiActionReport, HmiSession, HmiSnapshot};

use crate::{ApiError, AppState};

pub(super) async fn profile(
    RoutePath(id): RoutePath<String>,
    State(state): State<AppState>,
) -> Result<Json<HmiSnapshot>, ApiError> {
    let (appliances, _) = state.paths.load()?;
    require_appliance(&appliances, &id)?;
    let mut sessions = state.hmi_sessions.lock().await;
    if !sessions.contains_key(&id) {
        let session =
            HmiSession::from_repository(&appliances, &id).map_err(ApiError::validation)?;
        sessions.insert(id.clone(), session);
    }
    Ok(Json(
        sessions
            .get(&id)
            .expect("HMI session was inserted")
            .snapshot(),
    ))
}

pub(super) async fn action(
    RoutePath(id): RoutePath<String>,
    State(state): State<AppState>,
    Json(action): Json<HmiAction>,
) -> Result<Json<HmiActionReport>, ApiError> {
    let (appliances, _) = state.paths.load()?;
    require_appliance(&appliances, &id)?;
    let mut sessions = state.hmi_sessions.lock().await;
    if !sessions.contains_key(&id) {
        let session =
            HmiSession::from_repository(&appliances, &id).map_err(ApiError::validation)?;
        sessions.insert(id.clone(), session);
    }
    Ok(Json(
        sessions
            .get_mut(&id)
            .expect("HMI session was inserted")
            .execute(action),
    ))
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
