use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

use hearthline_model::{PartitionMessage, Text};
use serde::{Deserialize, Serialize};

use crate::{ConduitConfig, ConduitQueue};

pub type SiteId = Text<64>;
pub type CellId = Text<64>;
pub type ConduitId = Text<128>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CellIdentity {
    pub site: SiteId,
    pub cell: CellId,
}

impl Display for CellIdentity {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.site, self.cell)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CellRegistration {
    pub site: SiteId,
    pub cell: CellId,
    pub component_demand: usize,
    pub link_demand: usize,
    pub operational: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScheduledEnvelope {
    pub sequence: u64,
    pub sent_at_us: u64,
    pub deliver_at_us: u64,
    pub conduit: usize,
    pub source_site: SiteId,
    pub source_cell: CellId,
    pub destination_site: SiteId,
    pub destination_cell: CellId,
    pub message: PartitionMessage,
}

impl ScheduledEnvelope {
    pub(crate) fn order_key(&self) -> (u64, &str, &str, usize, u64) {
        (
            self.deliver_at_us,
            self.destination_site.as_str(),
            self.destination_cell.as_str(),
            self.conduit,
            self.sequence,
        )
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SchedulerMetrics {
    pub delivered: u64,
    pub unavailable: u64,
    pub sent: u64,
    pub conduit_high_water: usize,
    pub saturation_stops: u64,
    pub recoveries: u64,
}

pub struct StableScheduler {
    cells: Vec<CellRegistration>,
    cell_identities: Vec<Arc<CellIdentity>>,
    conduits: Vec<ConduitQueue>,
    sealed: bool,
    halted: bool,
    time_us: u64,
    next_sequence: u64,
    metrics: SchedulerMetrics,
}

impl Default for StableScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl StableScheduler {
    pub const fn new() -> Self {
        Self {
            cells: Vec::new(),
            cell_identities: Vec::new(),
            conduits: Vec::new(),
            sealed: false,
            halted: false,
            time_us: 0,
            next_sequence: 0,
            metrics: SchedulerMetrics {
                delivered: 0,
                unavailable: 0,
                sent: 0,
                conduit_high_water: 0,
                saturation_stops: 0,
                recoveries: 0,
            },
        }
    }

    pub fn register_cell(&mut self, cell: CellRegistration) -> Result<(), SchedulerError> {
        self.require_unsealed()?;
        if self
            .cells
            .iter()
            .any(|candidate| candidate.site == cell.site && candidate.cell == cell.cell)
        {
            return Err(SchedulerError::DuplicateCell(
                Box::new(cell.site),
                Box::new(cell.cell),
            ));
        }
        self.cells.push(cell);
        Ok(())
    }

    pub fn add_conduit(&mut self, config: ConduitConfig) -> Result<(), SchedulerError> {
        self.require_unsealed()?;
        self.require_cell(&config.source_site, &config.source_cell)?;
        self.require_cell(&config.destination_site, &config.destination_cell)?;
        if self
            .conduits
            .iter()
            .any(|conduit| conduit.config().id == config.id)
        {
            return Err(SchedulerError::DuplicateConduit(Box::new(config.id)));
        }
        self.conduits.push(ConduitQueue::new(config)?);
        Ok(())
    }

    pub fn seal(&mut self) -> Result<(), SchedulerError> {
        self.require_unsealed()?;
        self.cells
            .sort_by(|left, right| (&left.site, &left.cell).cmp(&(&right.site, &right.cell)));
        self.cell_identities = self
            .cells
            .iter()
            .map(|cell| {
                Arc::new(CellIdentity {
                    site: cell.site.clone(),
                    cell: cell.cell.clone(),
                })
            })
            .collect();
        self.conduits
            .sort_by(|left, right| left.config().id.cmp(&right.config().id));
        self.cells.shrink_to_fit();
        self.conduits.shrink_to_fit();
        self.sealed = true;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn send(
        &mut self,
        source_site: &SiteId,
        source_cell: &CellId,
        destination_site: &SiteId,
        destination_cell: &CellId,
        message: PartitionMessage,
    ) -> Result<u64, SchedulerError> {
        self.require_running()?;
        let source_index = self.require_cell_index(source_site, source_cell)?;
        if !self.cells[source_index].operational {
            return Err(SchedulerError::CellUnavailable(Arc::clone(
                &self.cell_identities[source_index],
            )));
        }
        let destination_index = self.require_cell_index(destination_site, destination_cell)?;
        let conduit_index = self
            .conduits
            .iter()
            .position(|conduit| {
                let config = conduit.config();
                &config.source_site == source_site
                    && &config.source_cell == source_cell
                    && &config.destination_site == destination_site
                    && &config.destination_cell == destination_cell
            })
            .ok_or_else(|| SchedulerError::UnknownRoute {
                source: Arc::clone(&self.cell_identities[source_index]),
                destination: Arc::clone(&self.cell_identities[destination_index]),
            })?;
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(SchedulerError::SequenceExhausted)?;
        let config = self.conduits[conduit_index].config();
        let envelope = ScheduledEnvelope {
            sequence,
            sent_at_us: self.time_us,
            deliver_at_us: self.time_us.saturating_add(config.latency_us),
            conduit: conduit_index,
            source_site: source_site.clone(),
            source_cell: source_cell.clone(),
            destination_site: destination_site.clone(),
            destination_cell: destination_cell.clone(),
            message,
        };
        if let Err(error) = self.conduits[conduit_index].enqueue(envelope) {
            if matches!(error, SchedulerError::Saturated { .. }) {
                self.halted = true;
                self.metrics.saturation_stops = self.metrics.saturation_stops.saturating_add(1);
            }
            return Err(error);
        }
        self.metrics.sent = self.metrics.sent.saturating_add(1);
        self.metrics.conduit_high_water = self
            .metrics
            .conduit_high_water
            .max(self.conduits[conduit_index].len());
        Ok(sequence)
    }

    pub fn pop_ready(&mut self, now_us: u64) -> Result<Option<ScheduledEnvelope>, SchedulerError> {
        self.require_sealed()?;
        if now_us < self.time_us {
            return Err(SchedulerError::TimeRegression {
                current: self.time_us,
                requested: now_us,
            });
        }
        self.time_us = now_us;
        let selected = self
            .conduits
            .iter()
            .enumerate()
            .filter_map(|(index, conduit)| conduit.front().map(|envelope| (index, envelope)))
            .filter(|(_, envelope)| envelope.deliver_at_us <= now_us)
            .min_by(|left, right| left.1.order_key().cmp(&right.1.order_key()))
            .map(|(index, _)| index);
        let Some(index) = selected else {
            return Ok(None);
        };
        let envelope = self.conduits[index]
            .pop_front()
            .expect("selected ready envelope");
        let available = self
            .require_cell(&envelope.destination_site, &envelope.destination_cell)?
            .operational;
        if available {
            self.metrics.delivered = self.metrics.delivered.saturating_add(1);
            Ok(Some(envelope))
        } else {
            self.metrics.unavailable = self.metrics.unavailable.saturating_add(1);
            Ok(Some(envelope))
        }
    }

    pub fn set_operational(
        &mut self,
        site: &SiteId,
        cell: &CellId,
        operational: bool,
    ) -> Result<(), SchedulerError> {
        self.cells
            .iter_mut()
            .find(|candidate| &candidate.site == site && &candidate.cell == cell)
            .ok_or_else(|| {
                SchedulerError::UnknownCell(Box::new(site.clone()), Box::new(cell.clone()))
            })?
            .operational = operational;
        Ok(())
    }

    pub fn recover_from_saturation(&mut self) -> Result<(), SchedulerError> {
        self.require_sealed()?;
        if !self.halted {
            return Ok(());
        }
        if self.pending_len() != 0 {
            return Err(SchedulerError::RecoveryBlocked {
                pending: self.pending_len(),
            });
        }
        self.halted = false;
        self.metrics.recoveries = self.metrics.recoveries.saturating_add(1);
        Ok(())
    }

    pub const fn is_sealed(&self) -> bool {
        self.sealed
    }
    pub const fn is_halted(&self) -> bool {
        self.halted
    }
    pub const fn time_us(&self) -> u64 {
        self.time_us
    }
    pub fn cells(&self) -> &[CellRegistration] {
        &self.cells
    }
    pub fn conduits(&self) -> &[ConduitQueue] {
        &self.conduits
    }
    pub fn metrics(&self) -> &SchedulerMetrics {
        &self.metrics
    }
    pub fn pending_len(&self) -> usize {
        self.conduits.iter().map(ConduitQueue::len).sum()
    }

    pub(crate) fn state(&self) -> (bool, bool, u64, u64, SchedulerMetrics) {
        (
            self.sealed,
            self.halted,
            self.time_us,
            self.next_sequence,
            self.metrics.clone(),
        )
    }

    pub(crate) fn restore_state(
        cells: Vec<CellRegistration>,
        conduits: Vec<ConduitQueue>,
        halted: bool,
        time_us: u64,
        next_sequence: u64,
        metrics: SchedulerMetrics,
    ) -> Self {
        let cell_identities = cells
            .iter()
            .map(|cell| {
                Arc::new(CellIdentity {
                    site: cell.site.clone(),
                    cell: cell.cell.clone(),
                })
            })
            .collect();
        Self {
            cells,
            cell_identities,
            conduits,
            sealed: true,
            halted,
            time_us,
            next_sequence,
            metrics,
        }
    }

    fn require_cell(
        &self,
        site: &SiteId,
        cell: &CellId,
    ) -> Result<&CellRegistration, SchedulerError> {
        let index = self.require_cell_index(site, cell)?;
        Ok(&self.cells[index])
    }

    fn require_cell_index(&self, site: &SiteId, cell: &CellId) -> Result<usize, SchedulerError> {
        self.cells
            .iter()
            .position(|candidate| &candidate.site == site && &candidate.cell == cell)
            .ok_or_else(|| {
                SchedulerError::UnknownCell(Box::new(site.clone()), Box::new(cell.clone()))
            })
    }

    fn require_unsealed(&self) -> Result<(), SchedulerError> {
        if self.sealed {
            Err(SchedulerError::AlreadySealed)
        } else {
            Ok(())
        }
    }

    fn require_sealed(&self) -> Result<(), SchedulerError> {
        if self.sealed {
            Ok(())
        } else {
            Err(SchedulerError::NotSealed)
        }
    }

    fn require_running(&self) -> Result<(), SchedulerError> {
        self.require_sealed()?;
        if self.halted {
            Err(SchedulerError::SimulationStopped)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchedulerError {
    AlreadySealed,
    NotSealed,
    DuplicateCell(Box<SiteId>, Box<CellId>),
    UnknownCell(Box<SiteId>, Box<CellId>),
    CellUnavailable(Arc<CellIdentity>),
    DuplicateConduit(Box<ConduitId>),
    InvalidConduit(String),
    UnknownRoute {
        source: Arc<CellIdentity>,
        destination: Arc<CellIdentity>,
    },
    Backpressure {
        conduit: Arc<ConduitId>,
        limit: usize,
    },
    Saturated {
        conduit: Arc<ConduitId>,
        limit: usize,
    },
    SimulationStopped,
    RecoveryBlocked {
        pending: usize,
    },
    TimeRegression {
        current: u64,
        requested: u64,
    },
    SequenceExhausted,
    InvalidSnapshot(String),
}

impl Display for SchedulerError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadySealed => formatter.write_str("scheduler is already sealed"),
            Self::NotSealed => formatter.write_str("scheduler must be sealed before execution"),
            Self::DuplicateCell(site, cell) => write!(formatter, "duplicate cell {site}/{cell}"),
            Self::UnknownCell(site, cell) => write!(formatter, "unknown cell {site}/{cell}"),
            Self::CellUnavailable(cell) => write!(formatter, "cell {cell} is unavailable"),
            Self::DuplicateConduit(id) => write!(formatter, "duplicate conduit {id}"),
            Self::InvalidConduit(detail) => write!(formatter, "invalid conduit: {detail}"),
            Self::UnknownRoute {
                source,
                destination,
            } => write!(formatter, "no conduit from {source} to {destination}"),
            Self::Backpressure { conduit, limit } => write!(
                formatter,
                "conduit {conduit} reached its {limit}-message limit"
            ),
            Self::Saturated { conduit, limit } => write!(
                formatter,
                "conduit {conduit} saturated at {limit}; simulation stopped"
            ),
            Self::SimulationStopped => {
                formatter.write_str("simulation is halted after conduit saturation")
            }
            Self::RecoveryBlocked { pending } => write!(
                formatter,
                "simulation recovery requires empty conduit queues; {pending} messages remain"
            ),
            Self::TimeRegression { current, requested } => write!(
                formatter,
                "scheduler time cannot regress from {current} to {requested}"
            ),
            Self::SequenceExhausted => formatter.write_str("scheduler sequence space exhausted"),
            Self::InvalidSnapshot(detail) => {
                write!(formatter, "invalid scheduler snapshot: {detail}")
            }
        }
    }
}

impl std::error::Error for SchedulerError {}
