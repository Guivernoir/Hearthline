use std::collections::VecDeque;

use axum::Json;
use axum::extract::{Path as RoutePath, State};
use axum::http::StatusCode;
use hearthline_config::{ScenarioReport, ScenarioSecurityEvent};
use serde::Serialize;

use crate::{ApiError, AppState};

const SECURITY_CONSOLE_SCHEMA_VERSION: &str = "0.1.0";
const EVENT_CAPACITY: usize = 256;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SecurityEventRecord {
    id: u64,
    received_sequence: u64,
    acknowledged: bool,
    event: ScenarioSecurityEvent,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SecurityConsoleSession {
    schema_version: &'static str,
    console_id: String,
    sequence: u64,
    active_count: usize,
    acknowledged_count: usize,
    events: Vec<SecurityEventRecord>,
}

#[derive(Default)]
pub(super) struct SecurityEventStore {
    next_sequence: u64,
    events: VecDeque<SecurityEventRecord>,
}

impl SecurityEventStore {
    fn record(&mut self, event: ScenarioSecurityEvent) {
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.events.push_front(SecurityEventRecord {
            id: self.next_sequence,
            received_sequence: self.next_sequence,
            acknowledged: false,
            event,
        });
        if self.events.len() > EVENT_CAPACITY {
            self.events.pop_back();
        }
    }

    fn acknowledge(&mut self, id: u64) -> Option<SecurityEventRecord> {
        let record = self.events.iter_mut().find(|record| record.id == id)?;
        record.acknowledged = true;
        Some(record.clone())
    }

    fn clear_console(&mut self, console_id: &str) {
        self.events
            .retain(|record| record.event.defender != console_id);
    }

    pub(super) fn clear(&mut self) {
        self.events.clear();
        self.next_sequence = 0;
    }

    fn session(&self, console_id: &str) -> SecurityConsoleSession {
        let events = self
            .events
            .iter()
            .filter(|record| record.event.defender == console_id)
            .cloned()
            .collect::<Vec<_>>();
        SecurityConsoleSession {
            schema_version: SECURITY_CONSOLE_SCHEMA_VERSION,
            console_id: console_id.into(),
            sequence: self.next_sequence,
            active_count: events.iter().filter(|record| !record.acknowledged).count(),
            acknowledged_count: events.iter().filter(|record| record.acknowledged).count(),
            events,
        }
    }
}

pub(super) async fn console(
    RoutePath(id): RoutePath<String>,
    State(state): State<AppState>,
) -> Result<Json<SecurityConsoleSession>, ApiError> {
    require_console(&state, &id)?;
    let session = state.security_events.lock().await.session(&id);
    Ok(Json(session))
}

pub(super) async fn acknowledge(
    RoutePath(id): RoutePath<u64>,
    State(state): State<AppState>,
) -> Result<Json<SecurityEventRecord>, ApiError> {
    state
        .security_events
        .lock()
        .await
        .acknowledge(id)
        .map(Json)
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                format!("unknown security event {id}"),
            )
        })
}

pub(super) async fn clear(
    RoutePath(id): RoutePath<String>,
    State(state): State<AppState>,
) -> Result<Json<SecurityConsoleSession>, ApiError> {
    require_console(&state, &id)?;
    let mut store = state.security_events.lock().await;
    store.clear_console(&id);
    Ok(Json(store.session(&id)))
}

pub(super) async fn record_scenario_reports(state: &AppState, reports: &[ScenarioReport]) {
    let mut store = state.security_events.lock().await;
    for report in reports {
        if let Some(event) = &report.security {
            store.record(event.clone());
        }
    }
}

fn require_console(state: &AppState, id: &str) -> Result<(), ApiError> {
    let (appliances, _) = state.paths.load()?;
    let appliance = appliances.get(id).ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            format!("unknown security console {id}"),
        )
    })?;
    if appliance.config.kind.to_string() != "operations-console"
        || !appliance.config.tags.iter().any(|tag| tag == "interactive")
    {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("appliance {id} is not an interactive security console"),
        ));
    }
    Ok(())
}
