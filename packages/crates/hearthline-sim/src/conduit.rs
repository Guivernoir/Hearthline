use std::collections::VecDeque;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{CellId, ConduitId, ScheduledEnvelope, SchedulerError, SiteId};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConduitOverflow {
    RejectNewest,
    DropOldest,
    CoalesceLatest,
    StopSimulation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConduitConfig {
    pub id: ConduitId,
    pub source_site: SiteId,
    pub source_cell: CellId,
    pub destination_site: SiteId,
    pub destination_cell: CellId,
    pub latency_us: u64,
    pub queue_capacity: usize,
    pub reviewed_burst: usize,
    pub overflow: ConduitOverflow,
}

impl ConduitConfig {
    pub fn validate(&self) -> Result<(), SchedulerError> {
        if self.id.trim().is_empty() {
            return Err(SchedulerError::InvalidConduit("conduit ID is empty".into()));
        }
        if self.queue_capacity == 0 || self.reviewed_burst == 0 {
            return Err(SchedulerError::InvalidConduit(format!(
                "conduit {} requires non-zero capacity and burst evidence",
                self.id
            )));
        }
        if self.reviewed_burst > self.queue_capacity
            || self.reviewed_burst.saturating_mul(4) > self.queue_capacity.saturating_mul(3)
        {
            return Err(SchedulerError::InvalidConduit(format!(
                "conduit {} does not preserve 25% queue reserve",
                self.id
            )));
        }
        if self.source_site == self.destination_site && self.source_cell == self.destination_cell {
            return Err(SchedulerError::InvalidConduit(format!(
                "conduit {} loops back to its source cell",
                self.id
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConduitMetrics {
    pub high_water: usize,
    pub rejected: u64,
    pub dropped: u64,
    pub coalesced: u64,
    pub stopped: u64,
}

pub struct ConduitQueue {
    config: ConduitConfig,
    diagnostic_id: Arc<ConduitId>,
    pending: VecDeque<ScheduledEnvelope>,
    metrics: ConduitMetrics,
}

impl ConduitQueue {
    pub fn new(config: ConduitConfig) -> Result<Self, SchedulerError> {
        config.validate()?;
        let pending = VecDeque::with_capacity(config.queue_capacity);
        let diagnostic_id = Arc::new(config.id.clone());
        Ok(Self {
            config,
            diagnostic_id,
            pending,
            metrics: ConduitMetrics::default(),
        })
    }

    pub fn config(&self) -> &ConduitConfig {
        &self.config
    }

    pub fn metrics(&self) -> ConduitMetrics {
        self.metrics
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub(crate) fn front(&self) -> Option<&ScheduledEnvelope> {
        self.pending.front()
    }

    pub(crate) fn pop_front(&mut self) -> Option<ScheduledEnvelope> {
        self.pending.pop_front()
    }

    pub(crate) fn enqueue(
        &mut self,
        envelope: ScheduledEnvelope,
    ) -> Result<Option<u64>, SchedulerError> {
        if self.pending.len() < self.config.queue_capacity {
            self.insert_ordered(envelope);
            return Ok(None);
        }
        match self.config.overflow {
            ConduitOverflow::RejectNewest => {
                self.metrics.rejected = self.metrics.rejected.saturating_add(1);
                Err(SchedulerError::Backpressure {
                    conduit: Arc::clone(&self.diagnostic_id),
                    limit: self.config.queue_capacity,
                })
            }
            ConduitOverflow::StopSimulation => {
                self.metrics.stopped = self.metrics.stopped.saturating_add(1);
                Err(SchedulerError::Saturated {
                    conduit: Arc::clone(&self.diagnostic_id),
                    limit: self.config.queue_capacity,
                })
            }
            ConduitOverflow::DropOldest => {
                let dropped = self
                    .pending
                    .pop_front()
                    .expect("saturated queue has an item")
                    .sequence;
                self.metrics.dropped = self.metrics.dropped.saturating_add(1);
                self.insert_ordered(envelope);
                Ok(Some(dropped))
            }
            ConduitOverflow::CoalesceLatest => {
                let class = envelope.message.class();
                let index = self
                    .pending
                    .iter()
                    .rposition(|candidate| candidate.message.class() == class)
                    .ok_or_else(|| SchedulerError::Backpressure {
                        conduit: Arc::clone(&self.diagnostic_id),
                        limit: self.config.queue_capacity,
                    })?;
                let replaced = self
                    .pending
                    .remove(index)
                    .expect("located envelope")
                    .sequence;
                self.metrics.coalesced = self.metrics.coalesced.saturating_add(1);
                self.insert_ordered(envelope);
                Ok(Some(replaced))
            }
        }
    }

    fn insert_ordered(&mut self, envelope: ScheduledEnvelope) {
        let position = self
            .pending
            .iter()
            .position(|candidate| candidate.order_key() > envelope.order_key())
            .unwrap_or(self.pending.len());
        self.pending.insert(position, envelope);
        self.metrics.high_water = self.metrics.high_water.max(self.pending.len());
    }

    pub(crate) fn snapshot(&self) -> Vec<ScheduledEnvelope> {
        self.pending.iter().cloned().collect()
    }

    pub(crate) fn restore(
        config: ConduitConfig,
        pending: Vec<ScheduledEnvelope>,
        metrics: ConduitMetrics,
    ) -> Result<Self, SchedulerError> {
        let mut queue = Self::new(config)?;
        if pending.len() > queue.config.queue_capacity {
            return Err(SchedulerError::InvalidSnapshot(
                "conduit queue exceeds capacity".into(),
            ));
        }
        for envelope in pending {
            queue.insert_ordered(envelope);
        }
        if metrics.high_water < queue.pending.len()
            || metrics.high_water > queue.config.queue_capacity
        {
            return Err(SchedulerError::InvalidSnapshot(
                "conduit metrics are inconsistent".into(),
            ));
        }
        queue.metrics = metrics;
        Ok(queue)
    }
}
