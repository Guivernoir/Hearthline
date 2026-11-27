pub(super) use crate::physics::checkpoint::NumericalCause;
use serde::{Deserialize, Serialize};

use super::{PhysicsEvent, ScheduleFailure, ScheduleInstanceError};
use crate::physics::PhysicsStepError;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FailureRecord {
    pub event: PhysicsEvent,
    pub diagnostic: Diagnostic,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", try_from = "DiagnosticFields")]
pub(super) enum Diagnostic {
    UnknownInstance,
    WrongProfile,
    CompletedInstance,
    Numerical {
        cause: NumericalCause,
    },
    Flow {
        cause: NumericalCause,
        attempts: u32,
        smallest_solver_step_us: u64,
    },
    Connected {
        cause: NumericalCause,
        retry: Option<RetryRecord>,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Kind {
    UnknownInstance,
    WrongProfile,
    CompletedInstance,
    Numerical,
    Flow,
    Connected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticFields {
    kind: Kind,
    cause: Option<NumericalCause>,
    attempts: Option<u32>,
    smallest_solver_step_us: Option<u64>,
    retry: Option<RetryRecord>,
}

// Decode directly into fixed fields instead of buffering an internally tagged
// map; this keeps malformed header memory inside the framed decoder envelope.
impl TryFrom<DiagnosticFields> for Diagnostic {
    type Error = &'static str;
    fn try_from(f: DiagnosticFields) -> Result<Self, Self::Error> {
        match (
            f.kind,
            f.cause,
            f.attempts,
            f.smallest_solver_step_us,
            f.retry,
        ) {
            (Kind::UnknownInstance, None, None, None, None) => Ok(Self::UnknownInstance),
            (Kind::WrongProfile, None, None, None, None) => Ok(Self::WrongProfile),
            (Kind::CompletedInstance, None, None, None, None) => Ok(Self::CompletedInstance),
            (Kind::Numerical, Some(cause), None, None, None) => Ok(Self::Numerical { cause }),
            (Kind::Flow, Some(cause), Some(attempts), Some(smallest_solver_step_us), None) => {
                Ok(Self::Flow {
                    cause,
                    attempts,
                    smallest_solver_step_us,
                })
            }
            (Kind::Connected, Some(cause), None, None, retry) => {
                Ok(Self::Connected { cause, retry })
            }
            _ => Err("inconsistent numerical schedule diagnostic fields"),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RetryRecord {
    attempts: u32,
    substeps: u32,
    completed_substeps: u32,
    smallest_step_raw: i64,
}

impl From<ScheduleFailure> for FailureRecord {
    fn from(failure: ScheduleFailure) -> Self {
        let diagnostic = match failure.error {
            ScheduleInstanceError::UnknownInstance => Diagnostic::UnknownInstance,
            ScheduleInstanceError::WrongProfile => Diagnostic::WrongProfile,
            ScheduleInstanceError::CompletedInstance => Diagnostic::CompletedInstance,
            ScheduleInstanceError::Proof(PhysicsStepError::Numerical(cause)) => {
                Diagnostic::Numerical {
                    cause: cause.into(),
                }
            }
            ScheduleInstanceError::Proof(PhysicsStepError::Flow(error)) => Diagnostic::Flow {
                cause: error.cause.into(),
                attempts: error.attempts,
                smallest_solver_step_us: error.smallest_step_us,
            },
            ScheduleInstanceError::Connected { cause, retry } => Diagnostic::Connected {
                cause: cause.into(),
                retry: retry.map(|r| RetryRecord {
                    attempts: r.attempts,
                    substeps: r.substeps,
                    completed_substeps: r.completed_substeps,
                    smallest_step_raw: r.smallest_step.raw(),
                }),
            },
        };
        Self {
            event: failure.event,
            diagnostic,
        }
    }
}
