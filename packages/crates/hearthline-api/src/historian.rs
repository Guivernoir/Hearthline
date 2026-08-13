use axum::Json;
use axum::extract::{Path as RoutePath, State};
use axum::http::StatusCode;
use hearthline_config::{
    ConfigRepository, ConnectionRepository, HmiSnapshot, ScenarioApplicationConfig,
    ScenarioPacketConfig, ScenarioReport, ScenarioRepository, build_forming_telemetry_packet,
    run_scenario_with_state_overrides,
};
use hearthline_engine::HistorianBuffer;
use serde::Serialize;

use crate::{ApiError, AppState};

pub(super) const FORMING_SCADA_ID: &str = "area-02-machine-pc-01";
const COLLECTION_SCENARIO: &str = "factory-forming-historian-collection";
const REPLICATION_SCENARIO: &str = "factory-historian-dmz-replication";
const PUBLICATION_SCENARIO: &str = "factory-operations-data";
const HISTORIAN_SCHEMA_VERSION: &str = "0.1.0";
const SAMPLE_INTERVAL_MS: u64 = 1_000;
const RETRY_INTERVAL_MS: u64 = 250;
const RECORD_CAPACITY: usize = 60;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HistorianRecord {
    source: String,
    sequence: u64,
    captured_at_ms: u64,
    phase: String,
    cycle: u64,
    payload: String,
    wire_length_bytes: u16,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HistorianTierStatus {
    appliance_id: &'static str,
    stored_records: usize,
    capacity: usize,
    latest: Option<HistorianRecord>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HistorianStatus {
    schema_version: &'static str,
    sample_interval_ms: u64,
    local: HistorianTierStatus,
    replica: HistorianTierStatus,
    pending_records: usize,
    dropped_unreplicated: u64,
    replication_attempts: u64,
    last_error: Option<String>,
    last_collection: Option<ScenarioReport>,
    last_replication: Option<ScenarioReport>,
    last_publication: Option<ScenarioReport>,
}

#[derive(Clone, Debug)]
struct StoredRecord {
    view: HistorianRecord,
    packet: ScenarioPacketConfig,
}

#[derive(Debug, Default)]
pub(super) struct HistorianStore {
    local: HistorianBuffer<StoredRecord, RECORD_CAPACITY>,
    replica: HistorianBuffer<StoredRecord, RECORD_CAPACITY>,
    collection_elapsed_ms: u64,
    retry_elapsed_ms: u64,
    replication_attempts: u64,
    last_error: Option<String>,
    last_collection: Option<ScenarioReport>,
    last_replication: Option<ScenarioReport>,
    last_publication: Option<ScenarioReport>,
}

impl HistorianStore {
    pub(super) fn tick(
        &mut self,
        elapsed_ms: u64,
        snapshot: &HmiSnapshot,
        appliances: &ConfigRepository,
        connections: &ConnectionRepository,
        scenarios: &ScenarioRepository,
    ) {
        self.collection_elapsed_ms = self.collection_elapsed_ms.saturating_add(elapsed_ms);
        self.retry_elapsed_ms = self.retry_elapsed_ms.saturating_add(elapsed_ms);

        if self.collection_elapsed_ms >= SAMPLE_INTERVAL_MS {
            self.collection_elapsed_ms %= SAMPLE_INTERVAL_MS;
            if let Err(error) = self.collect(snapshot, appliances, connections, scenarios) {
                self.last_error = Some(error);
            }
        }
        if self.pending_records() > 0 && self.retry_elapsed_ms >= RETRY_INTERVAL_MS {
            self.retry_elapsed_ms %= RETRY_INTERVAL_MS;
            if let Err(error) = self.replicate(appliances, connections, scenarios) {
                self.last_error = Some(error);
            }
        }
    }

    pub(super) fn status(&self) -> HistorianStatus {
        HistorianStatus {
            schema_version: HISTORIAN_SCHEMA_VERSION,
            sample_interval_ms: SAMPLE_INTERVAL_MS,
            local: tier_status("ot-operations-services-01", &self.local),
            replica: tier_status("ot-dmz-hist-replica-01", &self.replica),
            pending_records: self.pending_records(),
            dropped_unreplicated: self.local.dropped_unreplicated(),
            replication_attempts: self.replication_attempts,
            last_error: self.last_error.clone(),
            last_collection: self.last_collection.clone(),
            last_replication: self.last_replication.clone(),
            last_publication: self.last_publication.clone(),
        }
    }

    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    fn collect(
        &mut self,
        snapshot: &HmiSnapshot,
        appliances: &ConfigRepository,
        connections: &ConnectionRepository,
        scenarios: &ScenarioRepository,
    ) -> Result<(), String> {
        let scenario = scenario(scenarios, COLLECTION_SCENARIO)?;
        let packet = build_forming_telemetry_packet(snapshot, scenario.packet.clone())
            .map_err(|error| error.to_string())?;
        let report = run_scenario_with_state_overrides(
            appliances,
            connections,
            scenario,
            Some(packet.clone()),
            None,
            None,
            None,
        )
        .map_err(|error| error.to_string())?;
        let delivered = report.expectation_met;
        self.last_collection = Some(report);
        if !delivered {
            return Err("factory-local historian collection did not reach its expectation".into());
        }

        let record = stored_record(snapshot, packet)?;
        if self
            .local
            .latest()
            .is_some_and(|latest| latest.view.sequence == record.view.sequence)
        {
            return Ok(());
        }
        self.local.push(record, false);
        Ok(())
    }

    fn replicate(
        &mut self,
        appliances: &ConfigRepository,
        connections: &ConnectionRepository,
        scenarios: &ScenarioRepository,
    ) -> Result<(), String> {
        let Some((position, source)) = self.local.oldest_pending() else {
            return Ok(());
        };
        let source = source.clone();
        let scenario = scenario(scenarios, REPLICATION_SCENARIO)?;
        let packet = retarget_telemetry_packet(&source.packet, scenario.packet.clone())?;
        self.replication_attempts = self.replication_attempts.saturating_add(1);
        let report = run_scenario_with_state_overrides(
            appliances,
            connections,
            scenario,
            Some(packet.clone()),
            None,
            None,
            None,
        )
        .map_err(|error| error.to_string())?;
        let delivered = report.expectation_met;
        self.last_replication = Some(report);
        if !delivered {
            return Err("historian replication did not reach the OT DMZ replica".into());
        }

        debug_assert!(self.local.mark_replicated(position));
        self.replica.push(
            StoredRecord {
                view: source.view,
                packet,
            },
            true,
        );
        self.last_error = None;
        Ok(())
    }

    fn pending_records(&self) -> usize {
        self.local.pending_count()
    }

    fn publication_packet(
        &self,
        template: ScenarioPacketConfig,
    ) -> Result<ScenarioPacketConfig, String> {
        let record = self.replica.latest().ok_or_else(|| {
            "the OT DMZ historian replica has no telemetry records yet".to_owned()
        })?;
        retarget_telemetry_packet(&record.packet, template)
    }
}

pub(super) async fn status(
    RoutePath(id): RoutePath<String>,
    State(state): State<AppState>,
) -> Result<Json<HistorianStatus>, ApiError> {
    authorize_scada(&state, &id).await?;
    Ok(Json(state.historian.lock().await.status()))
}

pub(super) async fn publish(
    RoutePath(id): RoutePath<String>,
    State(state): State<AppState>,
) -> Result<Json<ScenarioReport>, ApiError> {
    authorize_scada(&state, &id).await?;
    let (appliances, connections) = state.paths.load()?;
    let scenarios = state.paths.load_scenarios(&appliances, &connections)?;
    let scenario = scenarios.get(PUBLICATION_SCENARIO).ok_or_else(|| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "factory operations-data scenario is unavailable",
        )
    })?;
    let packet = state
        .historian
        .lock()
        .await
        .publication_packet(scenario.config.packet.clone())
        .map_err(|error| ApiError::new(StatusCode::SERVICE_UNAVAILABLE, error))?;
    let report = run_scenario_with_state_overrides(
        &appliances,
        &connections,
        &scenario.config,
        Some(packet),
        None,
        None,
        None,
    )
    .map_err(ApiError::validation)?;
    state
        .hmi_sessions
        .lock()
        .await
        .record_telemetry_publication(&appliances, &id, report.expectation_met)
        .map_err(ApiError::validation)?;
    state.historian.lock().await.last_publication = Some(report.clone());
    Ok(Json(report))
}

async fn authorize_scada(state: &AppState, id: &str) -> Result<(), ApiError> {
    let (appliances, _) = state.paths.load()?;
    super::hmi::require_appliance(&appliances, id)?;
    let snapshot = state
        .hmi_sessions
        .lock()
        .await
        .profile(&appliances, id)
        .map_err(ApiError::validation)?;
    if snapshot.interface_kind == "scada-workstation"
        && snapshot
            .permissions
            .iter()
            .any(|permission| permission == "publish-telemetry")
    {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::FORBIDDEN,
            format!("operator interface {id} cannot access operations telemetry"),
        ))
    }
}

fn scenario<'a>(
    scenarios: &'a ScenarioRepository,
    id: &str,
) -> Result<&'a hearthline_config::ScenarioConfig, String> {
    scenarios
        .get(id)
        .map(|scenario| &scenario.config)
        .ok_or_else(|| format!("required historian scenario {id} is unavailable"))
}

fn stored_record(
    snapshot: &HmiSnapshot,
    packet: ScenarioPacketConfig,
) -> Result<StoredRecord, String> {
    let process = snapshot
        .process
        .as_ref()
        .ok_or_else(|| "Forming SCADA has no process state".to_owned())?;
    let ScenarioApplicationConfig::Telemetry {
        source,
        sequence,
        payload,
        ..
    } = &packet.application
    else {
        return Err("historian collection packet is not telemetry".into());
    };
    Ok(StoredRecord {
        view: HistorianRecord {
            source: source.clone(),
            sequence: *sequence,
            captured_at_ms: snapshot
                .signals
                .iter()
                .map(|signal| signal.timestamp_ms)
                .max()
                .unwrap_or_default(),
            phase: process.phase.into(),
            cycle: process.cycle_count,
            payload: payload.clone(),
            wire_length_bytes: packet.wire_length_bytes,
        },
        packet,
    })
}

fn retarget_telemetry_packet(
    source: &ScenarioPacketConfig,
    mut target: ScenarioPacketConfig,
) -> Result<ScenarioPacketConfig, String> {
    let ScenarioApplicationConfig::Telemetry {
        source: component,
        sequence,
        payload,
        ..
    } = &source.application
    else {
        return Err("source historian record is not telemetry".into());
    };
    let service = match &target.application {
        ScenarioApplicationConfig::Telemetry { service, .. }
        | ScenarioApplicationConfig::Service { service } => service.clone(),
        _ => return Err("historian target scenario does not declare a service".into()),
    };
    target.wire_length_bytes = source.wire_length_bytes;
    target.application = ScenarioApplicationConfig::Telemetry {
        service,
        source: component.clone(),
        sequence: *sequence,
        payload: payload.clone(),
    };
    target.validate().map_err(|error| error.to_string())?;
    Ok(target)
}

fn tier_status(
    appliance_id: &'static str,
    records: &HistorianBuffer<StoredRecord, RECORD_CAPACITY>,
) -> HistorianTierStatus {
    HistorianTierStatus {
        appliance_id,
        stored_records: records.len(),
        capacity: records.capacity(),
        latest: records.latest().map(|record| record.view.clone()),
    }
}
