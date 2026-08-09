use axum::Json;
use axum::extract::{Path as RoutePath, State};
use axum::http::StatusCode;
use hearthline_config::{HmiAction, HmiActionReport, HmiControlProgramDocument, HmiSnapshot};

use crate::{ApiError, AppState};

pub(super) async fn profile(
    RoutePath(id): RoutePath<String>,
    State(state): State<AppState>,
) -> Result<Json<HmiSnapshot>, ApiError> {
    let (appliances, _) = state.paths.load()?;
    require_appliance(&appliances, &id)?;
    let mut sessions = state.hmi_sessions.lock().await;
    Ok(Json(
        sessions
            .profile(&appliances, &id)
            .map_err(ApiError::validation)?,
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
    Ok(Json(
        sessions
            .execute(&appliances, &id, action)
            .map_err(ApiError::validation)?,
    ))
}

pub(super) async fn control_program(
    RoutePath(id): RoutePath<String>,
    State(state): State<AppState>,
) -> Result<Json<HmiControlProgramDocument>, ApiError> {
    let (appliances, _) = state.paths.load()?;
    require_appliance(&appliances, &id)?;
    let mut sessions = state.hmi_sessions.lock().await;
    let document = sessions
        .control_program(&appliances, &id)
        .map_err(ApiError::validation)?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                format!("HMI {id} has no executable control source"),
            )
        })?;
    Ok(Json(document))
}

pub(super) fn require_appliance(
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
