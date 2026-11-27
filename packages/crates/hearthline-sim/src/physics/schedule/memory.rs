use hearthline_project::physics::schedule::{CompiledNumericalProfile, CompiledPhysicsSchedule};
use serde::Serialize;

use super::PhysicsEvent;
use super::population::ScheduledPopulation;
use crate::physics::flow::connected::population::{
    ConnectedGasPopulation, ConnectedPopulationMemory, ConnectedPopulationWork,
};
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
    pub maximum_snapshot_row_bytes: u64,
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
        let mut maximum_snapshot_row_bytes = 4096;
        let mut population_objects_bytes =
            bytes(plan.profiles().len(), size_of::<ScheduledPopulation>())?;
        let mut profile_restore_fit = true;
        for (model, &count) in plan.profiles().iter().zip(plan.profile_counts()) {
            match model {
                CompiledNumericalProfile::Proof(model) => {
                    let population = PopulationMemoryPlan::compile(model, count as u64)?;
                    if !population.within_budget {
                        return Err(
                            "physics schedule profile exceeds its own reviewed budget".into()
                        );
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
                    snapshot_decoder_bytes = snapshot_decoder_bytes
                        .max(snapshot.decoder_envelope_bytes - snapshot.maximum_frame_bytes);
                    maximum_snapshot_row_bytes =
                        maximum_snapshot_row_bytes.max(snapshot.maximum_frame_bytes);
                    maximum_snapshot_frame_bytes =
                        maximum_snapshot_frame_bytes.max(snapshot.maximum_frame_bytes);
                    population_objects_bytes = add(
                        population_objects_bytes,
                        size_of::<PhysicsProofPopulation>() as u64,
                    )?;
                    profile_restore_fit &= snapshot.within_budget;
                }
                CompiledNumericalProfile::ConnectedGas(model) => {
                    ConnectedPopulationWork::compile(model, count as u64)?;
                    let population = ConnectedPopulationMemory::compile(model, count as u64)?;
                    if !population.within_budget {
                        return Err(
                            "scheduled gas population exceeds its own reviewed budget".into()
                        );
                    }
                    population_allocation_bytes = add(
                        population_allocation_bytes,
                        add(
                            model.memory().field_bytes,
                            add(population.state_bank_bytes, population.slot_metadata_bytes)?,
                        )?,
                    )?;
                    immutable_numerical_bytes = add(
                        immutable_numerical_bytes,
                        add(
                            model.memory().immutable_bytes,
                            2 * size_of::<usize>() as u64,
                        )?,
                    )?;
                    population_objects_bytes = add(
                        population_objects_bytes,
                        size_of::<ConnectedGasPopulation>() as u64,
                    )?;
                    snapshot_decoder_bytes = snapshot_decoder_bytes
                        .max(population.restore_peak_bytes - population.total_planned_bytes);
                    maximum_snapshot_frame_bytes =
                        maximum_snapshot_frame_bytes.max(population.maximum_frame_bytes);
                    profile_restore_fit &= population.restore_within_budget;
                }
            }
        }
        // Keep the largest proof row alive while a framed child is decoded.
        // Child envelopes also cover their 4 KiB buffered hashing reader.
        snapshot_decoder_bytes = add(snapshot_decoder_bytes, maximum_snapshot_row_bytes)?;
        maximum_snapshot_frame_bytes = add(maximum_snapshot_frame_bytes, 4096)?;
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
            maximum_snapshot_row_bytes,
            snapshot_load_peak_bytes,
            reviewed_budget_bytes,
            within_budget: total_planned_bytes <= reviewed_budget_bytes,
            snapshot_within_budget: profile_restore_fit
                && snapshot_load_peak_bytes <= reviewed_budget_bytes,
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
