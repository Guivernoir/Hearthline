use hearthline_project::physics::schedule::CompiledPhysicsSchedule;
use serde::Serialize;

use super::PhysicsEvent;
use crate::physics::population::{
    PhysicsProofPopulation, PopulationMemoryPlan, SnapshotLoadMemoryPlan,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ScheduleMemoryPlan {
    pub profiles: usize,
    pub instances: usize,
    pub event_capacity: usize,
    pub population_allocation_bytes: u64,
    pub population_objects_bytes: u64,
    pub pending_event_bytes: u64,
    pub construction_allocation_bytes: u64,
    pub immutable_numerical_bytes: u64,
    pub immutable_schedule_bytes: u64,
    pub total_planned_bytes: u64,
    pub snapshot_decoder_bytes: u64,
    pub maximum_snapshot_frame_bytes: u64,
    pub snapshot_load_peak_bytes: u64,
    pub reviewed_budget_bytes: u64,
    pub within_budget: bool,
    pub snapshot_within_budget: bool,
}

impl ScheduleMemoryPlan {
    pub fn compile(plan: &CompiledPhysicsSchedule) -> Result<Self, String> {
        let mut population_allocation_bytes = 0;
        let mut immutable_numerical_bytes = 0;
        let mut snapshot_decoder_bytes = 0;
        let mut maximum_snapshot_frame_bytes = 0;
        for (model, &count) in plan.profiles().iter().zip(plan.profile_counts()) {
            let population = PopulationMemoryPlan::compile(model, count as u64)?;
            if !population.within_budget {
                return Err("physics schedule profile exceeds its own reviewed budget".into());
            }
            population_allocation_bytes = add(
                population_allocation_bytes,
                population.construction_allocation_bytes,
            )?;
            immutable_numerical_bytes = add(
                immutable_numerical_bytes,
                add(
                    population.immutable_geometry_bytes,
                    population.immutable_material_bytes,
                )?,
            )?;
            let snapshot = SnapshotLoadMemoryPlan::compile(model, count as u64)?;
            snapshot_decoder_bytes = snapshot_decoder_bytes.max(snapshot.decoder_envelope_bytes);
            maximum_snapshot_frame_bytes =
                maximum_snapshot_frame_bytes.max(snapshot.maximum_frame_bytes);
        }
        let population_objects_bytes =
            bytes(plan.profiles().len(), size_of::<PhysicsProofPopulation>())?;
        let pending_event_bytes = bytes(plan.event_capacity(), size_of::<PhysicsEvent>())?;
        let construction_allocation_bytes = add(
            population_allocation_bytes,
            add(population_objects_bytes, pending_event_bytes)?,
        )?;
        let immutable_schedule_bytes = plan.allocation_bytes()?;
        let total_planned_bytes = add(
            construction_allocation_bytes,
            add(immutable_numerical_bytes, immutable_schedule_bytes)?,
        )?;
        let snapshot_load_peak_bytes = add(total_planned_bytes, snapshot_decoder_bytes)?;
        let reviewed_budget_bytes =
            u64::try_from(u128::from(plan.allocation_budget_bytes()) * 3 / 4)
                .map_err(|e| e.to_string())?;
        Ok(Self {
            profiles: plan.profiles().len(),
            instances: plan.instances().len(),
            event_capacity: plan.event_capacity(),
            population_allocation_bytes,
            population_objects_bytes,
            pending_event_bytes,
            construction_allocation_bytes,
            immutable_numerical_bytes,
            immutable_schedule_bytes,
            total_planned_bytes,
            snapshot_decoder_bytes,
            maximum_snapshot_frame_bytes,
            snapshot_load_peak_bytes,
            reviewed_budget_bytes,
            within_budget: total_planned_bytes <= reviewed_budget_bytes,
            snapshot_within_budget: snapshot_load_peak_bytes <= reviewed_budget_bytes,
        })
    }
}

fn bytes(count: usize, size: usize) -> Result<u64, String> {
    count
        .checked_mul(size)
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(|| "physics schedule allocation overflow".into())
}
fn add(a: u64, b: u64) -> Result<u64, String> {
    a.checked_add(b)
        .ok_or_else(|| "physics schedule memory overflow".into())
}
