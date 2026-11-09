use super::{CompressibleFlow, Conserved, FlowWorkspace};
use crate::{PhysicsError, Scalar};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowRetryPolicy {
    pub maximum_substeps: u32,
    pub minimum_step_us: u64,
}

pub struct FlowAdvanceWorkspace<'a> {
    pub candidate: &'a mut [Conserved],
    pub iteration_trial: &'a mut [Conserved],
    pub iteration_next: &'a mut [Conserved],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowAdvanceReport {
    pub completed_us: u64,
    pub substeps: u32,
    pub attempts: u32,
    pub successful_sweeps: u64,
    pub boundary_exchange: Conserved,
    pub balance_error: Conserved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowAdvanceError {
    pub cause: PhysicsError,
    pub attempts: u32,
    pub smallest_step_us: u64,
}

impl core::fmt::Display for FlowAdvanceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "flow advance rejected after {} attempts (smallest step {} us): {}",
            self.attempts, self.smallest_step_us, self.cause
        )
    }
}
impl core::error::Error for FlowAdvanceError {}

impl CompressibleFlow {
    /// Retry the entire requested horizon from its initial state. Microseconds
    /// are split with integer remainders, so even odd horizons lose no time.
    /// Failed attempts may modify scratch fields but never the caller's state.
    pub fn advance(
        &self,
        horizon_us: u64,
        policy: FlowRetryPolicy,
        state: &mut [Conserved],
        workspace: &mut FlowAdvanceWorkspace<'_>,
    ) -> Result<FlowAdvanceReport, FlowAdvanceError> {
        let invalid = |cause| FlowAdvanceError {
            cause,
            attempts: 0,
            smallest_step_us: horizon_us,
        };
        if horizon_us == 0
            || policy.minimum_step_us == 0
            || horizon_us < policy.minimum_step_us
            || !policy.maximum_substeps.is_power_of_two()
        {
            return Err(invalid(PhysicsError::InvalidCoefficient));
        }
        if state.len() != self.grid.count()
            || [
                workspace.candidate.len(),
                workspace.iteration_trial.len(),
                workspace.iteration_next.len(),
            ]
            .iter()
            .any(|n| *n != state.len())
        {
            return Err(invalid(PhysicsError::BufferLength));
        }
        for cell in state.iter() {
            self.gas.primitive(*cell).map_err(invalid)?;
        }
        let before = self.inventory(state).map_err(invalid)?;
        let mut substeps = 1_u32;
        let mut attempts = 0;
        loop {
            attempts += 1;
            let floor_step = horizon_us / u64::from(substeps);
            let remainder = horizon_us % u64::from(substeps);
            workspace.candidate.copy_from_slice(state);
            let mut work = FlowWorkspace {
                trial: workspace.iteration_trial,
                next: workspace.iteration_next,
            };
            let mut sweeps = 0_u64;
            let mut exchange = Conserved::default();
            let mut error = None;
            for step in 0..substeps {
                let dt_us = floor_step + u64::from(u64::from(step) < remainder);
                let dt = Scalar::ratio_wide(i128::from(dt_us), 1_000_000).map_err(|cause| {
                    FlowAdvanceError {
                        cause,
                        attempts,
                        smallest_step_us: floor_step,
                    }
                })?;
                match self.solve(dt, workspace.candidate, &mut work) {
                    Ok(report) => {
                        sweeps = sweeps.checked_add(u64::from(report.iterations)).ok_or(
                            FlowAdvanceError {
                                cause: PhysicsError::Overflow,
                                attempts,
                                smallest_step_us: floor_step,
                            },
                        )?;
                        exchange = exchange
                            .checked_add_scaled(report.boundary_exchange, Scalar::ONE)
                            .map_err(|cause| FlowAdvanceError {
                                cause,
                                attempts,
                                smallest_step_us: floor_step,
                            })?;
                    }
                    Err(cause) => {
                        error = Some(cause);
                        break;
                    }
                }
            }
            if let Some(cause) = error {
                let retryable = matches!(
                    cause,
                    PhysicsError::NonConvergence { .. } | PhysicsError::InvalidState
                );
                let next_count = substeps.checked_mul(2);
                if !retryable
                    || next_count.is_none_or(|count| {
                        count > policy.maximum_substeps
                            || horizon_us / u64::from(count) < policy.minimum_step_us
                    })
                {
                    return Err(FlowAdvanceError {
                        cause,
                        attempts,
                        smallest_step_us: floor_step,
                    });
                }
                substeps = next_count.ok_or(FlowAdvanceError {
                    cause,
                    attempts,
                    smallest_step_us: floor_step,
                })?;
                continue;
            }
            let report = (|| -> Result<FlowAdvanceReport, PhysicsError> {
                let after = self.inventory(workspace.candidate)?;
                let balance_error = after
                    .checked_add_scaled(before, Scalar::integer(-1)?)?
                    .checked_add_scaled(exchange, Scalar::integer(-1)?)?;
                Ok(FlowAdvanceReport {
                    completed_us: horizon_us,
                    substeps,
                    attempts,
                    successful_sweeps: sweeps,
                    boundary_exchange: exchange,
                    balance_error,
                })
            })()
            .map_err(|cause| FlowAdvanceError {
                cause,
                attempts,
                smallest_step_us: floor_step,
            })?;
            state.copy_from_slice(workspace.candidate);
            return Ok(report);
        }
    }
}
