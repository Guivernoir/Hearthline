use hearthline_config::physics::material::CaloricSolverConfig;
use hearthline_physics::material::caloric::{CaloricKnot, CaloricLaw};
use hearthline_physics::transport::enthalpy::EnthalpyLimits;
use hearthline_physics::{PhysicsError, Scalar};

use super::super::{error, positive};

#[derive(Clone, Debug)]
pub struct CompiledCaloricMaterial {
    knots: Vec<CaloricKnot>,
    limits: EnthalpyLimits,
    liquid_knots: Option<Vec<CaloricKnot>>,
}

impl CompiledCaloricMaterial {
    pub(super) fn compile(
        config: &CaloricSolverConfig,
        capacities: &[[String; 2]],
        liquid_capacities: Option<&[[String; 2]]>,
        temperature_range: [Scalar; 2],
        reference_temperature: Scalar,
        reference_capacity: Scalar,
    ) -> Result<Self, String> {
        let limits = EnthalpyLimits {
            sweeps: config.maximum_sweeps,
            local_iterations: config.maximum_local_iterations,
            residual: positive(&config.residual_tolerance_j)?,
        };
        if limits.sweeps == 0 || limits.local_iterations == 0 {
            return Err("caloric solver iteration limits must be positive".into());
        }
        let knots = compile_knots(capacities, temperature_range)?;
        let law = CaloricLaw::new(&knots).map_err(error)?;
        if law.heat_capacity(reference_temperature).map_err(error)? != reference_capacity {
            return Err(
                "caloric heat capacity at the initial temperature disagrees with the proof profile"
                    .into(),
            );
        }
        let liquid_knots = liquid_capacities
            .map(|capacities| compile_knots(capacities, temperature_range))
            .transpose()?;
        Ok(Self {
            knots,
            limits,
            liquid_knots,
        })
    }

    pub fn law(&self) -> Result<CaloricLaw<'_>, PhysicsError> {
        CaloricLaw::new(&self.knots)
    }
    pub const fn limits(&self) -> EnthalpyLimits {
        self.limits
    }
    pub fn liquid_law(&self) -> Result<Option<CaloricLaw<'_>>, PhysicsError> {
        self.liquid_knots
            .as_ref()
            .map(|knots| CaloricLaw::new(knots))
            .transpose()
    }
    pub const fn has_liquid(&self) -> bool {
        self.liquid_knots.is_some()
    }
}

fn compile_knots(
    capacities: &[[String; 2]],
    temperature_range: [Scalar; 2],
) -> Result<Vec<CaloricKnot>, String> {
    let mut points = Vec::new();
    points
        .try_reserve_exact(capacities.len())
        .map_err(|e| format!("caloric compiler allocation: {e}"))?;
    for [temperature, capacity] in capacities {
        points.push([positive(temperature)?, positive(capacity)?]);
    }
    let mut knots = Vec::new();
    knots
        .try_reserve_exact(points.len())
        .map_err(|e| format!("caloric material allocation: {e}"))?;
    knots.resize(points.len(), CaloricKnot::default());
    let law = CaloricLaw::compile(&points, Scalar::ZERO, &mut knots).map_err(error)?;
    if law.domain() != temperature_range {
        return Err(
            "caloric table endpoints must match the material temperature validity interval".into(),
        );
    }
    Ok(knots)
}
