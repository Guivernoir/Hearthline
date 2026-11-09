use hearthline_config::physics::capacity::PhysicsCapacityConfig;
use hearthline_project::physics::capacity::{CapacitySources, PhysicsCapacityReport};
use serde::Serialize;

use super::{PopulationMemoryPlan, SnapshotLoadMemoryPlan};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProfilePoolEstimate {
    pub profile: String,
    pub memory: PopulationMemoryPlan,
}

/// Compares the unchanged study with implemented per-profile state banks and
/// streamed archives. Factory scheduling and constitutive qualification remain
/// separate: this does not turn verification blocks into production geometry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PooledCapacityComparison {
    pub baseline: PhysicsCapacityReport,
    pub storage_contract: String,
    pub profiles: Vec<ProfilePoolEstimate>,
    pub state_bank_bytes: u64,
    pub slot_metadata_bytes: u64,
    pub worker_buffer_bytes: u64,
    pub immutable_geometry_bytes: u64,
    pub immutable_material_bytes: u64,
    pub archive_decoder_envelope_bytes: u64,
    pub runtime_planned_bytes: u64,
    pub compiler_peak_bytes: u64,
    pub total_planned_bytes: u64,
    pub within_budget: bool,
    pub within_workload_capacities: bool,
}

impl PooledCapacityComparison {
    pub fn compile(config: PhysicsCapacityConfig) -> Result<Self, String> {
        Self::compile_with_sources(config, CapacitySources::default())
    }

    pub fn compile_with_sources(
        config: PhysicsCapacityConfig,
        sources: CapacitySources,
    ) -> Result<Self, String> {
        let baseline = PhysicsCapacityReport::compile_with_sources(config, sources)?;
        let mut profiles = Vec::new();
        let mut state_bank_bytes = 0;
        let mut slot_metadata_bytes = 0;
        let mut worker_buffer_bytes = 0;
        let mut immutable_geometry_bytes = 0;
        let mut immutable_material_bytes = 0;
        let mut archive_decoder_envelope_bytes = 0;
        for profile in &baseline.profiles {
            let count = baseline
                .populations
                .iter()
                .filter(|p| p.population.profile == profile.profile.id)
                .try_fold(0_u64, |sum, p| add(sum, p.population.capacity))?;
            let archive = SnapshotLoadMemoryPlan::for_layout(
                profile.layout,
                count,
                baseline.allocation_budget_bytes,
            )?;
            let memory = archive.population;
            state_bank_bytes = add(state_bank_bytes, memory.state_bank_bytes)?;
            slot_metadata_bytes = add(slot_metadata_bytes, memory.slot_metadata_bytes)?;
            worker_buffer_bytes = add(worker_buffer_bytes, memory.worker_buffer_bytes)?;
            immutable_geometry_bytes =
                add(immutable_geometry_bytes, memory.immutable_geometry_bytes)?;
            immutable_material_bytes =
                add(immutable_material_bytes, memory.immutable_material_bytes)?;
            archive_decoder_envelope_bytes =
                archive_decoder_envelope_bytes.max(archive.decoder_envelope_bytes);
            profiles.push(ProfilePoolEstimate {
                profile: profile.profile.id.clone(),
                memory,
            });
        }
        let runtime_planned_bytes = add(
            add(
                add(state_bank_bytes, slot_metadata_bytes)?,
                worker_buffer_bytes,
            )?,
            add(
                add(
                    add(immutable_geometry_bytes, immutable_material_bytes)?,
                    archive_decoder_envelope_bytes,
                )?,
                baseline.auxiliary_envelope_bytes,
            )?,
        )?;
        let compiler_peak_bytes = baseline.compiler_peak_bytes;
        let total_planned_bytes = runtime_planned_bytes.max(compiler_peak_bytes);
        Ok(Self {
            storage_contract:
                "hearthline-physics-population/5:independent-state:optional-zero-start-elasticity:streamed-snapshot".into(),
            profiles,
            state_bank_bytes,
            slot_metadata_bytes,
            worker_buffer_bytes,
            immutable_geometry_bytes,
            immutable_material_bytes,
            archive_decoder_envelope_bytes,
            runtime_planned_bytes,
            compiler_peak_bytes,
            total_planned_bytes,
            within_budget: total_planned_bytes <= baseline.reviewed_budget_bytes,
            within_workload_capacities: baseline.within_population_capacities
                && baseline.within_auxiliary_capacities,
            baseline,
        })
    }

    pub fn require_fit(&self) -> Result<(), String> {
        if !self.within_budget || !self.within_workload_capacities {
            return Err("pooled physics study exceeds a reviewed memory or workload limit; no physical fields were removed".into());
        }
        Ok(())
    }
}

fn add(left: u64, right: u64) -> Result<u64, String> {
    left.checked_add(right)
        .ok_or_else(|| "pooled physics capacity overflow".into())
}
