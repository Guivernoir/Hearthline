use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};

use hearthline_model::{ComponentId, PartitionMessage, Text};

use crate::{
    CELL_SNAPSHOT_SCHEMA_VERSION, CellRegistration, CellSnapshot, ComponentSnapshot, ConduitConfig,
    ConduitOverflow, ProjectSnapshot, QUANTIZATION_CONTRACT_VERSION, REPLAY_SCHEMA_VERSION,
    ReplayArtifact, ReplayCheckpoint, ReplayError, ReplayOutcome, RunInput, RunManifest,
    SchedulerError, SchedulerMetrics, SnapshotValue, StableScheduler,
};

pub const CONDUIT_OVERLOAD_SCENARIO: &str = "foundation-conduit-overload";

pub fn run_conduit_overload_contract(
    model_digest: &str,
    simulation_version: &str,
) -> Result<ReplayArtifact, ContractScenarioError> {
    let mut metrics = BTreeMap::new();
    let mut initial_components = Vec::new();
    let mut final_components = Vec::new();
    for (name, policy) in [
        ("reject", ConduitOverflow::RejectNewest),
        ("drop", ConduitOverflow::DropOldest),
        ("coalesce", ConduitOverflow::CoalesceLatest),
        ("shutdown", ConduitOverflow::StopSimulation),
    ] {
        let (mut scheduler, site, source, destination) = scheduler(policy)?;
        for counter in 0..4 {
            scheduler.send(&site, &source, &site, &destination, heartbeat(counter)?)?;
        }
        let overflow = scheduler.send(&site, &source, &site, &destination, heartbeat(4)?);
        match policy {
            ConduitOverflow::RejectNewest => {
                require_error(&overflow, "backpressure", |error| {
                    matches!(error, SchedulerError::Backpressure { .. })
                })?;
            }
            ConduitOverflow::DropOldest | ConduitOverflow::CoalesceLatest => {
                if overflow.is_err() {
                    return Err(ContractScenarioError::Contract(format!(
                        "{name} policy rejected its reviewed overflow behavior"
                    )));
                }
            }
            ConduitOverflow::StopSimulation => {
                require_error(&overflow, "saturation shutdown", |error| {
                    matches!(error, SchedulerError::Saturated { .. })
                })?;
                while scheduler.pop_ready(0)?.is_some() {}
                scheduler.recover_from_saturation()?;
            }
        }
        let conduit = scheduler.conduits()[0].metrics();
        let scheduler_metrics = scheduler.metrics();
        metrics.insert(format!("{name}-rejected"), conduit.rejected);
        metrics.insert(format!("{name}-dropped"), conduit.dropped);
        metrics.insert(format!("{name}-coalesced"), conduit.coalesced);
        metrics.insert(format!("{name}-stopped"), conduit.stopped);
        metrics.insert(format!("{name}-recoveries"), scheduler_metrics.recoveries);
        initial_components.push(conduit_component(name, 0, 0, 0, 0, 0));
        final_components.push(conduit_component(
            name,
            conduit.rejected,
            conduit.dropped,
            conduit.coalesced,
            conduit.stopped,
            scheduler_metrics.recoveries,
        ));
    }
    let initial_snapshot = contract_snapshot(model_digest, 0, initial_components)?;
    let final_snapshot = contract_snapshot(model_digest, 4, final_components)?;
    let final_digest = final_snapshot.digest();
    ReplayArtifact {
        schema_version: REPLAY_SCHEMA_VERSION.into(),
        manifest: RunManifest {
            schema_version: REPLAY_SCHEMA_VERSION.into(),
            model_digest: model_digest.into(),
            simulation_version: simulation_version.into(),
            scenario: CONDUIT_OVERLOAD_SCENARIO.into(),
            quantization_contract: QUANTIZATION_CONTRACT_VERSION.into(),
            clock_policy: "fixed-step".into(),
            clock_step_us: 1,
            seed: 0,
            initial_state_digest: initial_snapshot.digest(),
            event_limit: 20,
            limits: BTreeMap::from([
                ("conduit-capacity".into(), 4),
                ("post-seal-allocations".into(), 0),
            ]),
            expected_outcomes: vec![
                "all-saturation-policies-observed".into(),
                "shutdown-policy-recovered".into(),
            ],
            inputs: ["reject", "drop", "coalesce", "shutdown"]
                .into_iter()
                .enumerate()
                .map(|(index, target)| RunInput::Fault {
                    at_us: index as u64,
                    target: format!("conduit/{target}"),
                    fault: "reviewed-burst-plus-one".into(),
                    active: true,
                })
                .collect(),
        },
        checkpoints: vec![
            ReplayCheckpoint::capture(0, &initial_snapshot),
            ReplayCheckpoint::capture(20, &final_snapshot),
        ],
        outcome: ReplayOutcome {
            status: "passed".into(),
            final_digest,
            event_count: 20,
            alarms: vec!["shutdown-policy-saturation-latched-and-recovered".into()],
            metrics,
        },
    }
    .normalize()
    .map_err(ContractScenarioError::Replay)
}

fn conduit_component(
    name: &str,
    rejected: u64,
    dropped: u64,
    coalesced: u64,
    stopped: u64,
    recoveries: u64,
) -> ComponentSnapshot {
    ComponentSnapshot {
        schema_version: CELL_SNAPSHOT_SCHEMA_VERSION.into(),
        component: format!("conduit-{name}"),
        component_kind: "bounded-conduit".into(),
        state: BTreeMap::from([
            ("coalesced".into(), SnapshotValue::Integer(coalesced as i64)),
            ("dropped".into(), SnapshotValue::Integer(dropped as i64)),
            (
                "recoveries".into(),
                SnapshotValue::Integer(recoveries as i64),
            ),
            ("rejected".into(), SnapshotValue::Integer(rejected as i64)),
            ("stopped".into(), SnapshotValue::Integer(stopped as i64)),
        ]),
    }
}

fn contract_snapshot(
    model_digest: &str,
    captured_at_us: u64,
    components: Vec<ComponentSnapshot>,
) -> Result<ProjectSnapshot, ContractScenarioError> {
    let site = Text::try_new("foundation-contract")
        .map_err(|error| ContractScenarioError::Contract(error.to_string()))?;
    let cell = Text::try_new("conduit-overload")
        .map_err(|error| ContractScenarioError::Contract(error.to_string()))?;
    ProjectSnapshot {
        schema_version: CELL_SNAPSHOT_SCHEMA_VERSION.into(),
        model_digest: model_digest.into(),
        captured_at_us,
        cells: vec![CellSnapshot {
            schema_version: CELL_SNAPSHOT_SCHEMA_VERSION.into(),
            site,
            cell,
            captured_at_us,
            components,
        }],
        conduits: Vec::new(),
        scheduler: SchedulerMetrics::default(),
    }
    .normalize()
    .map_err(ContractScenarioError::Contract)
}

fn scheduler(
    overflow: ConduitOverflow,
) -> Result<(StableScheduler, Text<64>, Text<64>, Text<64>), SchedulerError> {
    let site = Text::try_new("contract-site").expect("static site ID");
    let source = Text::try_new("source-cell").expect("static cell ID");
    let destination = Text::try_new("destination-cell").expect("static cell ID");
    let mut scheduler = StableScheduler::new();
    for cell in [&source, &destination] {
        scheduler.register_cell(CellRegistration {
            site: site.clone(),
            cell: cell.clone(),
            component_demand: 1,
            link_demand: 1,
            operational: true,
        })?;
    }
    scheduler.add_conduit(ConduitConfig {
        id: "contract-conduit".into(),
        source_site: site.clone(),
        source_cell: source.clone(),
        destination_site: site.clone(),
        destination_cell: destination.clone(),
        latency_us: 0,
        queue_capacity: 4,
        reviewed_burst: 3,
        overflow,
    })?;
    scheduler.seal()?;
    Ok((scheduler, site, source, destination))
}

fn heartbeat(counter: u64) -> Result<PartitionMessage, ContractScenarioError> {
    Ok(PartitionMessage::Heartbeat {
        source: ComponentId::new("contract-source")
            .map_err(|error| ContractScenarioError::Contract(error.to_string()))?,
        counter,
    })
}

fn require_error<T>(
    result: &Result<T, SchedulerError>,
    label: &str,
    predicate: impl FnOnce(&SchedulerError) -> bool,
) -> Result<(), ContractScenarioError> {
    if result.as_ref().is_err_and(predicate) {
        Ok(())
    } else {
        Err(ContractScenarioError::Contract(format!(
            "expected {label} from conduit saturation"
        )))
    }
}

#[derive(Debug)]
pub enum ContractScenarioError {
    Scheduler(SchedulerError),
    Replay(ReplayError),
    Contract(String),
}

impl Display for ContractScenarioError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scheduler(error) => Display::fmt(error, formatter),
            Self::Replay(error) => Display::fmt(error, formatter),
            Self::Contract(detail) => write!(formatter, "contract scenario failed: {detail}"),
        }
    }
}

impl std::error::Error for ContractScenarioError {}

impl From<SchedulerError> for ContractScenarioError {
    fn from(error: SchedulerError) -> Self {
        Self::Scheduler(error)
    }
}
