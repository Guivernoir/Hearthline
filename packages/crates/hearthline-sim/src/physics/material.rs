use hearthline_physics::drying::{DryingWorkspace, variable};
use hearthline_physics::transport::enthalpy::EnthalpySystem;
use hearthline_physics::transport::nonlinear::{
    NonlinearDiffusion, NonlinearReport, SurfaceBoundary,
};
use hearthline_physics::transport::{Boundary, TransportSystem, surface_conductance};
use hearthline_physics::{PhysicsError, Scalar};
use hearthline_project::physics::{CompiledPhysicsProof, CompiledProofCase};
use serde::{Deserialize, Serialize};

use super::{buffers::TransportBuffers, field};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffusionCheckpoint {
    pub iterations: u32,
    pub linear_sweeps: u64,
    pub residual_raw: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thermal: Option<ThermalCheckpoint>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThermalCheckpoint {
    pub iterations: u32,
    pub local_iterations: u64,
    pub residual_raw: i64,
    pub latent_exchange_error_raw: i64,
}

impl From<NonlinearReport> for DiffusionCheckpoint {
    fn from(report: NonlinearReport) -> Self {
        Self {
            iterations: report.iterations,
            linear_sweeps: report.linear_sweeps,
            residual_raw: report.transport.residual.raw(),
            thermal: None,
        }
    }
}

impl From<variable::VariableDryingReport> for DiffusionCheckpoint {
    fn from(report: variable::VariableDryingReport) -> Self {
        let mut checkpoint: Self = report.moisture.into();
        checkpoint.thermal =
            report
                .thermal_local_iterations
                .map(|local_iterations| ThermalCheckpoint {
                    iterations: report.thermal.iterations,
                    local_iterations,
                    residual_raw: report.thermal.residual.raw(),
                    latent_exchange_error_raw: report.latent_exchange_error.raw(),
                });
        checkpoint
    }
}

pub(super) struct VariableDryingBuffers {
    trial: Vec<Scalar>,
    conductance: Vec<[Scalar; 3]>,
}

impl VariableDryingBuffers {
    pub fn new(n: usize) -> Result<Self, String> {
        Ok(Self {
            trial: field(n, Scalar::ZERO)?,
            conductance: field(n, [Scalar::ZERO; 3])?,
        })
    }

    pub fn trial(
        &mut self,
        model: &CompiledPhysicsProof,
        buffers: &mut TransportBuffers,
        fields: [&[Scalar]; 2],
        trials: [&mut [Scalar]; 2],
    ) -> Result<variable::VariableDryingReport, PhysicsError> {
        let material = model.material().ok_or(PhysicsError::InvalidState)?;
        let CompiledProofCase::Drying {
            latent_heat,
            surface_transfer,
            density,
            equilibrium,
            ambient,
            ..
        } = model.case()
        else {
            return Err(PhysicsError::InvalidState);
        };
        let grid = model.grid();
        let mut film = [Scalar::ZERO; 3];
        for (coefficient, spacing) in film.iter_mut().zip(grid.spacing()) {
            *coefficient = surface_transfer
                .checked_mul(density)?
                .checked_mul(grid.volume()?.checked_div(spacing)?)?;
            if surface_transfer > Scalar::ZERO && *coefficient == Scalar::ZERO {
                return Err(PhysicsError::InsufficientResolution);
            }
        }
        let moisture = NonlinearDiffusion::new(
            model.transport_grid()?,
            &buffers.storage,
            buffers.conductance,
            material.multiplier()?,
            SurfaceBoundary::Film {
                conductance: film,
                ambient: equilibrium,
            },
            material.limits(),
        )?;
        let thermal = TransportSystem::on_grid(
            model.transport_grid()?,
            &buffers.thermal_storage,
            buffers.thermal_conductance,
            &buffers.thermal_boundary,
            model.limits(),
        )?;
        let caloric = material
            .caloric()
            .map(|caloric| {
                EnthalpySystem::new(
                    model.transport_grid()?,
                    &buffers.storage,
                    caloric.law()?,
                    buffers.thermal_conductance,
                    &buffers.thermal_boundary,
                    caloric.limits(),
                )
            })
            .transpose()?;
        let [moisture_trial, temperature_trial] = trials;
        let mut work = variable::VariableDryingWorkspace {
            drying: DryingWorkspace {
                moisture_trial,
                temperature_trial,
                linear: &mut buffers.linear,
                heat_sources: &mut buffers.heat_sources,
            },
            nonlinear_trial: &mut self.trial,
            cell_conductance: &mut self.conductance,
            moisture_boundary: &mut buffers.boundary,
        };
        let liquid = material
            .caloric()
            .map(|caloric| caloric.liquid_law())
            .transpose()?
            .flatten();
        let thermal_system = match (caloric.as_ref(), liquid) {
            (Some(system), Some(liquid)) => variable::ThermalSystem::Wet {
                system,
                liquid,
                inlet_temperature: ambient,
            },
            (Some(system), None) => variable::ThermalSystem::Caloric(system),
            (None, None) => variable::ThermalSystem::Constant(&thermal),
            _ => return Err(PhysicsError::InvalidState),
        };
        let report = variable::trial_step_with_thermal(
            &moisture,
            thermal_system,
            model.timestep(),
            latent_heat,
            fields[0],
            fields[1],
            &mut work,
        )?;
        for (&moisture, &temperature) in work
            .drying
            .moisture_trial
            .iter()
            .zip(work.drying.temperature_trial.iter())
        {
            material.validate_state(moisture, temperature)?;
        }
        Ok(report)
    }
}

pub(super) fn validate_checkpoint(
    model: &CompiledPhysicsProof,
    checkpoint: &super::PhysicsCheckpoint,
) -> Result<(), String> {
    let material = model.material();
    if checkpoint.diffusion.is_some() != (material.is_some() && checkpoint.step > 0) {
        return Err("physics checkpoint diffusion model mismatch".into());
    }
    if let Some(material) = material {
        for (&moisture, &temperature) in checkpoint
            .first_field_raw
            .iter()
            .zip(&checkpoint.temperature_k_raw)
        {
            material
                .validate_state(Scalar::from_raw(moisture), Scalar::from_raw(temperature))
                .map_err(|e| format!("physics checkpoint material domain: {e}"))?;
        }
        if let Some(report) = checkpoint.diffusion {
            let limits = material.limits();
            if report.iterations == 0
                || report.iterations > limits.iterations
                || report.residual_raw < 0
                || report.residual_raw > limits.residual.raw()
                || report.linear_sweeps < u64::from(report.iterations)
                || report.linear_sweeps
                    > u64::from(report.iterations) * u64::from(limits.linear.iterations)
            {
                return Err("physics checkpoint invalid nonlinear solver report".into());
            }
            if report.thermal.is_some() != material.caloric().is_some() {
                return Err("physics checkpoint caloric model mismatch".into());
            }
            if let (Some(thermal), Some(caloric)) = (report.thermal, material.caloric()) {
                let limits = caloric.limits();
                let visits = u64::from(thermal.iterations).checked_mul(model.cell_count() as u64);
                let maximum =
                    visits.and_then(|n| n.checked_mul(u64::from(limits.local_iterations)));
                if thermal.iterations == 0
                    || thermal.iterations > limits.sweeps
                    || thermal.residual_raw < 0
                    || thermal.residual_raw > limits.residual.raw()
                    || visits.is_none_or(|n| thermal.local_iterations < n)
                    || maximum.is_none_or(|n| thermal.local_iterations > n)
                {
                    return Err("physics checkpoint invalid caloric solver report".into());
                }
            }
        }
    }
    Ok(())
}

pub(super) fn initialize_caloric_boundary(
    model: &CompiledPhysicsProof,
    buffers: &mut TransportBuffers,
) -> Result<(), PhysicsError> {
    if !model
        .material()
        .is_some_and(|material| material.caloric().is_some())
    {
        return Ok(());
    }
    let CompiledProofCase::Drying {
        heat_transfer,
        ambient,
        ..
    } = model.case()
    else {
        return Err(PhysicsError::InvalidState);
    };
    let grid = model.grid();
    let shape = grid.shape();
    let mut face_conductance = [Scalar::ZERO; 3];
    for (axis, coefficient) in face_conductance.iter_mut().enumerate() {
        let film = grid
            .volume()?
            .checked_div(grid.spacing()[axis])?
            .checked_mul(heat_transfer)?;
        if heat_transfer > Scalar::ZERO && film == Scalar::ZERO {
            return Err(PhysicsError::InsufficientResolution);
        }
        *coefficient = surface_conductance(
            film,
            buffers.thermal_conductance[axis].checked_mul(Scalar::integer(2)?)?,
        )?;
    }
    for (cell, boundary) in buffers.thermal_boundary.iter_mut().enumerate() {
        let coordinates = [
            cell % shape[0],
            (cell / shape[0]) % shape[1],
            cell / (shape[0] * shape[1]),
        ];
        *boundary = Boundary {
            conductance: Scalar::ZERO,
            ambient,
        };
        for (axis, &conductance) in face_conductance.iter().enumerate() {
            let faces = if let Some(domain) = model.solid_domain() {
                let faces = domain
                    .exposed_faces(cell)
                    .ok_or(PhysicsError::InvalidGeometry)?;
                i64::from(faces[2 * axis]) + i64::from(faces[2 * axis + 1])
            } else {
                i64::from(coordinates[axis] == 0) + i64::from(coordinates[axis] + 1 == shape[axis])
            };
            boundary.conductance = boundary
                .conductance
                .checked_add(conductance.checked_mul(Scalar::integer(faces)?)?)?;
        }
    }
    Ok(())
}

pub(super) fn first_report_difference(
    expected: Option<DiffusionCheckpoint>,
    actual: Option<DiffusionCheckpoint>,
) -> Option<&'static str> {
    let Some((expected, actual)) = expected.zip(actual) else {
        return (expected != actual).then_some("diffusion");
    };
    for (field, differs) in [
        (
            "diffusion.iterations",
            expected.iterations != actual.iterations,
        ),
        (
            "diffusion.linear_sweeps",
            expected.linear_sweeps != actual.linear_sweeps,
        ),
        (
            "diffusion.residual_raw",
            expected.residual_raw != actual.residual_raw,
        ),
    ] {
        if differs {
            return Some(field);
        }
    }
    let Some((expected_thermal, actual_thermal)) = expected.thermal.zip(actual.thermal) else {
        return (expected.thermal != actual.thermal).then_some("diffusion.thermal");
    };
    [
        (
            "diffusion.thermal.iterations",
            expected_thermal.iterations != actual_thermal.iterations,
        ),
        (
            "diffusion.thermal.local_iterations",
            expected_thermal.local_iterations != actual_thermal.local_iterations,
        ),
        (
            "diffusion.thermal.residual_raw",
            expected_thermal.residual_raw != actual_thermal.residual_raw,
        ),
        (
            "diffusion.thermal.latent_exchange_error_raw",
            expected_thermal.latent_exchange_error_raw != actual_thermal.latent_exchange_error_raw,
        ),
    ]
    .into_iter()
    .find_map(|(field, differs)| differs.then_some(field))
}
