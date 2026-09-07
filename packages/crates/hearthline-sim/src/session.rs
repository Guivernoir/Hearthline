use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

use hearthline_model::Text;
use hearthline_project::CompiledProject;
use serde::{Deserialize, Serialize};

use crate::{
    CELL_SNAPSHOT_SCHEMA_VERSION, CellId, CellRegistration, CellSnapshot, ComponentSnapshot,
    ConduitConfig, ConduitId, ConduitOverflow, ConfiguredNetwork, ProjectSnapshot, SiteId,
    SnapshotValue, StableScheduler,
};
use hearthline_model::{PartitionMessage, PartitionMessageClass};

pub const SESSION_BUILD_STACK_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRevision {
    pub digest: String,
    pub compiler_version: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationSessionStatus {
    Current,
    Stale,
    Stopped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionCapacityPolicy {
    pub conduit_capacity: usize,
    pub reviewed_burst: usize,
    pub latency_us: u64,
    pub overflow: ConduitOverflow,
}

impl SessionCapacityPolicy {
    pub const fn reviewed_default() -> Self {
        Self {
            conduit_capacity: 32,
            reviewed_burst: 24,
            latency_us: 0,
            overflow: ConduitOverflow::RejectNewest,
        }
    }
}

pub struct SimulationSession {
    project: Arc<CompiledProject>,
    revision: ModelRevision,
    scheduler: StableScheduler,
    cells: Vec<CellRuntime>,
    stopped: bool,
}

enum CellRuntime {
    Configured {
        site: SiteId,
        cell: CellId,
        network: ConfiguredNetwork,
    },
    Blueprint {
        site: SiteId,
        cell: CellId,
        components: Vec<BlueprintComponentRuntime>,
    },
}

struct BlueprintComponentRuntime {
    component: String,
    component_kind: String,
    environment: String,
    ports: String,
    site: String,
    ticks: u64,
    elapsed_us: u64,
    received_messages: u64,
    last_message_class: Option<PartitionMessageClass>,
}

impl SimulationSession {
    pub fn build(
        project: Arc<CompiledProject>,
        policy: SessionCapacityPolicy,
    ) -> Result<Self, SessionError> {
        std::thread::Builder::new()
            .name("hearthline-model-loader".into())
            .stack_size(SESSION_BUILD_STACK_BYTES)
            .spawn(move || Self::build_inner(project, policy))
            .map_err(|error| SessionError::Build(format!("cannot start model loader: {error}")))?
            .join()
            .map_err(|_| SessionError::Build("model loader panicked".into()))?
    }

    fn build_inner(
        project: Arc<CompiledProject>,
        policy: SessionCapacityPolicy,
    ) -> Result<Self, SessionError> {
        let mut scheduler = StableScheduler::new();
        let mut cells = Vec::new();
        let canonical_site = site_id("canonical-project")?;
        for partition in &project.runtime_plan().partitions {
            let cell = cell_id(&partition.id)?;
            scheduler.register_cell(CellRegistration {
                site: canonical_site.clone(),
                cell: cell.clone(),
                component_demand: partition.component_demand,
                link_demand: partition.internal_link_demand,
                operational: true,
            })?;
            let appliance_ids = project
                .appliances()
                .appliances()
                .filter(|loaded| {
                    project.runtime_plan().partition_for(&loaded.config.id)
                        == Some(partition.id.as_str())
                })
                .map(|loaded| loaded.config.id.as_str())
                .collect::<Vec<_>>();
            let network = ConfiguredNetwork::from_selection(
                project.appliances(),
                project.connections(),
                appliance_ids,
            )
            .map_err(|error| SessionError::Build(error.to_string()))?;
            cells.push(CellRuntime::Configured {
                site: canonical_site.clone(),
                cell,
                network,
            });
        }
        for blueprint in project.expanded_blueprints() {
            let instance = project
                .blueprints()
                .instances()
                .find(|instance| instance.id == blueprint.instance)
                .ok_or_else(|| {
                    SessionError::Build(format!(
                        "missing blueprint instance {}",
                        blueprint.instance
                    ))
                })?;
            let site = site_id(&instance.site)?;
            let cell = cell_id(&instance.id)?;
            scheduler.register_cell(CellRegistration {
                site: site.clone(),
                cell: cell.clone(),
                component_demand: blueprint.nodes.len(),
                link_demand: blueprint.connections.len(),
                operational: true,
            })?;
            let components = blueprint
                .nodes
                .iter()
                .map(|node| BlueprintComponentRuntime {
                    component: node.id.clone(),
                    component_kind: node.family.clone(),
                    environment: node.environment.clone(),
                    ports: node.ports.join(","),
                    site: node.site.clone(),
                    ticks: 0,
                    elapsed_us: 0,
                    received_messages: 0,
                    last_message_class: None,
                })
                .collect();
            cells.push(CellRuntime::Blueprint {
                site,
                cell,
                components,
            });
        }
        let mut pairs = BTreeSet::new();
        for boundary in &project.runtime_plan().boundaries {
            pairs.insert((boundary.partition_a.clone(), boundary.partition_b.clone()));
            pairs.insert((boundary.partition_b.clone(), boundary.partition_a.clone()));
        }
        for (source, destination) in pairs {
            let conduit_name = format!("{source}-to-{destination}");
            scheduler.add_conduit(ConduitConfig {
                id: ConduitId::try_new(&conduit_name).map_err(|_| {
                    SessionError::Build(format!("conduit ID exceeds 128 bytes: {conduit_name}"))
                })?,
                source_site: canonical_site.clone(),
                source_cell: cell_id(&source)?,
                destination_site: canonical_site.clone(),
                destination_cell: cell_id(&destination)?,
                latency_us: policy.latency_us,
                queue_capacity: policy.conduit_capacity,
                reviewed_burst: policy.reviewed_burst,
                overflow: policy.overflow,
            })?;
        }
        scheduler.seal()?;
        cells.sort_by(|left, right| left.key().cmp(&right.key()));
        let revision = ModelRevision {
            digest: project.digest().into(),
            compiler_version: env!("CARGO_PKG_VERSION").into(),
        };
        Ok(Self {
            project,
            revision,
            scheduler,
            cells,
            stopped: false,
        })
    }

    pub fn revision(&self) -> &ModelRevision {
        &self.revision
    }
    pub fn project(&self) -> &CompiledProject {
        &self.project
    }
    pub fn scheduler(&self) -> &StableScheduler {
        &self.scheduler
    }
    pub fn snapshot(&self) -> Result<ProjectSnapshot, SessionError> {
        let captured_at_us = self.scheduler.time_us();
        let cells = self
            .cells
            .iter()
            .map(|cell| cell.snapshot(captured_at_us))
            .collect();
        ProjectSnapshot::capture(self.project.digest(), &self.scheduler, cells)
            .map_err(SessionError::Build)
    }

    pub fn component_count(&self) -> usize {
        self.cells.iter().map(CellRuntime::component_count).sum()
    }

    pub fn link_count(&self) -> usize {
        self.project.connections().len()
            + self
                .project
                .expanded_blueprints()
                .iter()
                .map(|blueprint| blueprint.connections.len())
                .sum::<usize>()
    }

    pub fn advance_to(&mut self, now_us: u64) -> Result<(), SessionError> {
        let elapsed_us = now_us
            .checked_sub(self.scheduler.time_us())
            .ok_or_else(|| SessionError::Build("simulation clock cannot move backwards".into()))?;
        while let Some(envelope) = self.scheduler.pop_ready(now_us)? {
            let destination_operational = self.scheduler.cells().iter().any(|registration| {
                registration.site == envelope.destination_site
                    && registration.cell == envelope.destination_cell
                    && registration.operational
            });
            if destination_operational
                && let Ok(index) = self.cells.binary_search_by(|cell| {
                    cell.key()
                        .cmp(&(&envelope.destination_site, &envelope.destination_cell))
                })
            {
                self.cells[index].receive(&envelope.message);
            }
        }
        for (registration, cell) in self.scheduler.cells().iter().zip(&mut self.cells) {
            debug_assert_eq!(
                (&registration.site, &registration.cell),
                cell.key(),
                "scheduler and runtime cells retain the same normalized order"
            );
            if registration.operational {
                cell.tick(elapsed_us);
            }
        }
        Ok(())
    }
    pub fn scheduler_mut(&mut self) -> Result<&mut StableScheduler, SessionError> {
        if self.stopped {
            Err(SessionError::Stopped)
        } else {
            Ok(&mut self.scheduler)
        }
    }
    pub fn stop(&mut self) {
        self.stopped = true;
    }

    pub fn status(&self, current_revision: &str) -> SimulationSessionStatus {
        if self.stopped {
            SimulationSessionStatus::Stopped
        } else if self.revision.digest == current_revision {
            SimulationSessionStatus::Current
        } else {
            SimulationSessionStatus::Stale
        }
    }
}

impl CellRuntime {
    fn key(&self) -> (&SiteId, &CellId) {
        match self {
            Self::Configured { site, cell, .. } | Self::Blueprint { site, cell, .. } => {
                (site, cell)
            }
        }
    }

    fn component_count(&self) -> usize {
        match self {
            Self::Configured { network, .. } => network.appliance_count(),
            Self::Blueprint { components, .. } => components.len(),
        }
    }

    fn tick(&mut self, elapsed_us: u64) {
        if let Self::Blueprint { components, .. } = self {
            for component in components {
                component.ticks = component.ticks.saturating_add(1);
                component.elapsed_us = component.elapsed_us.saturating_add(elapsed_us);
            }
        }
    }

    fn receive(&mut self, message: &PartitionMessage) {
        let Self::Blueprint { components, .. } = self else {
            return;
        };
        let target = match message {
            PartitionMessage::Network { target, .. } | PartitionMessage::Process { target, .. } => {
                components
                    .iter_mut()
                    .find(|component| component.component == target.as_str())
            }
            PartitionMessage::Telemetry { .. } | PartitionMessage::Heartbeat { .. } => {
                let index = components
                    .iter()
                    .position(|component| component.component_kind == "virtual-controller")
                    .unwrap_or(0);
                components.get_mut(index)
            }
        };
        if let Some(target) = target {
            target.received_messages = target.received_messages.saturating_add(1);
            target.last_message_class = Some(message.class());
        }
    }

    fn snapshot(&self, captured_at_us: u64) -> CellSnapshot {
        match self {
            Self::Configured {
                site,
                cell,
                network,
            } => CellSnapshot {
                schema_version: CELL_SNAPSHOT_SCHEMA_VERSION.into(),
                site: site.clone(),
                cell: cell.clone(),
                captured_at_us,
                components: network.component_snapshots(captured_at_us),
            },
            Self::Blueprint {
                site,
                cell,
                components,
            } => CellSnapshot {
                schema_version: CELL_SNAPSHOT_SCHEMA_VERSION.into(),
                site: site.clone(),
                cell: cell.clone(),
                captured_at_us,
                components: components
                    .iter()
                    .map(BlueprintComponentRuntime::snapshot)
                    .collect(),
            },
        }
    }
}

impl BlueprintComponentRuntime {
    fn snapshot(&self) -> ComponentSnapshot {
        ComponentSnapshot {
            schema_version: CELL_SNAPSHOT_SCHEMA_VERSION.into(),
            component: self.component.clone(),
            component_kind: self.component_kind.clone(),
            state: BTreeMap::from([
                (
                    "elapsed-us".into(),
                    SnapshotValue::Integer(integer_snapshot(self.elapsed_us)),
                ),
                (
                    "environment".into(),
                    SnapshotValue::Text(self.environment.clone()),
                ),
                (
                    "family".into(),
                    SnapshotValue::Text(self.component_kind.clone()),
                ),
                (
                    "last-message-class".into(),
                    SnapshotValue::Text(message_class_name(self.last_message_class).into()),
                ),
                ("ports".into(), SnapshotValue::Text(self.ports.clone())),
                (
                    "received-messages".into(),
                    SnapshotValue::Integer(integer_snapshot(self.received_messages)),
                ),
                ("site".into(), SnapshotValue::Text(self.site.clone())),
                (
                    "ticks".into(),
                    SnapshotValue::Integer(integer_snapshot(self.ticks)),
                ),
            ]),
        }
    }
}

const fn integer_snapshot(value: u64) -> i64 {
    if value > i64::MAX as u64 {
        i64::MAX
    } else {
        value as i64
    }
}

const fn message_class_name(class: Option<PartitionMessageClass>) -> &'static str {
    match class {
        Some(PartitionMessageClass::Network) => "network",
        Some(PartitionMessageClass::Process) => "process",
        Some(PartitionMessageClass::Telemetry) => "telemetry",
        Some(PartitionMessageClass::Heartbeat) => "heartbeat",
        None => "none",
    }
}

#[derive(Debug)]
pub enum SessionError {
    Build(String),
    Scheduler(crate::SchedulerError),
    Stopped,
}

impl Display for SessionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Build(detail) => write!(formatter, "simulation session build failed: {detail}"),
            Self::Scheduler(error) => Display::fmt(error, formatter),
            Self::Stopped => formatter.write_str("simulation session is stopped"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<crate::SchedulerError> for SessionError {
    fn from(error: crate::SchedulerError) -> Self {
        Self::Scheduler(error)
    }
}

fn site_id(value: &str) -> Result<SiteId, SessionError> {
    Text::try_new(value).map_err(|error| SessionError::Build(error.to_string()))
}

fn cell_id(value: &str) -> Result<CellId, SessionError> {
    Text::try_new(value).map_err(|error| SessionError::Build(error.to_string()))
}
