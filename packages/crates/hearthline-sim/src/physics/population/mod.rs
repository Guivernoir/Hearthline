//! Independent numerical instances with one shared, serial solver workspace.
//! This is not yet the coupled factory scheduler or an equipment controller.

mod archive;
mod capacity;
mod checkpoint;
mod state;
pub use archive::{POPULATION_SNAPSHOT_SCHEMA, SnapshotLoadMemoryPlan};
pub use capacity::PooledCapacityComparison;

use std::mem::size_of;
use std::sync::Arc;

use hearthline_physics::Scalar;
use hearthline_project::physics::{CompiledPhysicsProof, PhysicsStorageLayout};
use serde::Serialize;

use super::{PhysicsCheckpoint, PhysicsProofSession, PhysicsStepError, PhysicsStepReport, field};
use state::SlotMetadata;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PopulationMemoryPlan {
    pub instances: u64,
    pub state_bank_bytes: u64,
    pub slot_metadata_bytes: u64,
    pub worker_buffer_bytes: u64,
    pub immutable_geometry_bytes: u64,
    pub immutable_material_bytes: u64,
    pub construction_allocation_bytes: u64,
    pub total_numerical_bytes: u64,
    pub allocation_budget_bytes: u64,
    pub reviewed_budget_bytes: u64,
    pub within_budget: bool,
}

impl PopulationMemoryPlan {
    pub fn compile(model: &CompiledPhysicsProof, instances: u64) -> Result<Self, String> {
        Self::for_layout(
            model.storage_layout(),
            instances,
            model.config().allocation_budget_bytes,
        )
    }

    pub(super) fn for_layout(
        layout: PhysicsStorageLayout,
        instances: u64,
        allocation_budget_bytes: u64,
    ) -> Result<Self, String> {
        if instances == 0 {
            return Err("physics population requires at least one instance".into());
        }
        let state_bank_bytes = layout
            .state_bytes
            .checked_mul(instances)
            .ok_or("physics population state size overflow")?;
        let slot_metadata_bytes = (size_of::<SlotMetadata>() as u64)
            .checked_mul(instances)
            .ok_or("physics population metadata size overflow")?;
        let worker_buffer_bytes = layout.instance_buffer_bytes()?;
        let immutable_geometry_bytes = layout.immutable_geometry_bytes;
        let immutable_material_bytes = layout.immutable_material_bytes;
        let construction_allocation_bytes = state_bank_bytes
            .checked_add(slot_metadata_bytes)
            .and_then(|n| n.checked_add(worker_buffer_bytes))
            .ok_or("physics population allocation size overflow")?;
        let total_numerical_bytes = construction_allocation_bytes
            .checked_add(immutable_geometry_bytes)
            .and_then(|n| n.checked_add(immutable_material_bytes))
            .ok_or("physics population resident size overflow")?;
        let reviewed_budget_bytes = u64::try_from(u128::from(allocation_budget_bytes) * 3 / 4)
            .map_err(|e| e.to_string())?;
        Ok(Self {
            instances,
            state_bank_bytes,
            slot_metadata_bytes,
            worker_buffer_bytes,
            immutable_geometry_bytes,
            immutable_material_bytes,
            construction_allocation_bytes,
            total_numerical_bytes,
            allocation_budget_bytes,
            reviewed_budget_bytes,
            within_budget: total_numerical_bytes <= reviewed_budget_bytes,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PopulationError {
    UnknownInstance,
    Step(PhysicsStepError),
}

impl std::fmt::Display for PopulationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownInstance => formatter.write_str("unknown physics instance"),
            Self::Step(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for PopulationError {}

/// Construction preallocates all slots. There is no post-construction growth.
/// The working copy is disposable; only successful steps commit to the bank.
pub struct PhysicsProofPopulation {
    worker: PhysicsProofSession,
    values: Vec<Scalar>,
    metadata: Vec<SlotMetadata>,
    stride: usize,
    memory: PopulationMemoryPlan,
}

impl PhysicsProofPopulation {
    pub fn new(model: Arc<CompiledPhysicsProof>, instances: usize) -> Result<Self, String> {
        let memory = PopulationMemoryPlan::compile(&model, instances as u64)?;
        if !memory.within_budget {
            return Err("physics population exceeds its allocation budget with 25% reserve".into());
        }
        let value_count = usize::try_from(memory.state_bank_bytes / size_of::<Scalar>() as u64)
            .map_err(|e| e.to_string())?;
        let stride =
            usize::try_from(model.storage_layout().state_bytes / size_of::<Scalar>() as u64)
                .map_err(|e| e.to_string())?;
        let worker = PhysicsProofSession::new(model)?;
        let mut values = field(value_count, Scalar::ZERO)?;
        let initial = worker
            .save_population_state(&mut values[..stride])
            .map_err(|e| e.to_string())?;
        let metadata = field(instances, initial)?;
        for instance in 1..instances {
            values.copy_within(..stride, instance * stride);
        }
        Ok(Self {
            worker,
            values,
            metadata,
            stride,
            memory,
        })
    }

    pub fn instance_count(&self) -> usize {
        self.metadata.len()
    }

    pub const fn memory(&self) -> &PopulationMemoryPlan {
        &self.memory
    }

    pub fn completed_steps(&self, instance: usize) -> Result<u32, PopulationError> {
        self.metadata
            .get(instance)
            .map(|m| m.step)
            .ok_or(PopulationError::UnknownInstance)
    }

    /// Reconstructs derived fields in the shared workspace; no spatial copy.
    pub fn write_checkpoint(
        &mut self,
        instance: usize,
        output: impl std::io::Write,
    ) -> Result<(), serde_json::Error> {
        self.load(instance)
            .map_err(|error| serde_json::Error::io(std::io::Error::other(error)))?;
        serde_json::to_writer(output, &checkpoint::CheckpointView::new(&self.worker))
    }

    pub fn step(&mut self, instance: usize) -> Result<PhysicsStepReport, PopulationError> {
        self.load(instance)?;
        let report = self.worker.step().map_err(PopulationError::Step)?;
        self.commit(instance)?;
        Ok(report)
    }

    /// Only used immediately after an accepted step by the serial scheduler.
    pub(super) fn accepted_field_ranges(&self) -> super::NumericalFieldRanges {
        self.worker.field_ranges()
    }

    /// Allocating compatibility projection, outside the sealed worker path.
    /// Use write_snapshot for allocation-free capture of the complete population.
    pub fn checkpoint(&mut self, instance: usize) -> Result<PhysicsCheckpoint, PopulationError> {
        self.load(instance)?;
        Ok(self.worker.checkpoint())
    }

    /// Validation uses the shared worker's trial buffers. A rejected snapshot
    /// never modifies a resident instance or requires a second state bank.
    pub fn restore(
        &mut self,
        instance: usize,
        checkpoint: &PhysicsCheckpoint,
    ) -> Result<(), String> {
        self.load(instance).map_err(|e| e.to_string())?;
        self.worker.restore(checkpoint)?;
        self.commit(instance).map_err(|e| e.to_string())
    }

    fn load(&mut self, instance: usize) -> Result<(), PopulationError> {
        let metadata = *self
            .metadata
            .get(instance)
            .ok_or(PopulationError::UnknownInstance)?;
        let start = instance * self.stride;
        self.worker
            .load_population_state(&self.values[start..start + self.stride], metadata)
            .map_err(|e| PopulationError::Step(e.into()))
    }

    fn commit(&mut self, instance: usize) -> Result<(), PopulationError> {
        let start = instance * self.stride;
        self.metadata[instance] = self
            .worker
            .save_population_state(&mut self.values[start..start + self.stride])
            .map_err(|e| PopulationError::Step(e.into()))?;
        Ok(())
    }
}
