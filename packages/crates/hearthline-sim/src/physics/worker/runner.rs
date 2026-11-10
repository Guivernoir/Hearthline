use std::sync::Arc;
use std::time::{Duration, Instant};

use hearthline_physics::Scalar;

use super::super::PhysicsProofSession;
use super::super::clock::{PacingError, PhysicsPacer};
use super::mailbox::{CloseOnExit, Request, Shared};
use super::{
    ClockCommand, CommandRejection, CommandReply, FieldRange, WorkerFault, WorkerProjection,
};

pub(super) struct Runner {
    pub session: PhysicsProofSession,
    pub clock: PhysicsPacer,
    revision: u64,
    generation: u64,
    fault: Option<WorkerFault>,
}

impl Runner {
    pub fn new(session: PhysicsProofSession) -> Result<Self, String> {
        let time_us = u64::from(session.step)
            .checked_mul(session.model.timestep_us())
            .ok_or("worker clock overflow")?;
        let mut clock = PhysicsPacer::at_boundary(session.model.timestep_us(), time_us)
            .map_err(|e| format!("{e:?}"))?;
        clock.set_paused(true);
        Ok(Self {
            session,
            clock,
            revision: 0,
            generation: 0,
            fault: None,
        })
    }

    pub fn projection(&self) -> WorkerProjection {
        WorkerProjection {
            model_digest: self
                .session
                .model
                .digest()
                .as_bytes()
                .try_into()
                .expect("compiled SHA-256 digest"),
            generation: self.generation,
            command_revision: self.revision,
            step: self.session.step,
            time_us: self.clock.completed_us(),
            mode: self.clock.mode(),
            paused: self.clock.paused(),
            complete: self.complete(),
            terminated: false,
            active_wall_us: self.clock.elapsed_wall_us(),
            automatic_completed_us: self.clock.automatic_completed_us(),
            pending_pacing_us: self.clock.pending_credit_us(),
            first_field: range(&self.session.field),
            temperature_k: range(&self.session.temperature),
            fault: self.fault,
            outstanding_commands: 0,
            command_high_water: 0,
        }
    }

    fn complete(&self) -> bool {
        self.session.step == self.session.model.config().steps
    }

    fn fail(&mut self, fault: WorkerFault) {
        if self.fault.is_none() {
            self.fault = Some(fault);
        }
        self.clock.set_paused(true);
    }

    pub fn accrue(&mut self, elapsed_us: u64) {
        if self.complete() || self.fault.is_some() {
            return;
        }
        if let Err(error) = self.clock.accrue(elapsed_us) {
            self.fail(WorkerFault::Pacing(error));
        }
    }

    pub fn cycle(&mut self, elapsed_us: u64, request: Option<Request>) -> Option<CommandReply> {
        self.accrue(elapsed_us);
        let reply = request.map(|request| {
            let result = self.command(request);
            CommandReply {
                ticket: request.ticket,
                revision: self.revision,
                result,
            }
        });
        if self.fault.is_none() && !self.complete() && self.clock.step_due() {
            // Preflight clock arithmetic before committing any physical field.
            let mut next_clock = self.clock;
            match next_clock.commit_step() {
                Err(error) => self.fail(WorkerFault::Pacing(error)),
                Ok(()) => match self.session.step() {
                    Ok(_) => self.clock = next_clock,
                    Err(error) => self.fail(WorkerFault::Numerical(error)),
                },
            }
        }
        if let Some(generation) = self.generation.checked_add(1) {
            self.generation = generation;
        } else {
            self.fail(WorkerFault::PublicationOverflow);
        }
        reply
    }

    fn command(&mut self, request: Request) -> Result<(), CommandRejection> {
        if request.expected_revision != self.revision {
            return Err(CommandRejection::StaleRevision {
                current: self.revision,
            });
        }
        if self.fault.is_some() {
            return Err(CommandRejection::Faulted);
        }
        if self.complete() {
            return Err(CommandRejection::Complete);
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(CommandRejection::RevisionOverflow)?;
        let mut next = self.clock;
        match request.command {
            ClockCommand::Pause => next.set_paused(true),
            ClockCommand::Resume => next.set_paused(false),
            ClockCommand::SetRate(rate) => next.set_rate(rate),
            ClockCommand::Headless => next.set_headless(),
            ClockCommand::SingleStep => {
                if !next.paused() {
                    return Err(CommandRejection::NotPaused);
                }
                next.request_single_step()
                    .map_err(CommandRejection::Pacing)?;
            }
        }
        self.clock = next;
        self.revision = revision;
        Ok(())
    }

    pub fn run(mut self, shared: Arc<Shared>) -> PhysicsProofSession {
        let _close = CloseOnExit(&shared);
        let mut sampled = Instant::now();
        while !shared.stopped() {
            let request = match shared.mailbox.lock() {
                Ok(mut mailbox) => mailbox.take_next(),
                Err(_) => break,
            };
            let elapsed = self.sample_wall(&mut sampled);
            let automatic_before = self.clock.automatic_completed_us();
            let reply = self.cycle(elapsed, request);
            // Include solver time in achieved speed and pacing debt immediately.
            let elapsed = self.sample_wall(&mut sampled);
            if self.complete() && self.clock.automatic_completed_us() != automatic_before {
                if let Err(error) = self.clock.accrue(elapsed) {
                    self.fail(WorkerFault::Pacing(error));
                }
            } else {
                self.accrue(elapsed);
            }
            let projection = self.projection();
            let mut mailbox = match shared.mailbox.lock() {
                Ok(mailbox) => mailbox,
                Err(_) => break,
            };
            mailbox.publish(projection, reply);
            shared.wake.notify_all();
            if shared.stopped() || mailbox.has_pending() {
                continue;
            }
            let wait = if self.complete() || self.fault.is_some() {
                None
            } else {
                self.clock.wait_us()
            };
            if wait == Some(0) {
                continue;
            }
            let poisoned = match wait {
                Some(us) => shared
                    .wake
                    .wait_timeout(
                        mailbox,
                        Duration::from_micros(us).min(Duration::from_secs(1)),
                    )
                    .is_err(),
                None => shared.wake.wait(mailbox).is_err(),
            };
            if poisoned {
                break;
            }
        }
        self.session
    }

    fn sample_wall(&mut self, sampled: &mut Instant) -> u64 {
        let now = Instant::now();
        let Some(elapsed) = now.checked_duration_since(*sampled) else {
            self.fail(WorkerFault::WallClockRegression);
            return 0;
        };
        let Ok(micros) = u64::try_from(elapsed.as_micros()) else {
            self.fail(WorkerFault::Pacing(PacingError::Overflow));
            return 0;
        };
        // Retain sub-microsecond remainder rather than losing time each poll.
        if let Some(next) = sampled.checked_add(Duration::from_micros(micros)) {
            *sampled = next;
        } else {
            self.fail(WorkerFault::Pacing(PacingError::Overflow));
        }
        micros
    }
}

fn range(field: &[Scalar]) -> FieldRange {
    let initial = field[0].raw();
    field.iter().fold(
        FieldRange {
            minimum_raw: initial,
            maximum_raw: initial,
        },
        |range, value| FieldRange {
            minimum_raw: range.minimum_raw.min(value.raw()),
            maximum_raw: range.maximum_raw.max(value.raw()),
        },
    )
}
