use crate::transport::{SolveReport, TransportSystem};
use crate::{PhysicsError, Scalar};

pub mod variable;

/// Host-owned storage reused for every coupled step, including rollback.
pub struct DryingWorkspace<'a> {
    pub moisture_trial: &'a mut [Scalar],
    pub temperature_trial: &'a mut [Scalar],
    pub linear: &'a mut [Scalar],
    pub heat_sources: &'a mut [Scalar],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DryingReport {
    pub moisture: SolveReport,
    pub thermal: SolveReport,
    /// Positive for evaporated water, negative for condensation.
    pub water_removed_kg: Scalar,
}

/// Constant-property diffusion specimen with latent heat coupling. Moisture is
/// kg water / kg dry solid; its storage coefficient is dry mass per cell.
/// Thermal storage is J/K and its transported field is absolute temperature.
/// Sorption, shrinkage, mechanics, and fracture remain outside this proof.
pub fn trial_step(
    moisture_system: &TransportSystem<'_>,
    thermal_system: &TransportSystem<'_>,
    dt: Scalar,
    latent_heat_j_kg: Scalar,
    moisture: &[Scalar],
    temperature_k: &[Scalar],
    workspace: &mut DryingWorkspace<'_>,
) -> Result<DryingReport, PhysicsError> {
    let n = moisture.len();
    if temperature_k.len() != n
        || workspace.moisture_trial.len() != n
        || workspace.temperature_trial.len() != n
        || workspace.linear.len() != n
        || workspace.heat_sources.len() != n
    {
        return Err(PhysicsError::BufferLength);
    }
    if latent_heat_j_kg < Scalar::ZERO
        || moisture.iter().any(|v| *v < Scalar::ZERO)
        || temperature_k.iter().any(|v| *v <= Scalar::ZERO)
    {
        return Err(PhysicsError::InvalidState);
    }
    workspace.moisture_trial.copy_from_slice(moisture);
    workspace.temperature_trial.copy_from_slice(temperature_k);
    workspace.heat_sources.fill(Scalar::ZERO);
    let moisture_report = moisture_system.solve(
        dt,
        workspace.moisture_trial,
        workspace.heat_sources,
        workspace.linear,
    )?;
    moisture_system.boundary_rates(workspace.moisture_trial, workspace.heat_sources)?;
    for source in workspace.heat_sources.iter_mut() {
        *source = source.checked_mul(latent_heat_j_kg)?;
    }
    let thermal_report = thermal_system.solve(
        dt,
        workspace.temperature_trial,
        workspace.heat_sources,
        workspace.linear,
    )?;
    if workspace.moisture_trial.iter().any(|v| *v < Scalar::ZERO)
        || workspace
            .temperature_trial
            .iter()
            .any(|v| *v <= Scalar::ZERO)
    {
        return Err(PhysicsError::InvalidState);
    }
    let report = DryingReport {
        moisture: moisture_report,
        thermal: thermal_report,
        water_removed_kg: moisture_report.exchange.checked_neg()?,
    };
    Ok(report)
}
