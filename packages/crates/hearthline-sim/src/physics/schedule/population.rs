use std::io::Write;
use std::sync::Arc;

use hearthline_physics::PhysicsError;
use hearthline_physics::flow::connected::{ConnectedAdvanceError, ConnectedAdvanceReport};
use hearthline_project::physics::schedule::CompiledNumericalProfile;

use crate::physics::flow::connected::ConnectedStepError;
use crate::physics::flow::connected::population::{
    ConnectedGasPopulation, ConnectedPopulationError,
};
use crate::physics::population::{PhysicsProofPopulation, PopulationError};
use crate::physics::{
    NumericalFieldRanges, PhysicsCheckpoint, PhysicsStepError, PhysicsStepReport,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduleInstanceError {
    UnknownInstance,
    WrongProfile,
    CompletedInstance,
    Proof(PhysicsStepError),
    Connected {
        cause: PhysicsError,
        retry: Option<ConnectedAdvanceError>,
    },
}
impl std::fmt::Display for ScheduleInstanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "scheduled instance: {self:?}")
    }
}
impl std::error::Error for ScheduleInstanceError {}
impl From<PopulationError> for ScheduleInstanceError {
    fn from(error: PopulationError) -> Self {
        match error {
            PopulationError::UnknownInstance => Self::UnknownInstance,
            PopulationError::Step(error) => Self::Proof(error),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ScheduledStepReport {
    Proof(PhysicsStepReport),
    Connected(ConnectedAdvanceReport),
}

pub(super) enum ScheduledPopulation {
    Proof(Box<PhysicsProofPopulation>),
    Connected(Box<ConnectedGasPopulation>),
}
impl ScheduledPopulation {
    pub fn new(model: &CompiledNumericalProfile, count: usize) -> Result<Self, String> {
        match model {
            CompiledNumericalProfile::Proof(model) => Ok(Self::Proof(Box::new(
                PhysicsProofPopulation::new(Arc::clone(model), count)?,
            ))),
            CompiledNumericalProfile::ConnectedGas(model) => Ok(Self::Connected(Box::new(
                ConnectedGasPopulation::new(Arc::clone(model), count)?,
            ))),
        }
    }
    pub fn completed_steps(&self, slot: usize) -> Result<u32, ScheduleInstanceError> {
        match self {
            Self::Proof(p) => p.completed_steps(slot).map_err(Into::into),
            Self::Connected(p) => p
                .completed_steps(slot)
                .map_err(|_| ScheduleInstanceError::UnknownInstance),
        }
    }
    pub fn checkpoint(&mut self, slot: usize) -> Result<PhysicsCheckpoint, ScheduleInstanceError> {
        match self {
            Self::Proof(p) => p.checkpoint(slot).map_err(Into::into),
            Self::Connected(_) => Err(ScheduleInstanceError::WrongProfile),
        }
    }
    pub fn step(&mut self, slot: usize) -> Result<ScheduledStepReport, ScheduleInstanceError> {
        match self {
            Self::Proof(p) => p
                .step(slot)
                .map(ScheduledStepReport::Proof)
                .map_err(Into::into),
            Self::Connected(p) => p
                .step(slot)
                .map(ScheduledStepReport::Connected)
                .map_err(|e| match e {
                    ConnectedPopulationError::UnknownInstance => {
                        ScheduleInstanceError::UnknownInstance
                    }
                    ConnectedPopulationError::Step(ConnectedStepError::Complete) => {
                        ScheduleInstanceError::CompletedInstance
                    }
                    ConnectedPopulationError::Step(ConnectedStepError::Failed(cause)) => {
                        ScheduleInstanceError::Connected {
                            cause,
                            retry: p.retry_failure(slot).expect("validated slot"),
                        }
                    }
                }),
        }
    }
    pub fn fields(&self, slot: usize) -> NumericalFieldRanges {
        match self {
            Self::Proof(p) => p.accepted_field_ranges(),
            Self::Connected(p) => p.ranges(slot).expect("compiled slot"),
        }
    }
    pub fn stored_failure(&self, slot: usize) -> Option<ScheduleInstanceError> {
        match self {
            Self::Proof(_) => None,
            Self::Connected(p) => p.failure(slot).expect("compiled slot").map(|cause| {
                ScheduleInstanceError::Connected {
                    cause,
                    retry: p.retry_failure(slot).expect("compiled slot"),
                }
            }),
        }
    }
    pub fn write_instance(
        &mut self,
        slot: usize,
        mut output: impl Write,
    ) -> Result<(), serde_json::Error> {
        match self {
            Self::Proof(p) => {
                p.write_checkpoint(slot, &mut output)?;
                output.write_all(b"\n").map_err(serde_json::Error::io)
            }
            Self::Connected(p) => p.write_instance_snapshot(slot, output),
        }
    }
    pub fn instance_digest(&mut self, slot: usize) -> Result<[u8; 64], serde_json::Error> {
        match self {
            Self::Proof(p) => p.instance_digest(slot),
            Self::Connected(p) => p.instance_digest(slot),
        }
    }
}
