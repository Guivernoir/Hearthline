//! Dedicated execution for the numerical qualification models. No API handler
//! receives mutable plant state. Canonical factory integration remains gated.

mod mailbox;
mod runner;
#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;

use hearthline_project::physics::CompiledPhysicsProof;

use super::clock::{PacingError, PlaybackMode, PlaybackRate};
use super::{PhysicsProofSession, PhysicsStepError};
use mailbox::Shared;
pub use mailbox::{CommandTicket, PhysicsWorkerClient};
use runner::Runner;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockCommand {
    Pause,
    Resume,
    SingleStep,
    SetRate(PlaybackRate),
    Headless,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandRejection {
    StaleRevision { current: u64 },
    NotPaused,
    Complete,
    Faulted,
    RevisionOverflow,
    Pacing(PacingError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandReply {
    pub ticket: CommandTicket,
    pub revision: u64,
    pub result: Result<(), CommandRejection>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerFault {
    Numerical(PhysicsStepError),
    Pacing(PacingError),
    PublicationOverflow,
    WallClockRegression,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerAccessError {
    MailboxFull,
    UnknownTicket,
    SequenceOverflow,
    Stopped,
    Poisoned,
    Panicked,
}

impl std::fmt::Display for WorkerAccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "physics worker access: {self:?}")
    }
}
impl std::error::Error for WorkerAccessError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FieldRange {
    pub minimum_raw: i64,
    pub maximum_raw: i64,
}

/// Engineering state, not instrument readings. Wall metrics do not enter replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerProjection {
    pub model_digest: [u8; 64],
    pub generation: u64,
    pub command_revision: u64,
    pub step: u32,
    pub time_us: u64,
    pub mode: PlaybackMode,
    pub paused: bool,
    pub complete: bool,
    pub terminated: bool,
    pub active_wall_us: u64,
    pub automatic_completed_us: u64,
    pub pending_pacing_us: u64,
    pub first_field: FieldRange,
    pub temperature_k: FieldRange,
    pub fault: Option<WorkerFault>,
    pub outstanding_commands: usize,
    pub command_high_water: usize,
}

impl WorkerProjection {
    /// Thousandths of simulated seconds per active wall second. None means no
    /// active wall interval was measured (e.g. manual stepping while paused),
    /// or the reporting ratio cannot fit u64. This never changes physical state.
    pub fn achieved_milli_rate(self) -> Option<u64> {
        (self.active_wall_us != 0)
            .then(|| {
                u128::from(self.automatic_completed_us) * 1000 / u128::from(self.active_wall_us)
            })
            .and_then(|value| u64::try_from(value).ok())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerPolicy {
    pub command_capacity: usize,
    pub reviewed_command_burst: usize,
    pub stack_bytes: usize,
}

impl WorkerPolicy {
    pub const fn reviewed_default() -> Self {
        Self {
            command_capacity: 16,
            reviewed_command_burst: 12,
            stack_bytes: 1024 * 1024,
        }
    }
}

/// Numerical arrays, immutable numerical payloads, mailbox objects and requested
/// thread stack. OS/thread bookkeeping, model strings and allocator overhead
/// still require application RSS measurement; this is not that measurement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerMemoryPlan {
    pub numerical_bytes: u64,
    pub mailbox_bytes: u64,
    pub requested_stack_bytes: u64,
    pub planned_bytes: u64,
    pub reviewed_budget_bytes: u64,
}

impl WorkerMemoryPlan {
    pub fn compile(model: &CompiledPhysicsProof, policy: WorkerPolicy) -> Result<Self, String> {
        if policy.command_capacity == 0
            || policy.reviewed_command_burst == 0
            || policy.reviewed_command_burst as u128 * 4 > policy.command_capacity as u128 * 3
            || policy.stack_bytes < 1024 * 1024
        {
            return Err("worker requires a 1 MiB stack and 25% command reserve over a nonzero reviewed burst".into());
        }
        let numerical_bytes = model
            .storage_layout()
            .instance_buffer_bytes()?
            .checked_add(model.storage_layout().immutable_bytes()?)
            .ok_or("worker memory overflow")?;
        let mailbox_bytes = mailbox::allocation_bytes(policy.command_capacity)?;
        let requested_stack_bytes = u64::try_from(policy.stack_bytes).map_err(|e| e.to_string())?;
        let planned_bytes = numerical_bytes
            .checked_add(mailbox_bytes)
            .and_then(|n| n.checked_add(requested_stack_bytes))
            .ok_or("worker memory overflow")?;
        let reviewed_budget_bytes =
            u64::try_from(u128::from(model.config().allocation_budget_bytes) * 3 / 4)
                .map_err(|e| e.to_string())?;
        if planned_bytes > reviewed_budget_bytes {
            return Err("physics worker exceeds its budget with 25% reserve".into());
        }
        Ok(Self {
            numerical_bytes,
            mailbox_bytes,
            requested_stack_bytes,
            planned_bytes,
            reviewed_budget_bytes,
        })
    }
}

pub struct PhysicsWorker {
    client: PhysicsWorkerClient,
    thread: Option<JoinHandle<PhysicsProofSession>>,
    memory: WorkerMemoryPlan,
}

impl PhysicsWorker {
    /// Starts paused. Construction and thread startup precede sealed execution.
    pub fn spawn(model: Arc<CompiledPhysicsProof>, policy: WorkerPolicy) -> Result<Self, String> {
        let memory = WorkerMemoryPlan::compile(&model, policy)?;
        Self::launch(PhysicsProofSession::new(model)?, policy, memory)
    }

    /// A validated checkpoint can be restored before transferring exclusive
    /// ownership here. Resumed worker pacing starts paused at that boundary.
    pub fn spawn_session(
        session: PhysicsProofSession,
        policy: WorkerPolicy,
    ) -> Result<Self, String> {
        let memory = WorkerMemoryPlan::compile(&session.model, policy)?;
        Self::launch(session, policy, memory)
    }

    fn launch(
        session: PhysicsProofSession,
        policy: WorkerPolicy,
        memory: WorkerMemoryPlan,
    ) -> Result<Self, String> {
        let runner = Runner::new(session)?;
        let shared = Arc::new(Shared::new(policy.command_capacity, runner.projection())?);
        let client = PhysicsWorkerClient {
            shared: Arc::clone(&shared),
        };
        let thread = std::thread::Builder::new()
            .name("hearthline-physics".into())
            .stack_size(policy.stack_bytes)
            .spawn(move || runner.run(shared))
            .map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            thread: Some(thread),
            memory,
        })
    }

    pub fn client(&self) -> PhysicsWorkerClient {
        self.client.clone()
    }
    pub const fn memory(&self) -> &WorkerMemoryPlan {
        &self.memory
    }

    /// Stops at the next accepted/rejected solver boundary, joins the worker,
    /// and returns its state for archival or an explicit new session.
    pub fn shutdown(mut self) -> Result<PhysicsProofSession, WorkerAccessError> {
        self.stop();
        let session = self
            .thread
            .take()
            .ok_or(WorkerAccessError::Stopped)?
            .join()
            .map_err(|_| WorkerAccessError::Panicked)?;
        if self.client.shared.mailbox.is_poisoned() {
            return Err(WorkerAccessError::Poisoned);
        }
        Ok(session)
    }

    fn stop(&self) {
        let _mailbox = self.client.shared.mailbox.lock();
        self.client.shared.shutdown.store(true, Ordering::Release);
        self.client.shared.wake.notify_all();
    }
}

impl Drop for PhysicsWorker {
    fn drop(&mut self) {
        self.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
