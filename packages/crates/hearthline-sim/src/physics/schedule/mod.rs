//! Serial, multirate execution of independent numerical populations. This is
//! the event foundation, not yet the coupled equipment/conduit integration.

mod archive;
mod diagnostic;
mod memory;
#[cfg(test)]
mod tests;

pub use archive::SCHEDULE_SNAPSHOT_SCHEMA;
pub use memory::ScheduleMemoryPlan;

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;

use hearthline_project::physics::schedule::CompiledPhysicsSchedule;
use serde::{Deserialize, Serialize};

use super::population::{PhysicsProofPopulation, PopulationError};
use super::{PhysicsCheckpoint, PhysicsStepReport};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicsEvent {
    pub time_us: u64,
    /// Index into the immutable, site/cell/instance-sorted bindings.
    pub instance: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScheduleFailure {
    pub event: PhysicsEvent,
    pub error: PopulationError,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduleError {
    Complete,
    Failed(ScheduleFailure),
}

impl std::fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "physics schedule: {self:?}")
    }
}
impl std::error::Error for ScheduleError {}

#[derive(Clone, Copy, Debug)]
pub struct ScheduledPhysicsStep {
    pub event: PhysicsEvent,
    pub sequence: u64,
    pub report: PhysicsStepReport,
    pub instance_step: u32,
    pub fields: super::NumericalFieldRanges,
}

/// Ownership is transferred to one worker, never cloned between executors.
/// A solver failure retains the failed event, last committed time, and all
/// instance banks. No new event or spatial buffer is allocated while stepping.
pub struct PhysicsSchedule {
    plan: Arc<CompiledPhysicsSchedule>,
    populations: Vec<PhysicsProofPopulation>,
    pending: BinaryHeap<Reverse<PhysicsEvent>>,
    time_us: u64,
    committed_events: u64,
    failure: Option<ScheduleFailure>,
    memory: ScheduleMemoryPlan,
}

impl PhysicsSchedule {
    pub fn new(plan: Arc<CompiledPhysicsSchedule>) -> Result<Self, String> {
        let memory = ScheduleMemoryPlan::compile(&plan)?;
        if !memory.within_budget {
            return Err("physics schedule exceeds its budget with 25% reserve".into());
        }
        let mut populations = Vec::new();
        populations
            .try_reserve_exact(plan.profiles().len())
            .map_err(|e| e.to_string())?;
        for (model, &count) in plan.profiles().iter().zip(plan.profile_counts()) {
            populations.push(PhysicsProofPopulation::new(Arc::clone(model), count)?);
        }
        let mut pending = Vec::new();
        pending
            .try_reserve_exact(plan.event_capacity())
            .map_err(|e| e.to_string())?;
        for (instance, binding) in plan.instances().iter().enumerate() {
            pending.push(Reverse(PhysicsEvent {
                instance,
                time_us: plan.profiles()[binding.profile].timestep_us(),
            }));
        }
        Ok(Self {
            plan,
            populations,
            pending: BinaryHeap::from(pending),
            time_us: 0,
            committed_events: 0,
            failure: None,
            memory,
        })
    }

    pub fn plan(&self) -> &CompiledPhysicsSchedule {
        &self.plan
    }
    pub const fn memory(&self) -> &ScheduleMemoryPlan {
        &self.memory
    }
    pub const fn time_us(&self) -> u64 {
        self.time_us
    }
    pub const fn committed_events(&self) -> u64 {
        self.committed_events
    }
    pub const fn failure(&self) -> Option<ScheduleFailure> {
        self.failure
    }
    pub fn complete(&self) -> bool {
        self.pending.is_empty()
    }
    pub fn next_event(&self) -> Option<PhysicsEvent> {
        self.pending.peek().map(|e| e.0)
    }
    pub fn pending_events(&self) -> usize {
        self.pending.len()
    }

    pub fn completed_steps(&self, instance: usize) -> Result<u32, PopulationError> {
        let binding = self
            .plan
            .instances()
            .get(instance)
            .ok_or(PopulationError::UnknownInstance)?;
        self.populations[binding.profile].completed_steps(binding.slot)
    }

    /// Host-only allocating projection. The runtime path uses streamed capture.
    pub fn checkpoint(&mut self, instance: usize) -> Result<PhysicsCheckpoint, PopulationError> {
        let binding = self
            .plan
            .instances()
            .get(instance)
            .ok_or(PopulationError::UnknownInstance)?;
        self.populations[binding.profile].checkpoint(binding.slot)
    }

    pub fn step_next(&mut self) -> Result<ScheduledPhysicsStep, ScheduleError> {
        if let Some(failure) = self.failure {
            return Err(ScheduleError::Failed(failure));
        }
        let event = self.next_event().ok_or(ScheduleError::Complete)?;
        let binding = &self.plan.instances()[event.instance];
        let population = &mut self.populations[binding.profile];
        let report = match population.step(binding.slot) {
            Ok(report) => report,
            Err(error) => {
                let failure = ScheduleFailure { event, error };
                self.failure = Some(failure);
                return Err(ScheduleError::Failed(failure));
            }
        };
        // Counts, horizons and reserve were checked before construction. Pop
        // before push preserves the preallocated heap capacity even at a tie.
        self.pending.pop();
        let model = &self.plan.profiles()[binding.profile];
        let step = population
            .completed_steps(binding.slot)
            .expect("compiled slot");
        if step < model.config().steps {
            self.pending.push(Reverse(PhysicsEvent {
                time_us: (u64::from(step) + 1) * model.timestep_us(),
                instance: event.instance,
            }));
        }
        let sequence = self.committed_events;
        self.committed_events += 1;
        self.time_us = event.time_us;
        Ok(ScheduledPhysicsStep {
            event,
            sequence,
            report,
            instance_step: step,
            fields: population.accepted_field_ranges(),
        })
    }
}
