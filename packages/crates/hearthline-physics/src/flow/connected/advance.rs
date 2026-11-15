use super::{CellInventory, ConnectedFlow, ConnectedReport, ConnectedWorkspace};
use crate::transport::SolverLimits;
use crate::{PhysicsError, Scalar};

pub const CONNECTED_ADVANCE_CONTRACT: &str =
    "connected-advance/1:whole-horizon:leading-q9-remainder:fixed-tolerance";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectedRetryPolicy {
    pub maximum_substeps: u32,
    /// Minimum normalized duration, not seconds or physical-clock microseconds.
    pub minimum_step: Scalar,
}

impl ConnectedRetryPolicy {
    /// Conservative evaluation bound, including all rejected attempts. A solve
    /// evaluates at most twice per iteration; each retry starts from the origin.
    pub fn evaluation_bound(
        self,
        horizon: Scalar,
        limits: SolverLimits,
    ) -> Result<u64, PhysicsError> {
        if !self.maximum_substeps.is_power_of_two()
            || self.minimum_step <= Scalar::ZERO
            || horizon < self.minimum_step
            || limits.iterations == 0
            || limits.residual <= Scalar::ZERO
        {
            return Err(PhysicsError::InvalidCoefficient);
        }
        let mut substeps = 1_u32;
        let mut calls = 1_u64;
        while let Some(next) = self.next(horizon, substeps) {
            calls = calls
                .checked_add(u64::from(next))
                .ok_or(PhysicsError::Overflow)?;
            substeps = next;
        }
        calls
            .checked_mul(2 * u64::from(limits.iterations))
            .ok_or(PhysicsError::Overflow)
    }
    fn next(self, horizon: Scalar, substeps: u32) -> Option<u32> {
        let next = substeps.checked_mul(2)?;
        (next <= self.maximum_substeps
            && horizon.raw() / i64::from(next) >= self.minimum_step.raw())
        .then_some(next)
    }
}

pub struct ConnectedAdvanceWorkspace<'a> {
    pub candidate: &'a mut [CellInventory],
    pub trial: &'a mut [CellInventory],
    pub next: &'a mut [CellInventory],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectedAdvanceReport {
    pub completed: Scalar,
    pub substeps: u32,
    pub attempts: u32,
    /// Work in the successful attempt only; the policy bounds rejected work.
    pub accepted_iterations: u64,
    pub accepted_evaluations: u64,
    pub maximum_residual: Scalar,
    pub boundary_exchange: CellInventory,
    pub last: ConnectedReport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectedAdvanceError {
    pub cause: PhysicsError,
    pub attempts: u32,
    pub substeps: u32,
    pub completed_substeps: u32,
    /// Smallest duration in the final attempted partition, even if execution
    /// failed before reaching a shorter remainder-free substep.
    pub smallest_step: Scalar,
}

impl core::fmt::Display for ConnectedAdvanceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "connected flow rejected at attempt {}, substep {}/{} (minimum normalized Q9 duration {}): {}",
            self.attempts,
            self.completed_substeps,
            self.substeps,
            self.smallest_step.raw(),
            self.cause
        )
    }
}
impl core::error::Error for ConnectedAdvanceError {}

impl ConnectedFlow<'_> {
    /// Produces a complete-horizon candidate without changing accepted state.
    /// The caller publishes it only after its own projection/ledger checks.
    /// Scratch is unspecified on failure, including after successful substeps.
    pub fn advance(
        &self,
        horizon: Scalar,
        policy: ConnectedRetryPolicy,
        state: &[CellInventory],
        workspace: &mut ConnectedAdvanceWorkspace<'_>,
    ) -> Result<ConnectedAdvanceReport, ConnectedAdvanceError> {
        let invalid = |cause| ConnectedAdvanceError {
            cause,
            attempts: 0,
            substeps: 0,
            completed_substeps: 0,
            smallest_step: horizon,
        };
        policy
            .evaluation_bound(horizon, self.limits)
            .map_err(invalid)?;
        if state.len() != self.mesh.cells.len()
            || [
                workspace.candidate.len(),
                workspace.trial.len(),
                workspace.next.len(),
            ]
            .iter()
            .any(|n| *n != state.len())
        {
            return Err(invalid(PhysicsError::BufferLength));
        }
        self.validate_state(state).map_err(invalid)?;
        let before = self.inventory(state).map_err(invalid)?;
        let mut substeps = 1_u32;
        let mut attempts = 0;
        loop {
            attempts += 1;
            let floor = horizon.raw() / i64::from(substeps);
            let remainder = horizon.raw() % i64::from(substeps);
            workspace.candidate.copy_from_slice(state);
            let mut work = ConnectedWorkspace {
                trial: workspace.trial,
                next: workspace.next,
            };
            let mut iterations = 0_u64;
            let mut evaluations = 0_u64;
            let mut maximum_residual = Scalar::ZERO;
            let mut exchange = CellInventory::default();
            let mut last = None;
            let mut rejected = None;
            for step in 0..substeps {
                let failure = |cause| ConnectedAdvanceError {
                    cause,
                    attempts,
                    substeps,
                    completed_substeps: step,
                    smallest_step: Scalar::from_raw(floor),
                };
                let dt = Scalar::from_raw(floor + i64::from(i64::from(step) < remainder));
                match self.solve(dt, workspace.candidate, &mut work) {
                    Ok(report) => {
                        iterations = iterations
                            .checked_add(u64::from(report.iterations))
                            .ok_or_else(|| failure(PhysicsError::Overflow))?;
                        evaluations = evaluations
                            .checked_add(report.evaluations)
                            .ok_or_else(|| failure(PhysicsError::Overflow))?;
                        maximum_residual = maximum_residual.max(report.residual);
                        exchange = exchange
                            .add(report.boundary_exchange, false)
                            .map_err(failure)?;
                        last = Some(report);
                    }
                    Err(cause) => {
                        rejected = Some(failure(cause));
                        break;
                    }
                }
            }
            if let Some(failure) = rejected {
                if matches!(
                    failure.cause,
                    PhysicsError::NonConvergence { .. } | PhysicsError::InvalidState
                ) && let Some(next) = policy.next(horizon, substeps)
                {
                    substeps = next;
                    continue;
                }
                return Err(failure);
            }
            let failure = |cause| ConnectedAdvanceError {
                cause,
                attempts,
                substeps,
                completed_substeps: substeps,
                smallest_step: Scalar::from_raw(floor),
            };
            let balance = self
                .inventory(workspace.candidate)
                .map_err(failure)?
                .add(before, true)
                .map_err(failure)?
                .add(exchange, true)
                .map_err(failure)?;
            if balance != CellInventory::default() {
                return Err(failure(PhysicsError::InvalidState));
            }
            return Ok(ConnectedAdvanceReport {
                completed: horizon,
                substeps,
                attempts,
                accepted_iterations: iterations,
                accepted_evaluations: evaluations,
                maximum_residual,
                boundary_exchange: exchange,
                last: last.ok_or_else(|| failure(PhysicsError::InvalidState))?,
            });
        }
    }
}
