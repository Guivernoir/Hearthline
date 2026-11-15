pub(super) use crate::physics::checkpoint::NumericalCause;
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
