use std::collections::{BTreeMap, BTreeSet};

use hearthline_config::physics::capacity::{
    AuxiliaryResource, AuxiliaryStorage, CapacityProfile, PHYSICS_CAPACITY_SCHEMA_VERSION,
    PhysicsCapacityConfig, PhysicsPopulation, PopulationWorkload, StorageOverflow,
};
use hearthline_model::ComponentId;
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::storage::{add, multiply};
use super::{DEFAULT_PHYSICS_BUDGET_BYTES, PhysicsStorageLayout};

mod models;
pub use models::{CapacitySources, proof_source_digest};

const CAPACITY_CONTRACT: &str =
    "hearthline-physics-capacity/3:source-backed-layouts:compilation-peak";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProfileStorage {
    pub profile: CapacityProfile,
    pub layout: PhysicsStorageLayout,
    pub compiled_model_digest: Option<String>,
    pub compiler_scratch_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PopulationCapacity {
    pub population: PhysicsPopulation,
    pub peak_instances: u64,
    pub reviewed_limit: u64,
    pub within_capacity: bool,
    pub state_bytes: u64,
    pub scratch_and_coefficients_bytes: u64,
    pub snapshot_field_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuxiliaryCapacity {
    pub declaration: AuxiliaryStorage,
    pub reviewed_limit: u64,
    pub within_capacity: bool,
    pub allocated_bytes: u64,
}

/// Kernel arrays are measured layouts; auxiliary storage remains a declared
/// envelope. A fitting study is never evidence of factory numerical accuracy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PhysicsCapacityReport {
    pub schema_version: String,
    pub id: String,
    pub digest: String,
    pub capacity_contract: String,
    pub profiles: Vec<ProfileStorage>,
    pub populations: Vec<PopulationCapacity>,
    pub auxiliary: Vec<AuxiliaryCapacity>,
    pub immutable_geometry_bytes: u64,
    pub immutable_material_bytes: u64,
    pub allocated_instance_bytes: u64,
    pub snapshot_field_bytes: u64,
    pub auxiliary_envelope_bytes: u64,
    pub runtime_planned_bytes: u64,
    pub compiler_peak_bytes: u64,
    pub total_planned_bytes: u64,
    pub allocation_budget_bytes: u64,
    pub reviewed_budget_bytes: u64,
    pub within_budget: bool,
    pub within_population_capacities: bool,
    pub within_auxiliary_capacities: bool,
    pub qualification_gaps: [QualificationGap; 5],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum QualificationGap {
    FactoryConstitutiveModelsAndGeometry,
    ReusableWorkerAndStreamingArchive,
    AuxiliaryRuntimeMeasurements,
    FullApplicationResidentMemory,
    NativeReplayAndPhysicalValidation,
}

impl PhysicsCapacityReport {
    /// Produces a deterministic estimate without allocating any spatial fields.
    pub fn compile(config: PhysicsCapacityConfig) -> Result<Self, String> {
        Self::compile_with_budget(config, DEFAULT_PHYSICS_BUDGET_BYTES)
    }

    pub fn compile_with_budget(
        config: PhysicsCapacityConfig,
        host_budget: u64,
    ) -> Result<Self, String> {
        Self::compile_with_sources_and_budget(config, CapacitySources::default(), host_budget)
    }

    pub fn compile_with_sources(
        config: PhysicsCapacityConfig,
        sources: CapacitySources,
    ) -> Result<Self, String> {
        Self::compile_with_sources_and_budget(config, sources, DEFAULT_PHYSICS_BUDGET_BYTES)
    }

    pub fn compile_with_sources_and_budget(
        mut config: PhysicsCapacityConfig,
        sources: CapacitySources,
        host_budget: u64,
    ) -> Result<Self, String> {
        if config.schema_version != PHYSICS_CAPACITY_SCHEMA_VERSION {
            return Err("unsupported physics capacity schema".into());
        }
        valid_id(&config.id)?;
        if !(25..100).contains(&config.reserve_percent) {
            return Err("physics reserve must be at least 25% and less than 100%".into());
        }
        if config.allocation_budget_bytes == 0 || config.allocation_budget_bytes > host_budget {
            return Err("physics capacity budget exceeds the host policy or is zero".into());
        }
        if config.snapshot_copies == 0
            || config.profiles.is_empty()
            || config.populations.is_empty()
        {
            return Err("physics study requires profiles, populations and snapshot storage".into());
        }
        config.profiles.sort_by(|a, b| a.id.cmp(&b.id));
        config.populations.sort_by(|a, b| a.id.cmp(&b.id));
        config.auxiliary.sort_by_key(|item| item.resource);
        let mut hash = Sha256::new();
        hash.update(CAPACITY_CONTRACT);
        hash.update(serde_json::to_vec(&config).map_err(|e| e.to_string())?);
        let mut resolver = models::Resolver::new(sources)?;
        let mut layouts = BTreeMap::new();
        let mut profiles = Vec::new();
        let mut immutable_geometry_bytes = 0;
        let mut immutable_material_bytes = 0;
        let mut compiler_scratch_bytes = 0;
        for profile in config.profiles {
            valid_id(&profile.id)?;
            evidence(&profile.basis)?;
            let storage = resolver.resolve(profile, host_budget)?;
            let profile = &storage.profile;
            let layout = storage.layout;
            if layouts.insert(profile.id.clone(), layout).is_some() {
                return Err(format!("duplicate physics profile {}", profile.id));
            }
            immutable_geometry_bytes =
                add(immutable_geometry_bytes, layout.immutable_geometry_bytes)?;
            immutable_material_bytes =
                add(immutable_material_bytes, layout.immutable_material_bytes)?;
            compiler_scratch_bytes = compiler_scratch_bytes.max(storage.compiler_scratch_bytes);
            profiles.push(storage);
        }
        resolver.finish()?;
        hash.update(serde_json::to_vec(&profiles).map_err(|e| e.to_string())?);
        let digest = crate::source::sha256_hex(hash.finalize().as_slice());
        let mut populations = Vec::new();
        let mut ids = BTreeSet::new();
        let mut used_profiles = BTreeSet::new();
        let mut allocated_instance_bytes = 0;
        let mut snapshot_field_bytes = 0;
        for population in config.populations {
            valid_id(&population.id)?;
            valid_id(&population.area)?;
            evidence(&population.basis)?;
            if !ids.insert(population.id.clone()) {
                return Err(format!("duplicate physics population {}", population.id));
            }
            let layout = layouts
                .get(&population.profile)
                .ok_or_else(|| format!("unknown physics profile {}", population.profile))?;
            used_profiles.insert(population.profile.clone());
            let peak_instances = population_demand(population.workload)?;
            let reviewed_limit = reviewed(population.capacity, config.reserve_percent)?;
            let state_bytes = multiply(layout.state_bytes, population.capacity)?;
            let scratch_and_coefficients_bytes =
                multiply(layout.scratch_and_coefficients_bytes, population.capacity)?;
            let snapshot_bytes = multiply(
                multiply(layout.checkpoint_field_bytes, population.capacity)?,
                u64::from(config.snapshot_copies),
            )?;
            allocated_instance_bytes = add(
                allocated_instance_bytes,
                add(state_bytes, scratch_and_coefficients_bytes)?,
            )?;
            snapshot_field_bytes = add(snapshot_field_bytes, snapshot_bytes)?;
            populations.push(PopulationCapacity {
                population,
                peak_instances,
                reviewed_limit,
                within_capacity: peak_instances <= reviewed_limit,
                state_bytes,
                scratch_and_coefficients_bytes,
                snapshot_field_bytes: snapshot_bytes,
            });
        }
        if used_profiles.len() != layouts.len() {
            return Err("physics study contains an unused profile".into());
        }
        let auxiliary = auxiliary(config.auxiliary, config.reserve_percent)?;
        let auxiliary_envelope_bytes = auxiliary
            .iter()
            .try_fold(0, |sum, item| add(sum, item.allocated_bytes))?;
        let immutable_bytes = add(immutable_geometry_bytes, immutable_material_bytes)?;
        let runtime_planned_bytes = add(
            add(immutable_bytes, allocated_instance_bytes)?,
            add(snapshot_field_bytes, auxiliary_envelope_bytes)?,
        )?;
        // Source compilation precedes field allocation. Charge all retained
        // models plus the largest sequential compiler workspace and host envelope.
        let compiler_peak_bytes = add(
            add(immutable_bytes, compiler_scratch_bytes)?,
            auxiliary_envelope_bytes,
        )?;
        let total_planned_bytes = runtime_planned_bytes.max(compiler_peak_bytes);
        let reviewed_budget_bytes =
            reviewed(config.allocation_budget_bytes, config.reserve_percent)?;
        Ok(Self {
            schema_version: PHYSICS_CAPACITY_SCHEMA_VERSION.into(),
            id: config.id,
            digest,
            capacity_contract: CAPACITY_CONTRACT.into(),
            profiles,
            within_population_capacities: populations.iter().all(|item| item.within_capacity),
            within_auxiliary_capacities: auxiliary.iter().all(|item| item.within_capacity),
            populations,
            auxiliary,
            immutable_geometry_bytes,
            immutable_material_bytes,
            allocated_instance_bytes,
            snapshot_field_bytes,
            auxiliary_envelope_bytes,
            runtime_planned_bytes,
            compiler_peak_bytes,
            total_planned_bytes,
            allocation_budget_bytes: config.allocation_budget_bytes,
            reviewed_budget_bytes,
            within_budget: total_planned_bytes <= reviewed_budget_bytes,
            qualification_gaps: [
                QualificationGap::FactoryConstitutiveModelsAndGeometry,
                QualificationGap::ReusableWorkerAndStreamingArchive,
                QualificationGap::AuxiliaryRuntimeMeasurements,
                QualificationGap::FullApplicationResidentMemory,
                QualificationGap::NativeReplayAndPhysicalValidation,
            ],
        })
    }

    /// Budget acceptance only. This cannot authorize a canonical model migration.
    pub fn require_fit(&self) -> Result<(), String> {
        if !self.within_budget
            || !self.within_population_capacities
            || !self.within_auxiliary_capacities
        {
            return Err("physics resource study exceeds a reviewed memory or workload limit; fields and residence times were not reduced".into());
        }
        Ok(())
    }
}

pub fn population_demand(workload: PopulationWorkload) -> Result<u64, String> {
    match workload {
        PopulationWorkload::Fixed { count } if count > 0 => Ok(count),
        PopulationWorkload::Production {
            sources,
            minimum_cycle_us,
            maximum_residence_us,
            simultaneous_burst,
        } if sources > 0 && minimum_cycle_us > 0 && maximum_residence_us > 0 => {
            // Ceil per source: combining source rates first undercounts
            // synchronized arrivals when residence is not a cycle multiple.
            let per_source = maximum_residence_us.div_ceil(minimum_cycle_us);
            add(
                multiply(u64::from(sources), per_source)?,
                simultaneous_burst,
            )
        }
        _ => Err("physics population has no positive, bounded workload evidence".into()),
    }
}

fn auxiliary(
    declarations: Vec<AuxiliaryStorage>,
    reserve: u8,
) -> Result<Vec<AuxiliaryCapacity>, String> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for declaration in declarations {
        if !seen.insert(declaration.resource) {
            return Err("duplicate auxiliary physics resource".into());
        }
        evidence(&declaration.basis)?;
        if declaration.peak_entries == 0 || declaration.bytes_per_entry == 0 {
            return Err("auxiliary resource requires positive workload and entry size".into());
        }
        let allowed = match declaration.resource {
            AuxiliaryResource::TransferQueues => {
                matches!(declaration.overflow, StorageOverflow::Backpressure)
            }
            AuxiliaryResource::Historian => matches!(
                declaration.overflow,
                StorageOverflow::Backpressure
                    | StorageOverflow::StopRecording
                    | StorageOverflow::DropOldest
            ),
            AuxiliaryResource::Projections => matches!(
                declaration.overflow,
                StorageOverflow::Backpressure | StorageOverflow::DropOldest
            ),
            _ => declaration.overflow == StorageOverflow::RejectStart,
        };
        if !allowed {
            return Err("auxiliary overflow policy would discard authoritative state or is incompatible with its resource".into());
        }
        let reviewed_limit = reviewed(declaration.capacity_entries, reserve)?;
        result.push(AuxiliaryCapacity {
            within_capacity: declaration.peak_entries <= reviewed_limit,
            allocated_bytes: multiply(declaration.capacity_entries, declaration.bytes_per_entry)?,
            declaration,
            reviewed_limit,
        });
    }
    if AuxiliaryResource::ALL
        .iter()
        .any(|resource| !seen.contains(resource))
    {
        return Err("missing auxiliary physics resource evidence".into());
    }
    Ok(result)
}

fn reviewed(capacity: u64, reserve: u8) -> Result<u64, String> {
    if capacity == 0 {
        return Err("physics resource capacity must be positive".into());
    }
    u64::try_from(u128::from(capacity) * u128::from(100 - reserve) / 100).map_err(|e| e.to_string())
}

fn valid_id(id: &str) -> Result<(), String> {
    ComponentId::new(id).map(|_| ()).map_err(|e| e.to_string())
}

fn evidence(basis: &str) -> Result<(), String> {
    if basis.trim().is_empty() {
        return Err("physics resource assumptions must be documented".into());
    }
    Ok(())
}
