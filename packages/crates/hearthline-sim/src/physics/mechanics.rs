use hearthline_physics::mechanics::{MechanicalSystem, MechanicsReport, MechanicsWorkspace};
use hearthline_physics::{PhysicsError, Scalar};
use hearthline_project::physics::CompiledSpecimenMechanics;
use serde::{Deserialize, Serialize};

use super::field;

mod reconstruction;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MechanicalCheckpoint {
    /// Node-major xyz, divided by the compiled reference length.
    pub displacement_raw: Vec<i64>,
    /// Element-major xx, yy, zz, yz, xz, xy, divided by reference stress.
    pub stress_raw: Vec<i64>,
    pub iterations: u32,
    pub residual_raw: i64,
}

pub(super) struct SpecimenMechanics {
    displacement: Vec<Scalar>,
    stress: Vec<Scalar>,
    displacement_trial: Vec<Scalar>,
    stress_trial: Vec<Scalar>,
    eigenstrain: Vec<Scalar>,
    external: Vec<Scalar>,
    work: [Vec<Scalar>; 6],
    report: MechanicsReport,
    reconstructible: bool,
}

impl SpecimenMechanics {
    pub(super) fn save_population_state(
        &self,
        output: &mut [Scalar],
    ) -> Result<MechanicsReport, PhysicsError> {
        if self.reconstructible {
            if !output.is_empty() {
                return Err(PhysicsError::BufferLength);
            }
        } else if self.displacement.len() != output.len() {
            return Err(PhysicsError::BufferLength);
        } else {
            output.copy_from_slice(&self.displacement);
        }
        Ok(self.report)
    }

    /// Internal state-bank transfer. External snapshots use validate_restore.
    pub(super) fn load_population_state(
        &mut self,
        model: &CompiledSpecimenMechanics,
        input: &[Scalar],
        report: MechanicsReport,
        fields: [&[Scalar]; 2],
        reference: [Scalar; 2],
        step: u32,
    ) -> Result<(), PhysicsError> {
        let [moisture, temperature] = fields;
        if self.reconstructible {
            if !input.is_empty() {
                return Err(PhysicsError::BufferLength);
            }
            self.update_eigenstrain(model, moisture, temperature, reference)?;
            let reconstructed = if step == 0 {
                if moisture.iter().any(|v| *v != reference[0])
                    || temperature.iter().any(|v| *v != reference[1])
                {
                    return Err(PhysicsError::InvalidState);
                }
                self.initial_trial()
            } else {
                self.solve_trial(model)?
            };
            if reconstructed != report {
                return Err(PhysicsError::InvalidState);
            }
            self.commit(reconstructed);
            return Ok(());
        }
        if self.displacement.len() != input.len() {
            return Err(PhysicsError::BufferLength);
        }
        self.displacement.copy_from_slice(input);
        self.update_eigenstrain(model, moisture, temperature, reference)?;
        recover_stress(
            model,
            &self.displacement,
            &self.eigenstrain,
            &mut self.stress,
        )?;
        self.report = report;
        Ok(())
    }

    pub(super) fn displacement(&self) -> &[Scalar] {
        &self.displacement
    }
    pub(super) fn stress(&self) -> &[Scalar] {
        &self.stress
    }
    pub(super) const fn report(&self) -> MechanicsReport {
        self.report
    }

    pub fn new(model: &CompiledSpecimenMechanics) -> Result<Self, String> {
        let dofs = model.constraints().len();
        let elements = model.elements().len();
        Ok(Self {
            displacement: field(dofs, Scalar::ZERO)?,
            displacement_trial: field(dofs, Scalar::ZERO)?,
            stress: field(elements * 6, Scalar::ZERO)?,
            stress_trial: field(elements * 6, Scalar::ZERO)?,
            eigenstrain: field(elements, Scalar::ZERO)?,
            external: field(dofs, Scalar::ZERO)?,
            work: [
                field(dofs, Scalar::ZERO)?,
                field(dofs, Scalar::ZERO)?,
                field(dofs, Scalar::ZERO)?,
                field(dofs, Scalar::ZERO)?,
                field(dofs, Scalar::ZERO)?,
                field(dofs, Scalar::ZERO)?,
            ],
            report: MechanicsReport {
                iterations: 0,
                residual: Scalar::ZERO,
            },
            reconstructible: model.reconstructible(),
        })
    }

    pub fn trial(
        &mut self,
        model: &CompiledSpecimenMechanics,
        moisture: &[Scalar],
        temperature: &[Scalar],
        reference_moisture: Scalar,
        reference_temperature: Scalar,
    ) -> Result<MechanicsReport, PhysicsError> {
        self.update_eigenstrain(
            model,
            moisture,
            temperature,
            [reference_moisture, reference_temperature],
        )?;
        self.solve_trial(model)
    }

    fn solve_trial(
        &mut self,
        model: &CompiledSpecimenMechanics,
    ) -> Result<MechanicsReport, PhysicsError> {
        if self.reconstructible {
            self.displacement_trial.fill(Scalar::ZERO);
        } else {
            self.displacement_trial.copy_from_slice(&self.displacement);
        }
        let mut system = MechanicalSystem::new(
            model.elements(),
            model.materials(),
            model.constraints(),
            model.limits(),
        )?;
        if self.reconstructible {
            system = system.with_normalized_preconditioner();
        }
        if let Some(stiffness) = model.stiffness()? {
            system = system.with_stiffness(stiffness)?;
            if self.reconstructible {
                system = system.with_symmetric_sweep()?;
            }
        }
        let [
            trial,
            residual,
            preconditioned,
            direction,
            product,
            diagonal,
        ] = &mut self.work;
        let mut workspace = MechanicsWorkspace {
            trial,
            residual,
            preconditioned,
            direction,
            product,
            diagonal,
        };
        let report = if self.reconstructible {
            // This proof has no external mechanical loads. Rebuild the total
            // eigenstrain load for every solve, never reuse it across products.
            self.external.fill(Scalar::ZERO);
            system.assemble_load(&self.external, &self.eigenstrain, workspace.product)?;
            self.external.copy_from_slice(workspace.product);
            system.solve_loaded(&mut self.displacement_trial, &self.external, &mut workspace)?
        } else {
            system.solve(
                &mut self.displacement_trial,
                &self.external,
                &self.eigenstrain,
                &mut workspace,
            )?
        };
        recover_stress(
            model,
            &self.displacement_trial,
            &self.eigenstrain,
            &mut self.stress_trial,
        )?;
        Ok(report)
    }

    fn update_eigenstrain(
        &mut self,
        model: &CompiledSpecimenMechanics,
        moisture: &[Scalar],
        temperature: &[Scalar],
        reference: [Scalar; 2],
    ) -> Result<(), PhysicsError> {
        if moisture.len().checked_mul(6) != Some(self.eigenstrain.len())
            || temperature.len() != moisture.len()
        {
            return Err(PhysicsError::BufferLength);
        }
        for (cell, (&moisture, &temperature)) in moisture.iter().zip(temperature).enumerate() {
            let strain = eigenstrain(model, moisture, temperature, reference[0], reference[1])?;
            self.eigenstrain[6 * cell..6 * cell + 6].fill(strain);
        }
        Ok(())
    }

    pub fn commit(&mut self, report: MechanicsReport) {
        self.displacement.copy_from_slice(&self.displacement_trial);
        self.stress.copy_from_slice(&self.stress_trial);
        self.report = report;
    }

    pub fn checkpoint(&self) -> MechanicalCheckpoint {
        MechanicalCheckpoint {
            displacement_raw: self.displacement.iter().map(|v| v.raw()).collect(),
            stress_raw: self.stress.iter().map(|v| v.raw()).collect(),
            iterations: self.report.iterations,
            residual_raw: self.report.residual.raw(),
        }
    }

    /// Restore validates derived stresses and restraints before publishing any
    /// displacement. Scratch fields need no serialization: each trial resets them.
    pub fn validate_restore(
        &mut self,
        model: &CompiledSpecimenMechanics,
        checkpoint: &MechanicalCheckpoint,
        moisture: &[i64],
        temperature: &[i64],
        reference: [Scalar; 2],
        step: u32,
    ) -> Result<MechanicsReport, String> {
        if checkpoint.displacement_raw.len() != self.displacement.len()
            || checkpoint.stress_raw.len() != self.stress.len()
            || checkpoint.iterations > model.limits().iterations
            || !(0..=model.limits().residual.raw()).contains(&checkpoint.residual_raw)
        {
            return Err("invalid mechanical checkpoint shape or solver state".into());
        }
        if self.reconstructible {
            return self.validate_reconstructed(
                model,
                checkpoint,
                moisture,
                temperature,
                reference,
                step,
            );
        }
        for ((trial, &raw), constraint) in self
            .displacement_trial
            .iter_mut()
            .zip(&checkpoint.displacement_raw)
            .zip(model.constraints())
        {
            let value = Scalar::from_raw(raw);
            if constraint.is_some_and(|prescribed| value != prescribed) {
                return Err("mechanical checkpoint violates a displacement restraint".into());
            }
            *trial = value;
        }
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
        recover_stress(
            model,
            &self.displacement_trial,
            &self.eigenstrain,
            &mut self.stress_trial,
        )
        .map_err(|e| e.to_string())?;
        if self
            .stress_trial
            .iter()
            .zip(&checkpoint.stress_raw)
            .any(|(value, raw)| value.raw() != *raw)
        {
            return Err("mechanical checkpoint stress is inconsistent with its fields".into());
        }
        Ok(MechanicsReport {
            iterations: checkpoint.iterations,
            residual: Scalar::from_raw(checkpoint.residual_raw),
        })
    }
}

fn eigenstrain(
    model: &CompiledSpecimenMechanics,
    moisture: Scalar,
    temperature: Scalar,
    reference_moisture: Scalar,
    reference_temperature: Scalar,
) -> Result<Scalar, PhysicsError> {
    moisture
        .checked_sub(reference_moisture)?
        .checked_mul(model.moisture_expansion())?
        .checked_add(
            temperature
                .checked_sub(reference_temperature)?
                .checked_mul(model.thermal_expansion())?,
        )
}

fn recover_stress(
    model: &CompiledSpecimenMechanics,
    displacement: &[Scalar],
    eigenstrain: &[Scalar],
    output: &mut [Scalar],
) -> Result<(), PhysicsError> {
    for (i, ((element, material), &strain)) in model
        .elements()
        .iter()
        .zip(model.materials())
        .zip(eigenstrain)
        .enumerate()
    {
        let tensor = material.stress(element.strain(displacement)?, strain)?;
        for (component, [row, column]) in [[0, 0], [1, 1], [2, 2], [1, 2], [0, 2], [0, 1]]
            .into_iter()
            .enumerate()
        {
            output[6 * i + component] = tensor[row][column];
        }
    }
    Ok(())
}
