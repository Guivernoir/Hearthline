use crate::material::caloric::CaloricLaw;
use crate::transport::enthalpy::{EnthalpySystem, EnthalpyWorkspace, WaterTransport};
use crate::transport::nonlinear::{NonlinearDiffusion, NonlinearReport, NonlinearWorkspace};
use crate::transport::{Boundary, SolveReport, TransportSystem};
use crate::{PhysicsError, Scalar};

use super::DryingWorkspace;

pub struct VariableDryingWorkspace<'a> {
    pub drying: DryingWorkspace<'a>,
    pub nonlinear_trial: &'a mut [Scalar],
    pub cell_conductance: &'a mut [[Scalar; 3]],
    pub moisture_boundary: &'a mut [Boundary],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VariableDryingReport {
    pub moisture: NonlinearReport,
    pub thermal: SolveReport,
    pub water_removed_kg: Scalar,
    pub thermal_local_iterations: Option<u64>,
    /// Integrated latent source minus latent heat times the water exchange.
    pub latent_exchange_error: Scalar,
}

pub enum ThermalSystem<'a> {
    Constant(&'a TransportSystem<'a>),
    Caloric(&'a EnthalpySystem<'a>),
    Wet {
        system: &'a EnthalpySystem<'a>,
        liquid: CaloricLaw<'a>,
        inlet_temperature: Scalar,
    },
}

/// Moisture-dependent diffusion followed by latent-heat-coupled thermal
/// diffusion. Both equations must use compatible mass/energy/time scales.
/// Input fields remain authoritative until the caller commits both trials.
/// Thermal storage is constant here; this is not a varying-enthalpy model.
pub fn trial_step(
    moisture_system: &NonlinearDiffusion<'_>,
    thermal_system: &TransportSystem<'_>,
    dt: Scalar,
    latent_heat_j_kg: Scalar,
    moisture: &[Scalar],
    temperature_k: &[Scalar],
    work: &mut VariableDryingWorkspace<'_>,
) -> Result<VariableDryingReport, PhysicsError> {
    trial_step_with_thermal(
        moisture_system,
        ThermalSystem::Constant(thermal_system),
        dt,
        latent_heat_j_kg,
        moisture,
        temperature_k,
        work,
    )
}

pub fn trial_step_with_thermal(
    moisture_system: &NonlinearDiffusion<'_>,
    thermal_system: ThermalSystem<'_>,
    dt: Scalar,
    latent_heat_j_kg: Scalar,
    moisture: &[Scalar],
    temperature_k: &[Scalar],
    work: &mut VariableDryingWorkspace<'_>,
) -> Result<VariableDryingReport, PhysicsError> {
    let n = moisture.len();
    if [
        temperature_k.len(),
        work.drying.moisture_trial.len(),
        work.drying.temperature_trial.len(),
        work.drying.linear.len(),
        work.drying.heat_sources.len(),
        work.nonlinear_trial.len(),
        work.cell_conductance.len(),
        work.moisture_boundary.len(),
    ]
    .iter()
    .any(|len| *len != n)
    {
        return Err(PhysicsError::BufferLength);
    }
    if latent_heat_j_kg < Scalar::ZERO
        || moisture.iter().any(|v| *v < Scalar::ZERO)
        || temperature_k.iter().any(|v| *v <= Scalar::ZERO)
    {
        return Err(PhysicsError::InvalidState);
    }
    work.drying.moisture_trial.copy_from_slice(moisture);
    work.drying.temperature_trial.copy_from_slice(temperature_k);
    work.drying.heat_sources.fill(Scalar::ZERO);
    let moisture_report = moisture_system.solve(
        dt,
        work.drying.moisture_trial,
        work.drying.heat_sources,
        &mut NonlinearWorkspace {
            trial: work.nonlinear_trial,
            linear: work.drying.linear,
            cell_conductance: work.cell_conductance,
            boundary: work.moisture_boundary,
        },
    )?;
    for ((boundary, &value), source) in work
        .moisture_boundary
        .iter()
        .zip(work.drying.moisture_trial.iter())
        .zip(work.drying.heat_sources.iter_mut())
    {
        *source = boundary.rate(value)?.checked_mul(latent_heat_j_kg)?;
    }
    let latent_source = work
        .drying
        .heat_sources
        .iter()
        .try_fold(Scalar::ZERO, |sum, &source| {
            sum.checked_add(dt.checked_mul(source)?)
        })?;
    let latent_exchange_error = latent_source
        .checked_sub(latent_heat_j_kg.checked_mul(moisture_report.transport.exchange)?)?;
    let (thermal_report, thermal_local_iterations) = match thermal_system {
        ThermalSystem::Constant(system) => (
            system.solve(
                dt,
                work.drying.temperature_trial,
                work.drying.heat_sources,
                work.drying.linear,
            )?,
            None,
        ),
        ThermalSystem::Caloric(system) => {
            let report = system.solve(
                dt,
                work.drying.temperature_trial,
                work.drying.heat_sources,
                &mut EnthalpyWorkspace {
                    trial: work.drying.linear,
                    old_enthalpy: work.nonlinear_trial,
                },
            )?;
            (report.transport, Some(report.local_iterations))
        }
        ThermalSystem::Wet {
            system,
            liquid,
            inlet_temperature,
        } => {
            let wet = system.with_water(WaterTransport {
                liquid,
                previous_moisture: moisture,
                current_moisture: work.drying.moisture_trial,
                conductance: work.cell_conductance,
                boundary: work.moisture_boundary,
                inlet_temperature,
            })?;
            let report = wet.solve(
                dt,
                work.drying.temperature_trial,
                work.drying.heat_sources,
                &mut EnthalpyWorkspace {
                    trial: work.drying.linear,
                    old_enthalpy: work.nonlinear_trial,
                },
            )?;
            (report.transport, Some(report.local_iterations))
        }
    };
    if work.drying.moisture_trial.iter().any(|v| *v < Scalar::ZERO)
        || work
            .drying
            .temperature_trial
            .iter()
            .any(|v| *v <= Scalar::ZERO)
    {
        return Err(PhysicsError::InvalidState);
    }
    Ok(VariableDryingReport {
        moisture: moisture_report,
        thermal: thermal_report,
        water_removed_kg: moisture_report.transport.exchange.checked_neg()?,
        thermal_local_iterations,
        latent_exchange_error,
    })
}
