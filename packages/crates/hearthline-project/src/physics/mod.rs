pub mod capacity;
mod flow;
pub mod geometry;
pub mod material;
mod mechanics;
mod storage;
pub use flow::CompiledReceiverFlow;
pub use mechanics::CompiledSpecimenMechanics;
pub use storage::PhysicsStorageLayout;

use geometry::{CompiledSolidDomain, SolidDomainPlan};
use hearthline_config::physics::capacity::CapacityKernel;
use hearthline_config::physics::geometry::SolidGeometryDocument;
use hearthline_config::physics::material::DryingMaterialDocument;
use hearthline_config::physics::{
    PHYSICS_PROOF_SCHEMA_VERSION, PhysicsProofCase, PhysicsProofConfig,
};
use hearthline_model::ComponentId;
use hearthline_physics::gas::IsothermalReceiver;
use hearthline_physics::transport::{Grid3d, SolverLimits, TransportGrid};
use hearthline_physics::{PhysicsError, Scalar};
use material::CompiledDryingMaterial;
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const DEFAULT_PHYSICS_BUDGET_BYTES: u64 = 1_610_612_736;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PhysicsMemoryPlan {
    pub cells: u64,
    pub field_buffer_bytes: u64,
    pub immutable_mesh_bytes: u64,
    pub immutable_material_bytes: u64,
    /// Upper bound on temporary compiler arrays, released before mutable fields
    /// are allocated. The reserve covers mesh plus max(fields, compiler scratch).
    pub compiler_scratch_bytes: u64,
    pub reserve_bytes: u64,
    pub allocation_budget_bytes: u64,
}

impl PhysicsMemoryPlan {
    /// Eight scalar fields and two boundary fields are resident in the proof.
    /// The budget covers numerical buffers, not host metadata, HTTP or browser.
    pub fn compile(cells: u64, budget: u64) -> Result<Self, String> {
        let per_cell = PhysicsStorageLayout::compile([1; 3], CapacityKernel::ScalarTransport)?;
        Self::assess(
            cells,
            budget,
            storage::multiply(cells, per_cell.instance_buffer_bytes()?)?,
            0,
            0,
            0,
        )
    }

    fn assess(
        cells: u64,
        budget: u64,
        field_buffer_bytes: u64,
        immutable_mesh_bytes: u64,
        immutable_material_bytes: u64,
        compiler_scratch_bytes: u64,
    ) -> Result<Self, String> {
        let allocated = field_buffer_bytes
            .max(compiler_scratch_bytes)
            .checked_add(immutable_mesh_bytes)
            .and_then(|n| n.checked_add(immutable_material_bytes))
            .ok_or("physics memory demand overflow")?;
        let reviewed = budget
            .checked_mul(3)
            .ok_or("physics memory budget overflow")?
            / 4;
        if cells == 0 || allocated > reviewed {
            return Err("physics fields exceed the budget with 25% reserve".into());
        }
        Ok(Self {
            cells,
            field_buffer_bytes,
            immutable_mesh_bytes,
            immutable_material_bytes,
            compiler_scratch_bytes,
            reserve_bytes: budget - allocated,
            allocation_budget_bytes: budget,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub enum CompiledProofCase {
    CompressibleReceiver,
    Receiver {
        storage: Scalar,
        temperature: Scalar,
        pressure: Scalar,
        receiver_pressure: Scalar,
        conductance: Scalar,
    },
    Drying {
        moisture: Scalar,
        equilibrium: Scalar,
        temperature: Scalar,
        ambient: Scalar,
        density: Scalar,
        diffusion: Scalar,
        surface_transfer: Scalar,
        heat_capacity: Scalar,
        conductivity: Scalar,
        heat_transfer: Scalar,
        latent_heat: Scalar,
    },
}

#[derive(Clone, Debug)]
pub struct CompiledPhysicsProof {
    config: PhysicsProofConfig,
    digest: String,
    grid: Grid3d,
    timestep: Scalar,
    timestep_us: u64,
    limits: SolverLimits,
    memory: PhysicsMemoryPlan,
    storage_layout: PhysicsStorageLayout,
    case: CompiledProofCase,
    mechanics: Option<CompiledSpecimenMechanics>,
    flow: Option<CompiledReceiverFlow>,
    solid_domain: Option<CompiledSolidDomain>,
    geometry: Option<SolidGeometryDocument>,
    material: Option<CompiledDryingMaterial>,
}

impl CompiledPhysicsProof {
    pub fn compile(config: PhysicsProofConfig) -> Result<Self, String> {
        Self::compile_with_budget(config, DEFAULT_PHYSICS_BUDGET_BYTES)
    }

    pub fn compile_with_budget(
        config: PhysicsProofConfig,
        host_budget: u64,
    ) -> Result<Self, String> {
        Self::compile_with_geometry_and_budget(config, None, host_budget)
    }

    pub fn compile_with_geometry(
        config: PhysicsProofConfig,
        geometry: Option<SolidGeometryDocument>,
    ) -> Result<Self, String> {
        Self::compile_with_geometry_and_budget(config, geometry, DEFAULT_PHYSICS_BUDGET_BYTES)
    }

    /// Array counts only: replay adapters can reject oversized artifacts before
    /// allocating the immutable mesh or any mutable fields.
    pub fn planned_storage(
        config: &PhysicsProofConfig,
        geometry: Option<&SolidGeometryDocument>,
    ) -> Result<PhysicsStorageLayout, String> {
        Self::planned_storage_with_material(config, geometry, None)
    }

    pub fn planned_storage_with_material(
        config: &PhysicsProofConfig,
        geometry: Option<&SolidGeometryDocument>,
        material: Option<&DryingMaterialDocument>,
    ) -> Result<PhysicsStorageLayout, String> {
        let grid = proof_grid(config)?;
        let layout = layout_and_domain(config, geometry, grid)?.0;
        match (&config.material, material) {
            (None, None) => Ok(layout),
            (Some(_), Some(document)) => layout.with_material(document),
            _ => Err("drying material requires both a digest reference and its document".into()),
        }
    }

    pub fn compile_with_geometry_and_budget(
        config: PhysicsProofConfig,
        geometry: Option<SolidGeometryDocument>,
        host_budget: u64,
    ) -> Result<Self, String> {
        Self::compile_with_resources_and_budget(config, geometry, None, host_budget)
    }

    pub fn compile_with_resources(
        config: PhysicsProofConfig,
        geometry: Option<SolidGeometryDocument>,
        material: Option<DryingMaterialDocument>,
    ) -> Result<Self, String> {
        Self::compile_with_resources_and_budget(
            config,
            geometry,
            material,
            DEFAULT_PHYSICS_BUDGET_BYTES,
        )
    }

    pub fn compile_with_resources_and_budget(
        config: PhysicsProofConfig,
        geometry: Option<SolidGeometryDocument>,
        material: Option<DryingMaterialDocument>,
        host_budget: u64,
    ) -> Result<Self, String> {
        if config.allocation_budget_bytes > host_budget {
            return Err("physics proof exceeds the host allocation policy".into());
        }
        if config.schema_version != PHYSICS_PROOF_SCHEMA_VERSION {
            return Err("unsupported physics proof schema".into());
        }
        ComponentId::new(&config.id).map_err(|e| e.to_string())?;
        if config.steps == 0 || config.maximum_iterations == 0 {
            return Err("proof step and iteration counts must be positive".into());
        }
        let grid = proof_grid(&config)?;
        let timestep = positive(&config.timestep_s)?;
        if timestep.raw() % 1_000 != 0 {
            return Err("physics timestep must be an exact number of microseconds".into());
        }
        let timestep_us = u64::try_from(timestep.raw() / 1_000).map_err(|e| e.to_string())?;
        timestep_us
            .checked_mul(u64::from(config.steps))
            .ok_or("proof time horizon overflow")?;
        let limits = SolverLimits {
            iterations: config.maximum_iterations,
            residual: positive(&config.residual_tolerance)?,
        };
        let mechanics_config = match &config.case {
            PhysicsProofCase::DryingSpecimen { mechanics, .. } => mechanics.as_deref(),
            _ => None,
        };
        let (mut layout, domain_plan) = layout_and_domain(&config, geometry.as_ref(), grid)?;
        if mechanics_config.is_some_and(|config| !config.initial_guess.is_previous_step()) {
            layout = layout.with_reconstructible_elasticity()?;
        }
        layout = match (&config.material, material.as_ref()) {
            (None, None) => layout,
            (Some(_), Some(document)) => layout.with_material(document)?,
            _ => {
                return Err(
                    "drying material requires both a digest reference and its document".into(),
                );
            }
        };
        let (matrix_bound, compiler_scratch) =
            if domain_plan.is_some() && mechanics_config.is_some() {
                mechanics::storage_bound(layout.nodes, layout.elements)?
            } else {
                (0, 0)
            };
        let caloric_scratch = storage::multiply(
            material.as_ref().map_or(0, |document| {
                document
                    .dry_heat_capacity_j_kg_k_by_temperature
                    .as_ref()
                    .map_or(0, Vec::len)
                    .max(
                        document
                            .liquid_heat_capacity_j_kg_k_by_temperature
                            .as_ref()
                            .map_or(0, Vec::len),
                    )
            }) as u64,
            16,
        )?;
        let compiler_scratch = compiler_scratch.max(caloric_scratch);
        PhysicsMemoryPlan::assess(
            layout.cells,
            config.allocation_budget_bytes,
            layout.instance_buffer_bytes()?,
            layout.immutable_geometry_bytes,
            layout.immutable_material_bytes,
            compiler_scratch,
        )?;
        let case = match &config.case {
            PhysicsProofCase::CompressibleReceiver { .. } => {
                CompiledProofCase::CompressibleReceiver
            }
            PhysicsProofCase::IsothermalReceiver {
                temperature_k,
                gas_constant_j_kg_k,
                initial_pressure_pa,
                receiver_pressure_pa,
                conductance_kg_pa_s,
            } => {
                if grid.count() < 2 {
                    return Err("receiver proof requires at least two connected volumes".into());
                }
                let receiver = IsothermalReceiver::new(
                    grid.volume().map_err(error)?,
                    positive(temperature_k)?,
                    positive(gas_constant_j_kg_k)?,
                )
                .map_err(error)?;
                CompiledProofCase::Receiver {
                    storage: receiver.storage_kg_pa(),
                    temperature: positive(temperature_k)?,
                    pressure: positive(initial_pressure_pa)?,
                    receiver_pressure: positive(receiver_pressure_pa)?,
                    conductance: positive(conductance_kg_pa_s)?,
                }
            }
            PhysicsProofCase::DryingSpecimen {
                initial_moisture_kg_kg,
                equilibrium_moisture_kg_kg,
                initial_temperature_k,
                ambient_temperature_k,
                dry_density_kg_m3,
                moisture_diffusivity_m2_s,
                surface_transfer_m_s,
                heat_capacity_j_kg_k,
                thermal_conductivity_w_m_k,
                surface_heat_transfer_w_m2_k,
                latent_heat_j_kg,
                ..
            } => CompiledProofCase::Drying {
                moisture: nonnegative(initial_moisture_kg_kg)?,
                equilibrium: nonnegative(equilibrium_moisture_kg_kg)?,
                temperature: positive(initial_temperature_k)?,
                ambient: positive(ambient_temperature_k)?,
                density: positive(dry_density_kg_m3)?,
                diffusion: positive(moisture_diffusivity_m2_s)?,
                surface_transfer: nonnegative(surface_transfer_m_s)?,
                heat_capacity: positive(heat_capacity_j_kg_k)?,
                conductivity: positive(thermal_conductivity_w_m_k)?,
                heat_transfer: nonnegative(surface_heat_transfer_w_m2_k)?,
                latent_heat: nonnegative(latent_heat_j_kg)?,
            },
        };
        let material = match (&config.material, material) {
            (Some(settings), Some(document)) => Some(CompiledDryingMaterial::compile(
                settings, document, case, limits,
            )?),
            _ => None,
        };
        let solid_domain = domain_plan.map(SolidDomainPlan::build).transpose()?;
        let mechanics = mechanics_config
            .map(|config| CompiledSpecimenMechanics::compile(config, grid, solid_domain.as_ref()))
            .transpose()?;
        layout.immutable_geometry_bytes = storage::add(
            layout.immutable_geometry_bytes - matrix_bound,
            mechanics
                .as_ref()
                .map_or(0, CompiledSpecimenMechanics::stiffness_bytes),
        )?;
        let memory = PhysicsMemoryPlan::assess(
            layout.cells,
            config.allocation_budget_bytes,
            layout.instance_buffer_bytes()?,
            layout.immutable_geometry_bytes,
            layout.immutable_material_bytes,
            compiler_scratch,
        )?;
        let flow = match &config.case {
            PhysicsProofCase::CompressibleReceiver { flow } => {
                Some(CompiledReceiverFlow::compile(flow, grid, timestep, limits)?)
            }
            _ => None,
        };
        let mut hash = Sha256::new();
        hash.update(hearthline_physics::NUMERICAL_CONTRACT);
        if mechanics
            .as_ref()
            .is_some_and(CompiledSpecimenMechanics::reconstructible)
        {
            hash.update(mechanics::RECONSTRUCTION_CONTRACT);
            hash.update(hearthline_physics::mechanics::NORMALIZED_JACOBI_CONTRACT);
            hash.update(hearthline_physics::mechanics::SYMMETRIC_SWEEP_CONTRACT);
        }
        if geometry.is_some() {
            hash.update("hearthline-solid-domain/2:orthogonal-basin:sparse-elasticity");
        }
        if material.is_some() {
            hash.update(hearthline_physics::transport::nonlinear::NONLINEAR_DIFFUSION_CONTRACT);
            hash.update("hearthline-drying-material/1:constant-storage:table-domain");
        }
        if material
            .as_ref()
            .is_some_and(|material| material.caloric().is_some())
        {
            hash.update(hearthline_physics::material::caloric::CALORIC_CONTRACT);
            hash.update(hearthline_physics::transport::enthalpy::ENTHALPY_TRANSPORT_CONTRACT);
            hash.update("hearthline-drying-caloric/1:dry-solid:surface-resistance");
        }
        if material
            .as_ref()
            .and_then(|material| material.caloric())
            .is_some_and(|caloric| caloric.has_liquid())
        {
            hash.update(hearthline_physics::transport::enthalpy::WET_ENTHALPY_CONTRACT);
        }
        hash.update(serde_json::to_vec(&config).map_err(|e| e.to_string())?);
        let digest = crate::source::sha256_hex(hash.finalize().as_slice());
        Ok(Self {
            config,
            digest,
            grid,
            timestep,
            timestep_us,
            limits,
            memory,
            storage_layout: layout,
            case,
            mechanics,
            flow,
            solid_domain,
            geometry,
            material,
        })
    }

    pub fn config(&self) -> &PhysicsProofConfig {
        &self.config
    }
    pub const fn storage_layout(&self) -> PhysicsStorageLayout {
        self.storage_layout
    }
    pub const fn grid(&self) -> Grid3d {
        self.grid
    }
    pub const fn timestep(&self) -> Scalar {
        self.timestep
    }
    pub const fn timestep_us(&self) -> u64 {
        self.timestep_us
    }
    pub const fn limits(&self) -> SolverLimits {
        self.limits
    }
    pub fn memory(&self) -> &PhysicsMemoryPlan {
        &self.memory
    }
    pub const fn case(&self) -> CompiledProofCase {
        self.case
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn mechanics(&self) -> Option<&CompiledSpecimenMechanics> {
        self.mechanics.as_ref()
    }
    pub fn flow(&self) -> Option<&CompiledReceiverFlow> {
        self.flow.as_ref()
    }
    pub fn cell_count(&self) -> usize {
        self.storage_layout.cells as usize
    }
    pub fn solid_domain(&self) -> Option<&CompiledSolidDomain> {
        self.solid_domain.as_ref()
    }
    pub fn geometry(&self) -> Option<&SolidGeometryDocument> {
        self.geometry.as_ref()
    }
    pub fn material(&self) -> Option<&CompiledDryingMaterial> {
        self.material.as_ref()
    }
    pub fn transport_grid(&self) -> Result<TransportGrid<'_>, PhysicsError> {
        match &self.solid_domain {
            Some(domain) => Ok(TransportGrid::Sparse(domain.topology()?)),
            None => Ok(TransportGrid::Dense(self.grid)),
        }
    }
}

fn proof_grid(config: &PhysicsProofConfig) -> Result<Grid3d, String> {
    let shape = config.shape.map(|v| v as usize);
    shape
        .into_iter()
        .try_fold(1_usize, |n, axis| n.checked_mul(axis))
        .ok_or("physics lattice dimension overflow")?;
    let spacing = [
        positive(&config.spacing_m[0])?,
        positive(&config.spacing_m[1])?,
        positive(&config.spacing_m[2])?,
    ];
    Grid3d::new(shape, spacing)
        .map_err(|e| format!("physics lattice dimension or geometry error: {e}"))
}

fn layout_and_domain(
    config: &PhysicsProofConfig,
    document: Option<&SolidGeometryDocument>,
    grid: Grid3d,
) -> Result<(PhysicsStorageLayout, Option<SolidDomainPlan>), String> {
    if config.material.is_some() && !matches!(config.case, PhysicsProofCase::DryingSpecimen { .. })
    {
        return Err("drying material is only supported by a drying proof".into());
    }
    let kernel = match config.case {
        PhysicsProofCase::DryingSpecimen {
            mechanics: Some(_), ..
        } => CapacityKernel::DryingElasticity,
        PhysicsProofCase::CompressibleReceiver { .. } => CapacityKernel::CompressibleFlow,
        _ => CapacityKernel::ScalarTransport,
    };
    match (&config.geometry, document) {
        (None, None) => Ok((PhysicsStorageLayout::compile(config.shape, kernel)?, None)),
        (Some(reference), Some(document)) => {
            if !matches!(config.case, PhysicsProofCase::DryingSpecimen { .. }) {
                return Err("solid geometry is only supported by the drying proof".into());
            }
            let plan = SolidDomainPlan::compile(document, reference, grid)?;
            let mut layout = plan.storage_layout(kernel)?;
            if kernel == CapacityKernel::DryingElasticity {
                layout.immutable_geometry_bytes = storage::add(
                    layout.immutable_geometry_bytes,
                    mechanics::storage_bound(layout.nodes, layout.elements)?.0,
                )?;
            }
            Ok((layout, Some(plan)))
        }
        _ => Err("solid geometry requires both a digest reference and its document".into()),
    }
}

fn error(error: PhysicsError) -> String {
    error.to_string()
}
fn positive(value: &str) -> Result<Scalar, String> {
    let value = nonnegative(value)?;
    if value == Scalar::ZERO {
        Err("physical coefficient must be positive".into())
    } else {
        Ok(value)
    }
}
fn nonnegative(value: &str) -> Result<Scalar, String> {
    let value = value.parse::<Scalar>().map_err(error)?;
    if value < Scalar::ZERO {
        Err("physical coefficient must not be negative".into())
    } else {
        Ok(value)
    }
}
