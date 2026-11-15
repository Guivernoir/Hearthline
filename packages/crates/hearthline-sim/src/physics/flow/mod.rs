use hearthline_physics::flow::{Conserved, FlowAdvanceReport, FlowAdvanceWorkspace};
use hearthline_physics::{PhysicsError, Scalar};
use hearthline_project::physics::{CompiledPhysicsProof, CompiledReceiverFlow};
use serde::{Deserialize, Serialize};

use super::{PhysicsStepError, field};

pub mod connected;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowCheckpoint {
    /// Cell-major normalized density, momentum xyz, total energy density.
    pub conserved_raw: Vec<[i64; 5]>,
    pub balance_error_raw: [i64; 5],
    pub last_step: Option<FlowStepCheckpoint>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowStepCheckpoint {
    pub solver_time_us: u64,
    pub substeps: u32,
    pub attempts: u32,
    pub successful_sweeps: u64,
    pub boundary_exchange_raw: [i64; 5],
    pub balance_error_raw: [i64; 5],
}

impl From<FlowAdvanceReport> for FlowStepCheckpoint {
    fn from(report: FlowAdvanceReport) -> Self {
        Self {
            solver_time_us: report.completed_us,
            substeps: report.substeps,
            attempts: report.attempts,
            successful_sweeps: report.successful_sweeps,
            boundary_exchange_raw: report.boundary_exchange.components().map(Scalar::raw),
            balance_error_raw: report.balance_error.components().map(Scalar::raw),
        }
    }
}

pub(super) struct FlowBuffers {
    pub state: Vec<Conserved>,
    trial: Vec<Conserved>,
    candidate: Vec<Conserved>,
    iteration_trial: Vec<Conserved>,
    iteration_next: Vec<Conserved>,
    balance_error: [Scalar; 5],
    last_step: Option<FlowStepCheckpoint>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FlowRuntimeMetadata {
    pub balance_error: [Scalar; 5],
    pub last_step: Option<FlowStepCheckpoint>,
}

impl FlowBuffers {
    pub(super) const fn metadata(&self) -> FlowRuntimeMetadata {
        FlowRuntimeMetadata {
            balance_error: self.balance_error,
            last_step: self.last_step,
        }
    }
    pub(super) fn save_population_state(
        &self,
        output: &mut [Scalar],
    ) -> Result<FlowRuntimeMetadata, PhysicsError> {
        if self.state.len().checked_mul(5) != Some(output.len()) {
            return Err(PhysicsError::BufferLength);
        }
        for (value, target) in self.state.iter().zip(output.as_chunks_mut::<5>().0) {
            *target = value.components();
        }
        Ok(FlowRuntimeMetadata {
            balance_error: self.balance_error,
            last_step: self.last_step,
        })
    }

    /// Internal state-bank transfer. External snapshots use validate_restore.
    pub(super) fn load_population_state(
        &mut self,
        input: &[Scalar],
        metadata: FlowRuntimeMetadata,
    ) -> Result<(), PhysicsError> {
        if self.state.len().checked_mul(5) != Some(input.len()) {
            return Err(PhysicsError::BufferLength);
        }
        for (value, source) in self.state.iter_mut().zip(input.as_chunks::<5>().0) {
            *value = Conserved::from_components(*source);
        }
        self.balance_error = metadata.balance_error;
        self.last_step = metadata.last_step;
        Ok(())
    }

    pub fn new(model: &CompiledPhysicsProof) -> Result<Self, String> {
        let flow = model.flow().ok_or("missing compiled flow model")?;
        let n = model.grid().count();
        let mut state = field(n, flow.upstream())?;
        for (cell, value) in state.iter_mut().enumerate() {
            if cell % model.grid().shape()[0] >= flow.receiver_start_x() {
                *value = flow.receiver();
            }
        }
        Ok(Self {
            state,
            trial: field(n, Conserved::default())?,
            candidate: field(n, Conserved::default())?,
            iteration_trial: field(n, Conserved::default())?,
            iteration_next: field(n, Conserved::default())?,
            balance_error: [Scalar::ZERO; 5],
            last_step: None,
        })
    }

    pub fn trial(
        &mut self,
        model: &CompiledReceiverFlow,
        pressure: &mut [Scalar],
        temperature: &mut [Scalar],
    ) -> Result<(FlowAdvanceReport, [Scalar; 5]), PhysicsStepError> {
        if pressure.len() != self.trial.len() || temperature.len() != self.trial.len() {
            return Err(PhysicsError::BufferLength.into());
        }
        self.trial.copy_from_slice(&self.state);
        let report = model
            .solver()
            .advance(
                model.solver_timestep_us(),
                model.retries(),
                &mut self.trial,
                &mut FlowAdvanceWorkspace {
                    candidate: &mut self.candidate,
                    iteration_trial: &mut self.iteration_trial,
                    iteration_next: &mut self.iteration_next,
                },
            )
            .map_err(PhysicsStepError::Flow)?;
        let mut balance = self.balance_error;
        for (value, error) in balance.iter_mut().zip(report.balance_error.components()) {
            *value = value.checked_add(error)?;
        }
        for (cell, &state) in self.trial.iter().enumerate() {
            [pressure[cell], temperature[cell]] = model.readings(state)?;
        }
        Ok((report, balance))
    }

    pub fn commit(&mut self, last_step: Option<FlowStepCheckpoint>, balance_error: [Scalar; 5]) {
        self.state.copy_from_slice(&self.trial);
        self.balance_error = balance_error;
        self.last_step = last_step;
    }

    pub fn checkpoint(&self) -> FlowCheckpoint {
        FlowCheckpoint {
            conserved_raw: self
                .state
                .iter()
                .map(|c| c.components().map(Scalar::raw))
                .collect(),
            balance_error_raw: self.balance_error.map(Scalar::raw),
            last_step: self.last_step,
        }
    }

    pub fn validate_restore(
        &mut self,
        model: &CompiledPhysicsProof,
        checkpoint: &FlowCheckpoint,
        pressure: &[i64],
        temperature: &[i64],
        step: u32,
    ) -> Result<(), String> {
        let flow = model.flow().ok_or("missing flow checkpoint model")?;
        if checkpoint.conserved_raw.len() != self.state.len() {
            return Err("flow checkpoint shape mismatch".into());
        }
        match checkpoint.last_step {
            None if step == 0 => (),
            Some(report)
                if step > 0
                    && report.solver_time_us == flow.solver_timestep_us()
                    && report.substeps.is_power_of_two()
                    && report.substeps <= flow.retries().maximum_substeps
                    && report.attempts == report.substeps.trailing_zeros() + 1
                    && (u64::from(report.substeps)
                        ..=u64::from(report.substeps) * u64::from(model.limits().iterations))
                        .contains(&report.successful_sweeps)
                    && report.solver_time_us / u64::from(report.substeps)
                        >= flow.retries().minimum_step_us => {}
            _ => return Err("invalid flow checkpoint solver state".into()),
        }
        for (cell, raw) in checkpoint.conserved_raw.iter().enumerate() {
            let state = Conserved::from_components(raw.map(Scalar::from_raw));
            let values = flow.readings(state).map_err(|e| e.to_string())?;
            if values.map(Scalar::raw) != [pressure[cell], temperature[cell]] {
                return Err(format!(
                    "flow checkpoint cell {cell} pressure/temperature disagrees with conserved fields"
                ));
            }
            self.trial[cell] = state;
        }
        Ok(())
    }
}
