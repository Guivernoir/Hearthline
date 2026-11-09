use hearthline_physics::Scalar;
use hearthline_physics::mechanics::MechanicsReport;
use hearthline_project::physics::CompiledSpecimenMechanics;

use super::{MechanicalCheckpoint, SpecimenMechanics, eigenstrain};

#[cfg(test)]
mod tests;

impl SpecimenMechanics {
    pub(super) fn initial_trial(&mut self) -> MechanicsReport {
        self.displacement_trial.fill(Scalar::ZERO);
        self.stress_trial.fill(Scalar::ZERO);
        MechanicsReport {
            iterations: 0,
            residual: Scalar::ZERO,
        }
    }

    /// A zero-start snapshot must describe the unique *numerical* solve, not
    /// merely a displacement/stress pair that is mutually consistent.
    pub(super) fn validate_reconstructed(
        &mut self,
        model: &CompiledSpecimenMechanics,
        checkpoint: &MechanicalCheckpoint,
        moisture: &[i64],
        temperature: &[i64],
        reference: [Scalar; 2],
        step: u32,
    ) -> Result<MechanicsReport, String> {
        if moisture.len().checked_mul(6) != Some(self.eigenstrain.len())
            || temperature.len() != moisture.len()
        {
            return Err("invalid reconstructible mechanical field shape".into());
        }
        let report = if step == 0 {
            if moisture.iter().any(|v| *v != reference[0].raw())
                || temperature.iter().any(|v| *v != reference[1].raw())
            {
                return Err("zero-start initial fields differ from the model".into());
            }
            self.initial_trial()
        } else {
            for (cell, (&moisture, &temperature)) in moisture.iter().zip(temperature).enumerate() {
                let strain = eigenstrain(
                    model,
                    Scalar::from_raw(moisture),
                    Scalar::from_raw(temperature),
                    reference[0],
                    reference[1],
                )
                .map_err(|e| e.to_string())?;
                self.eigenstrain[6 * cell..6 * cell + 6].fill(strain);
            }
            self.solve_trial(model).map_err(|e| e.to_string())?
        };
        if report.iterations != checkpoint.iterations
            || report.residual.raw() != checkpoint.residual_raw
            || self
                .displacement_trial
                .iter()
                .map(|v| v.raw())
                .ne(checkpoint.displacement_raw.iter().copied())
            || self
                .stress_trial
                .iter()
                .map(|v| v.raw())
                .ne(checkpoint.stress_raw.iter().copied())
        {
            return Err("mechanical checkpoint differs from its zero-start reconstruction".into());
        }
        Ok(report)
    }
}
