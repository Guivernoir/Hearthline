use hearthline_physics::mechanics::MechanicsReport;
use hearthline_physics::{PhysicsError, Scalar};
use hearthline_project::physics::CompiledProofCase;

use super::super::flow::FlowRuntimeMetadata;
use super::super::{PhysicsProofSession, buffers::ProofBuffers};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SlotMetadata {
    pub step: u32,
    pub balance_error: [Scalar; 2],
    pub flow: Option<FlowRuntimeMetadata>,
    pub mechanics: Option<MechanicsReport>,
    pub diffusion: Option<super::super::DiffusionCheckpoint>,
}

impl PhysicsProofSession {
    pub(in crate::physics::population) fn save_population_state(
        &self,
        output: &mut [Scalar],
    ) -> Result<SlotMetadata, PhysicsError> {
        if byte_len(output.len()) != Some(self.model.storage_layout().state_bytes) {
            return Err(PhysicsError::BufferLength);
        }
        let (flow, mechanics) = if let ProofBuffers::Flow(buffers) = &self.buffers {
            (Some(buffers.save_population_state(output)?), None)
        } else {
            let (first, rest) = output.split_at_mut(self.field.len());
            let (temperature, displacement) = rest.split_at_mut(self.temperature.len());
            first.copy_from_slice(&self.field);
            temperature.copy_from_slice(&self.temperature);
            (
                None,
                self.mechanics
                    .as_ref()
                    .map(|m| m.save_population_state(displacement))
                    .transpose()?,
            )
        };
        Ok(SlotMetadata {
            step: self.step,
            balance_error: self.balance_error,
            flow,
            mechanics,
            diffusion: self.diffusion,
        })
    }

    /// Only the private state bank calls this. Derived gas readings, elastic
    /// stress and opt-in zero-start displacement use the same checked solver.
    /// Untrusted snapshots must first pass PhysicsProofSession::restore.
    pub(in crate::physics::population) fn load_population_state(
        &mut self,
        input: &[Scalar],
        metadata: SlotMetadata,
    ) -> Result<(), PhysicsError> {
        if byte_len(input.len()) != Some(self.model.storage_layout().state_bytes)
            || metadata.flow.is_some() != self.model.flow().is_some()
            || metadata.mechanics.is_some() != self.model.mechanics().is_some()
        {
            return Err(PhysicsError::BufferLength);
        }
        if let (ProofBuffers::Flow(buffers), Some(flow)) = (&mut self.buffers, metadata.flow) {
            buffers.load_population_state(input, flow)?;
            let model = self.model.flow().ok_or(PhysicsError::InvalidState)?;
            for (cell, &state) in buffers.state.iter().enumerate() {
                [self.field[cell], self.temperature[cell]] = model.readings(state)?;
            }
        } else {
            let (first, rest) = input.split_at(self.field.len());
            let (temperature, displacement) = rest.split_at(self.temperature.len());
            if let (
                Some(mechanics),
                Some(report),
                Some(model),
                CompiledProofCase::Drying {
                    moisture: reference_moisture,
                    temperature: reference_temperature,
                    ..
                },
            ) = (
                &mut self.mechanics,
                metadata.mechanics,
                self.model.mechanics(),
                self.model.case(),
            ) {
                mechanics.load_population_state(
                    model,
                    displacement,
                    report,
                    [first, temperature],
                    [reference_moisture, reference_temperature],
                    metadata.step,
                )?;
            }
            self.field.copy_from_slice(first);
            self.temperature.copy_from_slice(temperature);
        }
        self.step = metadata.step;
        self.balance_error = metadata.balance_error;
        self.diffusion = metadata.diffusion;
        Ok(())
    }
}

fn byte_len(values: usize) -> Option<u64> {
    u64::try_from(values)
        .ok()?
        .checked_mul(size_of::<Scalar>() as u64)
}
