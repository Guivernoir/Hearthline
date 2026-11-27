use super::{ConnectedWork, WorkerFault, WorkerFields};
use crate::physics::PhysicsProofSession;
use crate::physics::PhysicsStepError;
use crate::physics::clock::PacingError;
use crate::physics::flow::connected::{ConnectedGasSession, ConnectedStepError};
use crate::physics::schedule::{PhysicsSchedule, ScheduleError, ScheduledStepReport};

mod sealed {
    pub trait Sealed {}
    impl Sealed for crate::physics::PhysicsProofSession {}
    impl Sealed for crate::physics::schedule::PhysicsSchedule {}
    impl Sealed for crate::physics::flow::connected::ConnectedGasSession {}
}

impl WorkerRuntime for ConnectedGasSession {
    fn digest(&self) -> &str {
        self.model().digest()
    }
    fn event_count(&self) -> u64 {
        u64::from(self.step())
    }
    fn time_us(&self) -> Result<u64, WorkerFault> {
        Ok(ConnectedGasSession::time_us(self))
    }
    fn next_deadline_us(&self) -> Result<Option<u64>, WorkerFault> {
        Ok(self.next_time_us())
    }
    fn complete(&self) -> bool {
        ConnectedGasSession::complete(self)
    }
    fn initial_failure(&self) -> Option<WorkerFault> {
        self.failure().map(|e| {
            self.retry_failure().map_or(
                WorkerFault::Numerical(PhysicsStepError::Numerical(e)),
                WorkerFault::Connected,
            )
        })
    }
    fn initial_fields(&self) -> Result<Option<WorkerFields>, WorkerFault> {
        Ok(Some(WorkerFields {
            instance: None,
            profile_digest: digest_bytes(self.model().digest()),
            instance_step: self.step(),
            time_us: ConnectedGasSession::time_us(self),
            values: self.ranges(),
            connected_work: self.last_report().map(|last| {
                let advance = self.last_advance();
                ConnectedWork {
                    attempts: advance.map_or(1, |r| r.attempts),
                    substeps: advance.map_or(1, |r| r.substeps),
                    accepted_iterations: advance
                        .map_or(u64::from(last.iterations), |r| r.accepted_iterations),
                    accepted_evaluations: advance
                        .map_or(last.evaluations, |r| r.accepted_evaluations),
                    evaluation_bound: self.model().evaluation_bound(),
                }
            }),
        }))
    }
    fn advance(&mut self) -> Result<WorkerFields, WorkerFault> {
        self.step_next().map_err(|e| match e {
            ConnectedStepError::Complete => WorkerFault::RuntimeInvariant,
            ConnectedStepError::Failed(e) => self.retry_failure().map_or(
                WorkerFault::Numerical(PhysicsStepError::Numerical(e)),
                WorkerFault::Connected,
            ),
        })?;
        self.initial_fields()?.ok_or(WorkerFault::RuntimeInvariant)
    }
}

/// Sealed to the allocation-tested runtimes. This is an execution adapter, not
/// an extension point for arbitrary callbacks on the authoritative worker.
#[doc(hidden)]
pub trait WorkerRuntime: sealed::Sealed + Send + 'static {
    fn digest(&self) -> &str;
    fn event_count(&self) -> u64;
    fn time_us(&self) -> Result<u64, WorkerFault>;
    fn next_deadline_us(&self) -> Result<Option<u64>, WorkerFault>;
    fn complete(&self) -> bool;
    fn initial_failure(&self) -> Option<WorkerFault> {
        None
    }
    fn initial_fields(&self) -> Result<Option<WorkerFields>, WorkerFault> {
        Ok(None)
    }
    fn advance(&mut self) -> Result<WorkerFields, WorkerFault>;
}

pub(super) fn digest_bytes(digest: &str) -> [u8; 64] {
    digest
        .as_bytes()
        .try_into()
        .expect("compiled SHA-256 digest")
}

impl WorkerRuntime for PhysicsProofSession {
    fn digest(&self) -> &str {
        self.model.digest()
    }
    fn event_count(&self) -> u64 {
        u64::from(self.step)
    }
    fn time_us(&self) -> Result<u64, WorkerFault> {
        u64::from(self.step)
            .checked_mul(self.model.timestep_us())
            .ok_or(WorkerFault::Pacing(PacingError::Overflow))
    }
    fn next_deadline_us(&self) -> Result<Option<u64>, WorkerFault> {
        if self.complete() {
            return Ok(None);
        }
        (u64::from(self.step) + 1)
            .checked_mul(self.model.timestep_us())
            .map(Some)
            .ok_or(WorkerFault::Pacing(PacingError::Overflow))
    }
    fn complete(&self) -> bool {
        self.step == self.model.config().steps
    }
    fn initial_fields(&self) -> Result<Option<WorkerFields>, WorkerFault> {
        Ok(Some(WorkerFields {
            instance: None,
            profile_digest: digest_bytes(self.model.digest()),
            instance_step: self.step,
            time_us: self.time_us()?,
            values: self.field_ranges(),
            connected_work: None,
        }))
    }
    fn advance(&mut self) -> Result<WorkerFields, WorkerFault> {
        self.step().map_err(WorkerFault::Numerical)?;
        self.initial_fields()?.ok_or(WorkerFault::RuntimeInvariant)
    }
}

impl WorkerRuntime for PhysicsSchedule {
    fn digest(&self) -> &str {
        self.plan().digest()
    }
    fn event_count(&self) -> u64 {
        self.committed_events()
    }
    fn time_us(&self) -> Result<u64, WorkerFault> {
        Ok(PhysicsSchedule::time_us(self))
    }
    fn next_deadline_us(&self) -> Result<Option<u64>, WorkerFault> {
        Ok(self.next_event().map(|event| event.time_us))
    }
    fn complete(&self) -> bool {
        PhysicsSchedule::complete(self)
    }
    fn initial_failure(&self) -> Option<WorkerFault> {
        self.failure().map(WorkerFault::Schedule)
    }
    fn advance(&mut self) -> Result<WorkerFields, WorkerFault> {
        let result = self.step_next().map_err(|error| match error {
            ScheduleError::Complete => WorkerFault::RuntimeInvariant,
            ScheduleError::Failed(failure) => WorkerFault::Schedule(failure),
        })?;
        let binding = &self.plan().instances()[result.event.instance];
        let model = &self.plan().profiles()[binding.profile];
        Ok(WorkerFields {
            instance: Some(result.event.instance),
            profile_digest: digest_bytes(model.digest()),
            instance_step: result.instance_step,
            time_us: result.event.time_us,
            values: result.fields,
            connected_work: match result.report {
                ScheduledStepReport::Proof(_) => None,
                ScheduledStepReport::Connected(report) => Some(ConnectedWork {
                    attempts: report.attempts,
                    substeps: report.substeps,
                    accepted_iterations: report.accepted_iterations,
                    accepted_evaluations: report.accepted_evaluations,
                    evaluation_bound: model
                        .connected_gas()
                        .expect("connected report profile")
                        .evaluation_bound(),
                }),
            },
        })
    }
}
