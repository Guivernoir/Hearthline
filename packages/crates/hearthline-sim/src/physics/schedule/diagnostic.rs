use hearthline_physics::PhysicsError;
use serde::{Deserialize, Serialize};

use super::{PhysicsEvent, ScheduleFailure};
use crate::physics::PhysicsStepError;
use crate::physics::population::PopulationError;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FailureRecord {
    pub event: PhysicsEvent,
    pub diagnostic: Diagnostic,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Diagnostic {
    UnknownInstance,
    Numerical {
        cause: NumericalCause,
    },
    Flow {
        cause: NumericalCause,
        attempts: u32,
        smallest_solver_step_us: u64,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum NumericalCause {
    Overflow,
    DivisionByZero,
    InvalidDecimal,
    InvalidGeometry,
    InvalidCoefficient,
    InsufficientResolution,
    InvalidState,
    BufferLength,
    NonPositiveDefinite,
    NonConvergence { iterations: u32, residual_raw: i64 },
}

impl From<PhysicsError> for NumericalCause {
    fn from(error: PhysicsError) -> Self {
        match error {
            PhysicsError::Overflow => Self::Overflow,
            PhysicsError::DivisionByZero => Self::DivisionByZero,
            PhysicsError::InvalidDecimal => Self::InvalidDecimal,
            PhysicsError::InvalidGeometry => Self::InvalidGeometry,
            PhysicsError::InvalidCoefficient => Self::InvalidCoefficient,
            PhysicsError::InsufficientResolution => Self::InsufficientResolution,
            PhysicsError::InvalidState => Self::InvalidState,
            PhysicsError::BufferLength => Self::BufferLength,
            PhysicsError::NonPositiveDefinite => Self::NonPositiveDefinite,
            PhysicsError::NonConvergence {
                iterations,
                residual,
            } => Self::NonConvergence {
                iterations,
                residual_raw: residual.raw(),
            },
        }
    }
}

impl From<ScheduleFailure> for FailureRecord {
    fn from(failure: ScheduleFailure) -> Self {
        let diagnostic = match failure.error {
            PopulationError::UnknownInstance => Diagnostic::UnknownInstance,
            PopulationError::Step(PhysicsStepError::Numerical(cause)) => Diagnostic::Numerical {
                cause: cause.into(),
            },
            PopulationError::Step(PhysicsStepError::Flow(error)) => Diagnostic::Flow {
                cause: error.cause.into(),
                attempts: error.attempts,
                smallest_solver_step_us: error.smallest_step_us,
            },
        };
        Self {
            event: failure.event,
            diagnostic,
        }
    }
}
