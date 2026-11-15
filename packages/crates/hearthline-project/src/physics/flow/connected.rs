//! Immutable gas-island parameters. Source adapters must supply SI values;
//! normalization and the initial extensive inventories are compiler-owned.

use hearthline_physics::flow::connected::{
    CONNECTED_ADVANCE_CONTRACT, CONNECTED_FLOW_CONTRACT, CellInventory, ConnectedFlow,
    ConnectedRetryPolicy,
};
use hearthline_physics::flow::{
    Conserved, FlowBoundary, GasScales, IdealGas, Primitive, ViscousTransport,
};
use hearthline_physics::transport::{Grid3d, SolverLimits};
use hearthline_physics::{NUMERICAL_CONTRACT, PhysicsError, Scalar};
use sha2::{Digest, Sha256};

use super::mesh::{CompiledFlowMesh, ConnectedMemoryPlan, FlowPatchConnection, FlowRegion};
use crate::physics::schedule::PhysicsInstanceIdentity;

pub const CONNECTED_GAS_MODEL_CONTRACT: &str =
    "hearthline-connected-gas/1:si-inputs:whole-island-commit";

#[derive(Clone, Copy, Debug)]
pub struct GasReference {
    pub length_m: Scalar,
    pub velocity_m_s: Scalar,
    pub density_kg_m3: Scalar,
    pub gas_constant_j_kg_k: Scalar,
}

#[derive(Clone, Copy, Debug)]
pub struct ConnectedGasProfile {
    pub reference: GasReference,
    pub heat_capacity_ratio: Scalar,
    pub dynamic_viscosity_pa_s: Scalar,
    pub thermal_conductivity_w_m_k: Scalar,
    pub timestep_us: u64,
    pub steps: u32,
    pub limits: SolverLimits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GasRetryConfig {
    pub maximum_substeps: u32,
    pub minimum_timestep_us: u64,
}

#[derive(Clone, Debug)]
pub struct RegionInitialGas {
    pub identity: PhysicsInstanceIdentity,
    pub density_kg_m3: Scalar,
    pub pressure_pa: Scalar,
    pub velocity_m_s: [Scalar; 3],
}

#[derive(Clone, Debug)]
pub struct GasRegion {
    pub identity: PhysicsInstanceIdentity,
    pub shape: [usize; 3],
    pub spacing_m: [Scalar; 3],
    pub boundaries: [FlowBoundary; 6],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectedGasMemory {
    pub immutable_bytes: u64,
    pub field_bytes: u64,
    pub compiler_peak_bytes: u64,
    pub runtime_bytes: u64,
    pub allocation_budget_bytes: u64,
}

impl ConnectedGasMemory {
    fn compile(
        mesh: ConnectedMemoryPlan,
        regions: usize,
        initial_capacity: usize,
        source_bytes: u64,
        source_overhead: u64,
    ) -> Result<Self, String> {
        let extra = (regions as u64)
            .checked_mul(size_of::<Conserved>() as u64)
            .and_then(|n| n.checked_add(size_of::<CompiledConnectedGas>() as u64 + 64))
            .ok_or("connected model memory overflow")?;
        let immutable_bytes = mesh
            .immutable_bytes
            .checked_add(extra)
            .ok_or("connected model memory overflow")?;
        let field_bytes = mesh
            .cells
            .checked_mul(4 * size_of::<CellInventory>() as u64)
            .ok_or("connected model field overflow")?;
        let runtime_bytes = immutable_bytes
            .checked_add(field_bytes)
            .ok_or("connected model memory overflow")?;
        let compiler_peak_bytes = immutable_bytes
            .checked_add(mesh.compiler_input_bytes)
            .and_then(|n| {
                n.checked_add(
                    (initial_capacity as u64).checked_mul(size_of::<RegionInitialGas>() as u64)?,
                )
            })
            .and_then(|n| n.checked_add(source_bytes))
            .and_then(|n| n.checked_add(source_overhead))
            .ok_or("connected model compiler overflow")?;
        if u128::from(runtime_bytes.max(compiler_peak_bytes)) * 4
            > u128::from(mesh.allocation_budget_bytes) * 3
        {
            return Err("connected gas model exceeds budget with 25% reserve".into());
        }
        Ok(Self {
            immutable_bytes,
            field_bytes,
            compiler_peak_bytes,
            runtime_bytes,
            allocation_budget_bytes: mesh.allocation_budget_bytes,
        })
    }
}

#[derive(Debug)]
pub struct CompiledConnectedGas {
    mesh: CompiledFlowMesh,
    profile: ConnectedGasProfile,
    scales: GasScales,
    gas: IdealGas,
    transport: ViscousTransport,
    solver_dt: Scalar,
    retries: Option<ConnectedRetryPolicy>,
    evaluation_bound: u64,
    initial: Box<[Conserved]>,
    initial_inventory: CellInventory,
    memory: ConnectedGasMemory,
    digest: String,
}

impl CompiledConnectedGas {
    /// Region spacing, reservoir states and wall temperatures/velocities are SI.
    pub fn compile(
        sources: Vec<GasRegion>,
        connections: Vec<FlowPatchConnection>,
        initial: Vec<RegionInitialGas>,
        profile: ConnectedGasProfile,
        budget: u64,
    ) -> Result<Self, String> {
        Self::compile_inner(sources, connections, initial, profile, budget, None, 0)
    }
    pub fn compile_with_retries(
        sources: Vec<GasRegion>,
        connections: Vec<FlowPatchConnection>,
        initial: Vec<RegionInitialGas>,
        profile: ConnectedGasProfile,
        budget: u64,
        retries: GasRetryConfig,
    ) -> Result<Self, String> {
        Self::compile_inner(
            sources,
            connections,
            initial,
            profile,
            budget,
            Some(retries),
            0,
        )
    }
    pub(super) fn compile_inner(
        sources: Vec<GasRegion>,
        connections: Vec<FlowPatchConnection>,
        mut initial: Vec<RegionInitialGas>,
        profile: ConnectedGasProfile,
        budget: u64,
        retries: Option<GasRetryConfig>,
        source_overhead: u64,
    ) -> Result<Self, String> {
        let source_bytes = (sources.capacity() as u64)
            .checked_mul(size_of::<GasRegion>() as u64)
            .ok_or("gas source size overflow")?;
        let metadata = source_bytes
            .checked_add(
                (initial.capacity() as u64)
                    .checked_mul(size_of::<RegionInitialGas>() as u64)
                    .ok_or("gas initial size overflow")?,
            )
            .and_then(|n| {
                n.checked_add(
                    (connections.capacity() as u64)
                        .checked_mul(size_of::<FlowPatchConnection>() as u64)?,
                )
            })
            .and_then(|n| {
                n.checked_add((sources.len() as u64).checked_mul(size_of::<FlowRegion>() as u64)?)
            })
            .and_then(|n| n.checked_add(source_overhead))
            .ok_or("gas source size overflow")?;
        if u128::from(metadata) * 4 > u128::from(budget) * 3 {
            return Err("gas source metadata exceeds budget with reserve".into());
        }
        if profile.steps == 0
            || profile.timestep_us == 0
            || profile
                .timestep_us
                .checked_mul(u64::from(profile.steps))
                .is_none()
        {
            return Err("connected gas requires a positive representable physical horizon".into());
        }
        let reference = profile.reference;
        let scales = GasScales::new(
            reference.length_m,
            reference.velocity_m_s,
            reference.density_kg_m3,
            reference.gas_constant_j_kg_k,
        )
        .map_err(|e| e.to_string())?;
        let gas = IdealGas::new(profile.heat_capacity_ratio).map_err(|e| e.to_string())?;
        if profile.dynamic_viscosity_pa_s < Scalar::ZERO
            || profile.thermal_conductivity_w_m_k < Scalar::ZERO
        {
            return Err("gas transport coefficients must be nonnegative".into());
        }
        let viscosity = scales
            .viscosity_to_solver(profile.dynamic_viscosity_pa_s)
            .map_err(|e| e.to_string())?;
        let conductivity = scales
            .conductivity_to_solver(profile.thermal_conductivity_w_m_k)
            .map_err(|e| e.to_string())?;
        if (profile.dynamic_viscosity_pa_s > Scalar::ZERO && viscosity == Scalar::ZERO)
            || (profile.thermal_conductivity_w_m_k > Scalar::ZERO && conductivity == Scalar::ZERO)
        {
            return Err("reference scales erase a positive transport coefficient".into());
        }
        let transport = ViscousTransport::new(viscosity, conductivity, Scalar::ONE)
            .map_err(|e| e.to_string())?;
        let physical_dt = Scalar::ratio_wide(i128::from(profile.timestep_us), 1_000_000)
            .map_err(|e| e.to_string())?;
        let solver_dt = scales
            .time_to_solver(physical_dt)
            .map_err(|e| e.to_string())?;
        if solver_dt <= Scalar::ZERO
            || solver_dt
                .checked_mul(scales.time())
                .map_err(|e| e.to_string())?
                != physical_dt
        {
            return Err("connected gas timestep is unresolved by its reference scales".into());
        }
        let retries = retries
            .map(|config| -> Result<ConnectedRetryPolicy, String> {
                let physical_minimum =
                    Scalar::ratio_wide(i128::from(config.minimum_timestep_us), 1_000_000)
                        .map_err(|e| e.to_string())?;
                let minimum_step = scales
                    .time_to_solver(physical_minimum)
                    .map_err(|e| e.to_string())?;
                if minimum_step
                    .checked_mul(scales.time())
                    .map_err(|e| e.to_string())?
                    != physical_minimum
                {
                    return Err(
                        "connected retry minimum is unresolved by its reference scales".into(),
                    );
                }
                Ok(ConnectedRetryPolicy {
                    maximum_substeps: config.maximum_substeps,
                    minimum_step,
                })
            })
            .transpose()?;
        let evaluation_bound = retries.map_or_else(
            || Ok(2 * u64::from(profile.limits.iterations)),
            |policy| {
                policy
                    .evaluation_bound(solver_dt, profile.limits)
                    .map_err(|e| e.to_string())
            },
        )?;
        let mut regions = Vec::with_capacity(sources.len());
        for source in &sources {
            let spacing = source.spacing_m.map(|v| scales.length_to_solver(v));
            let grid = Grid3d::new(
                source.shape,
                [
                    spacing[0].map_err(|e| e.to_string())?,
                    spacing[1].map_err(|e| e.to_string())?,
                    spacing[2].map_err(|e| e.to_string())?,
                ],
            )
            .map_err(|e| e.to_string())?;
            let mut boundaries = source.boundaries;
            for (face, boundary) in boundaries.iter_mut().enumerate() {
                *boundary = match *boundary {
                    FlowBoundary::Reservoir(state) => FlowBoundary::Reservoir(
                        normalize(state, scales).map_err(|e| e.to_string())?,
                    ),
                    FlowBoundary::NoSlipWall {
                        velocity,
                        temperature,
                    } => {
                        if velocity[face / 2] != Scalar::ZERO {
                            return Err("moving normal walls require moving geometry".into());
                        }
                        let v = velocity.map(|v| scales.velocity_to_solver(v));
                        FlowBoundary::NoSlipWall {
                            velocity: [
                                v[0].map_err(|e| e.to_string())?,
                                v[1].map_err(|e| e.to_string())?,
                                v[2].map_err(|e| e.to_string())?,
                            ],
                            temperature: temperature
                                .map(|v| scales.temperature_to_solver(v))
                                .transpose()
                                .map_err(|e| e.to_string())?,
                        }
                    }
                    other => other,
                };
            }
            regions.push(FlowRegion {
                identity: source.identity.clone(),
                grid,
                boundaries,
            });
        }
        let base = ConnectedMemoryPlan::compile(
            &regions,
            regions.capacity(),
            connections.capacity(),
            connections.len(),
            budget,
        )?;
        let memory = ConnectedGasMemory::compile(
            base,
            regions.len(),
            initial.capacity(),
            source_bytes,
            source_overhead,
        )?;
        initial.sort_unstable_by(|a, b| a.identity.cmp(&b.identity));
        if initial.len() != regions.len()
            || initial.windows(2).any(|p| p[0].identity == p[1].identity)
        {
            return Err("one unique initial gas state is required for every region".into());
        }
        let mesh = CompiledFlowMesh::compile(regions, connections, budget)?;
        let mut normalized = Vec::with_capacity(initial.len());
        let mut totals = [0_i128; 5];
        for (source, region) in initial.iter().zip(mesh.regions()) {
            if source.identity != region.identity {
                return Err("initial gas identity does not match compiled region".into());
            }
            let state = gas
                .conserved(
                    normalize(
                        Primitive {
                            density: source.density_kg_m3,
                            pressure: source.pressure_pa,
                            velocity: source.velocity_m_s,
                        },
                        scales,
                    )
                    .map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
            let inventory = CellInventory::from_density(
                state,
                region.grid.volume().map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            let recovered = gas
                .primitive(
                    inventory
                        .density(region.grid.volume().map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
            readings(scales, recovered).map_err(|e| e.to_string())?;
            for (total, value) in totals.iter_mut().zip(inventory.components()) {
                *total = total
                    .checked_add(i128::from(value.raw()) * region.grid.count() as i128)
                    .ok_or("initial gas inventory overflow")?;
            }
            normalized.push(state);
        }
        let mut inventory = [Scalar::ZERO; 5];
        for (out, total) in inventory.iter_mut().zip(totals) {
            *out = Scalar::from_raw(i64::try_from(total).map_err(|e| e.to_string())?);
        }
        let mut model = Self {
            mesh,
            profile,
            scales,
            gas,
            transport,
            solver_dt,
            retries,
            evaluation_bound,
            initial: normalized.into_boxed_slice(),
            initial_inventory: CellInventory::from_components(inventory),
            memory,
            digest: String::new(),
        };
        model.solver().map_err(|e| e.to_string())?;
        model.digest = model.hash();
        Ok(model)
    }
    pub fn mesh(&self) -> &CompiledFlowMesh {
        &self.mesh
    }
    pub const fn profile(&self) -> ConnectedGasProfile {
        self.profile
    }
    pub const fn solver_dt(&self) -> Scalar {
        self.solver_dt
    }
    pub const fn retries(&self) -> Option<ConnectedRetryPolicy> {
        self.retries
    }
    pub const fn evaluation_bound(&self) -> u64 {
        self.evaluation_bound
    }
    pub const fn initial_inventory(&self) -> CellInventory {
        self.initial_inventory
    }
    pub const fn memory(&self) -> &ConnectedGasMemory {
        &self.memory
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn solver(&self) -> Result<ConnectedFlow<'_>, PhysicsError> {
        ConnectedFlow::new(
            self.mesh
                .mesh()
                .map_err(|_| PhysicsError::InvalidGeometry)?,
            self.gas,
            self.profile.limits,
            Some(self.transport),
        )
    }
    pub fn initialize(&self, state: &mut [CellInventory]) -> Result<(), PhysicsError> {
        if state.len() as u64 != self.mesh.memory().cells {
            return Err(PhysicsError::BufferLength);
        }
        for (region, value) in self.mesh.regions().iter().zip(&self.initial) {
            let cell = CellInventory::from_density(*value, region.grid.volume()?)?;
            state[region.offset..region.offset + region.grid.count()].fill(cell);
        }
        Ok(())
    }
    pub fn readings(
        &self,
        state: CellInventory,
        volume: Scalar,
    ) -> Result<[Scalar; 2], PhysicsError> {
        readings(self.scales, self.gas.primitive(state.density(volume)?)?)
    }
    fn hash(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(CONNECTED_GAS_MODEL_CONTRACT);
        hash.update(CONNECTED_FLOW_CONTRACT);
        hash.update(NUMERICAL_CONTRACT);
        hash.update(self.mesh.digest());
        let p = self.profile;
        for value in [
            p.reference.length_m,
            p.reference.velocity_m_s,
            p.reference.density_kg_m3,
            p.reference.gas_constant_j_kg_k,
            p.heat_capacity_ratio,
            p.dynamic_viscosity_pa_s,
            p.thermal_conductivity_w_m_k,
            p.limits.residual,
            self.solver_dt,
        ] {
            hash.update(value.raw().to_be_bytes());
        }
        hash.update(p.timestep_us.to_be_bytes());
        hash.update(p.steps.to_be_bytes());
        hash.update(p.limits.iterations.to_be_bytes());
        for value in &self.initial {
            for component in value.components() {
                hash.update(component.raw().to_be_bytes());
            }
        }
        if let Some(policy) = self.retries {
            hash.update(CONNECTED_ADVANCE_CONTRACT);
            hash.update(policy.maximum_substeps.to_be_bytes());
            hash.update(policy.minimum_step.raw().to_be_bytes());
        }
        crate::source::sha256_hex(hash.finalize().as_slice())
    }
}

fn normalize(state: Primitive, scales: GasScales) -> Result<Primitive, PhysicsError> {
    let v = state.velocity.map(|value| scales.velocity_to_solver(value));
    Ok(Primitive {
        density: scales.density_to_solver(state.density)?,
        pressure: scales.pressure_to_solver(state.pressure)?,
        velocity: [v[0]?, v[1]?, v[2]?],
    })
}
fn readings(scales: GasScales, state: Primitive) -> Result<[Scalar; 2], PhysicsError> {
    let values = [
        scales.pressure_to_si(state.pressure)?,
        scales.temperature_to_si(state.pressure.checked_div(state.density)?)?,
    ];
    if values.iter().any(|v| *v <= Scalar::ZERO) {
        return Err(PhysicsError::InvalidState);
    }
    Ok(values)
}
