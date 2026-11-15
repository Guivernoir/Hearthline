use hearthline_config::physics::CompressibleReceiverConfig;
use hearthline_physics::flow::{
    CompressibleFlow, Conserved, FlowBoundary, FlowRetryPolicy, GasScales, IdealGas, Primitive,
    ViscousTransport,
};
use hearthline_physics::transport::{Grid3d, SolverLimits};
use hearthline_physics::{PhysicsError, Scalar};

use super::{error, nonnegative, positive};

pub mod connected;
pub mod mesh;
pub mod source;

#[derive(Clone, Debug)]
pub struct CompiledReceiverFlow {
    solver: CompressibleFlow,
    gas: IdealGas,
    scales: GasScales,
    upstream: Conserved,
    receiver: Conserved,
    receiver_start_x: usize,
    solver_timestep_us: u64,
    retries: FlowRetryPolicy,
}

impl CompiledReceiverFlow {
    pub(super) fn compile(
        config: &CompressibleReceiverConfig,
        grid: Grid3d,
        timestep: Scalar,
        limits: SolverLimits,
    ) -> Result<Self, String> {
        let scales = GasScales::new(
            positive(&config.length_scale_m)?,
            positive(&config.velocity_scale_m_s)?,
            positive(&config.density_scale_kg_m3)?,
            positive(&config.gas_constant_j_kg_k)?,
        )
        .map_err(error)?;
        let spacing = grid.spacing().map(|v| scales.length_to_solver(v));
        let solver_grid = Grid3d::new(
            grid.shape(),
            [
                spacing[0].map_err(error)?,
                spacing[1].map_err(error)?,
                spacing[2].map_err(error)?,
            ],
        )
        .map_err(error)?;
        let solver_timestep = scales.time_to_solver(timestep).map_err(error)?;
        if solver_timestep <= Scalar::ZERO || solver_timestep.raw() % 1000 != 0 {
            return Err(
                "normalized timestep must be a positive whole number of solver microseconds".into(),
            );
        }
        let solver_timestep_us = (solver_timestep.raw() / 1000) as u64;
        if config.maximum_substeps == 0
            || !config.maximum_substeps.is_power_of_two()
            || config.minimum_solver_step_us == 0
            || config.minimum_solver_step_us > solver_timestep_us
        {
            return Err("invalid bounded flow retry policy".into());
        }
        let receiver_start_x =
            usize::try_from(config.receiver_start_x).map_err(|e| e.to_string())?;
        if receiver_start_x == 0 || receiver_start_x >= grid.shape()[0] {
            return Err("receiver interface must be inside the X extent".into());
        }
        let gas = IdealGas::new(positive(&config.heat_capacity_ratio)?).map_err(error)?;
        let temperature = scales
            .temperature_to_solver(positive(&config.initial_temperature_k)?)
            .map_err(error)?;
        let initial = |pressure: &str| -> Result<Conserved, String> {
            let pressure = scales
                .pressure_to_solver(positive(pressure)?)
                .map_err(error)?;
            let density = pressure.checked_div(temperature).map_err(error)?;
            gas.conserved(Primitive {
                density,
                velocity: [Scalar::ZERO; 3],
                pressure,
            })
            .map_err(error)
        };
        let upstream = initial(&config.upstream_pressure_pa)?;
        let receiver = initial(&config.receiver_pressure_pa)?;
        let viscosity = nonnegative(&config.dynamic_viscosity_pa_s)?;
        let conductivity = nonnegative(&config.thermal_conductivity_w_m_k)?;
        let normalized_viscosity = scales.viscosity_to_solver(viscosity).map_err(error)?;
        let normalized_conductivity = scales.conductivity_to_solver(conductivity).map_err(error)?;
        if (viscosity > Scalar::ZERO && normalized_viscosity == Scalar::ZERO)
            || (conductivity > Scalar::ZERO && normalized_conductivity == Scalar::ZERO)
        {
            return Err("flow reference scales erase a positive transport coefficient".into());
        }
        let transport =
            ViscousTransport::new(normalized_viscosity, normalized_conductivity, Scalar::ONE)
                .map_err(error)?;
        let wall_temperature = config
            .wall_temperature_k
            .as_deref()
            .map(|t| scales.temperature_to_solver(positive(t)?).map_err(error))
            .transpose()?;
        let boundaries = [FlowBoundary::NoSlipWall {
            velocity: [Scalar::ZERO; 3],
            temperature: wall_temperature,
        }; 6];
        let solver = CompressibleFlow::new_viscous(solver_grid, gas, boundaries, limits, transport)
            .map_err(error)?;
        Ok(Self {
            solver,
            gas,
            scales,
            upstream,
            receiver,
            receiver_start_x,
            solver_timestep_us,
            retries: FlowRetryPolicy {
                maximum_substeps: config.maximum_substeps,
                minimum_step_us: config.minimum_solver_step_us,
            },
        })
    }

    pub fn solver(&self) -> &CompressibleFlow {
        &self.solver
    }
    pub const fn scales(&self) -> GasScales {
        self.scales
    }
    pub const fn gas(&self) -> IdealGas {
        self.gas
    }
    pub const fn upstream(&self) -> Conserved {
        self.upstream
    }
    pub const fn receiver(&self) -> Conserved {
        self.receiver
    }
    pub const fn receiver_start_x(&self) -> usize {
        self.receiver_start_x
    }
    pub const fn solver_timestep_us(&self) -> u64 {
        self.solver_timestep_us
    }
    pub const fn retries(&self) -> FlowRetryPolicy {
        self.retries
    }
    pub fn readings(&self, state: Conserved) -> Result<[Scalar; 2], PhysicsError> {
        let state = self.gas.primitive(state)?;
        let temperature = state.pressure.checked_div(state.density)?;
        let values = [
            self.scales.pressure_to_si(state.pressure)?,
            self.scales.temperature_to_si(temperature)?,
        ];
        if values.iter().any(|value| *value <= Scalar::ZERO) {
            return Err(PhysicsError::InvalidState);
        }
        Ok(values)
    }
}
