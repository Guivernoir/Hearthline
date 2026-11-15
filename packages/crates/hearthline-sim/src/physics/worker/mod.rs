//! Dedicated execution for the numerical qualification models. No API handler
//! receives mutable plant state. Canonical factory integration remains gated.

mod backend;
mod mailbox;
mod runner;
#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;

use super::flow::connected::{ConnectedGasSession, ConnectedSessionMemory};
use hearthline_project::physics::CompiledPhysicsProof;
use hearthline_project::physics::flow::connected::CompiledConnectedGas;
use hearthline_project::physics::schedule::CompiledPhysicsSchedule;

use super::clock::{PacingError, PlaybackMode, PlaybackRate};
use super::schedule::{PhysicsSchedule, ScheduleFailure, ScheduleMemoryPlan};
use super::{NumericalFieldRanges, PhysicsProofSession, PhysicsStepError};
pub use backend::WorkerRuntime;
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
    Connected(hearthline_physics::flow::connected::ConnectedAdvanceError),
    Schedule(ScheduleFailure),
    Pacing(PacingError),
    PublicationOverflow,
    WallClockRegression,
    RuntimeInvariant,
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
pub struct ConnectedWork {
    pub attempts: u32,
    pub substeps: u32,
    pub accepted_iterations: u64,
    pub accepted_evaluations: u64,
    /// Conservative bound for the complete horizon, including rejected work.
    pub evaluation_bound: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerFields {
    /// None is the single-proof runtime; aggregate instances use compiled order.
    pub instance: Option<usize>,
    pub profile_digest: [u8; 64],
    pub instance_step: u32,
    pub time_us: u64,
    pub values: NumericalFieldRanges,
    pub connected_work: Option<ConnectedWork>,
}

/// Engineering state, not instrument readings. Wall metrics do not enter replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerProjection {
    pub model_digest: [u8; 64],
    pub generation: u64,
    pub command_revision: u64,
    pub committed_events: u64,
    pub time_us: u64,
    pub mode: PlaybackMode,
    pub paused: bool,
    pub complete: bool,
    pub terminated: bool,
    pub active_wall_us: u64,
    pub automatic_completed_us: u64,
    pub pending_pacing_us: u64,
    /// Latest accepted instance update, never extrema across unrelated profiles.
    pub fields: Option<WorkerFields>,
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
    pub fn compile_connected(
        model: &CompiledConnectedGas,
        policy: WorkerPolicy,
    ) -> Result<Self, String> {
        Self::with_runtime(
            ConnectedSessionMemory::compile(model)?.runtime_bytes,
            model.memory().allocation_budget_bytes,
            policy,
        )
    }
    pub fn compile(model: &CompiledPhysicsProof, policy: WorkerPolicy) -> Result<Self, String> {
        let numerical_bytes = model
            .storage_layout()
            .instance_buffer_bytes()?
            .checked_add(model.storage_layout().immutable_bytes()?)
            .ok_or("worker memory overflow")?;
        Self::with_runtime(
            numerical_bytes,
            model.config().allocation_budget_bytes,
            policy,
        )
    }

    pub fn compile_schedule(
        plan: &CompiledPhysicsSchedule,
        policy: WorkerPolicy,
    ) -> Result<Self, String> {
        let numerical_bytes = ScheduleMemoryPlan::compile(plan)?.total_planned_bytes;
        Self::with_runtime(numerical_bytes, plan.allocation_budget_bytes(), policy)
    }

    fn with_runtime(
        numerical_bytes: u64,
        budget: u64,
        policy: WorkerPolicy,
    ) -> Result<Self, String> {
        if policy.command_capacity == 0
            || policy.reviewed_command_burst == 0
            || policy.reviewed_command_burst as u128 * 4 > policy.command_capacity as u128 * 3
            || policy.stack_bytes < 1024 * 1024
        {
            return Err("worker requires a 1 MiB stack and 25% command reserve over a nonzero reviewed burst".into());
        }
        let mailbox_bytes = mailbox::allocation_bytes(policy.command_capacity)?;
        let requested_stack_bytes = u64::try_from(policy.stack_bytes).map_err(|e| e.to_string())?;
        let planned_bytes = numerical_bytes
            .checked_add(mailbox_bytes)
            .and_then(|n| n.checked_add(requested_stack_bytes))
            .ok_or("worker memory overflow")?;
        let reviewed_budget_bytes =
            u64::try_from(u128::from(budget) * 3 / 4).map_err(|e| e.to_string())?;
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

pub struct PhysicsWorker<S: WorkerRuntime = PhysicsProofSession> {
    client: PhysicsWorkerClient,
    thread: Option<JoinHandle<S>>,
    memory: WorkerMemoryPlan,
}

impl PhysicsWorker<ConnectedGasSession> {
    pub fn spawn_connected(
        model: Arc<CompiledConnectedGas>,
        policy: WorkerPolicy,
    ) -> Result<Self, String> {
        let memory = WorkerMemoryPlan::compile_connected(&model, policy)?;
        Self::launch(ConnectedGasSession::new(model)?, policy, memory)
    }
    pub fn spawn_connected_session(
        session: ConnectedGasSession,
        policy: WorkerPolicy,
    ) -> Result<Self, String> {
        let memory = WorkerMemoryPlan::compile_connected(session.model(), policy)?;
        Self::launch(session, policy, memory)
    }
}

impl PhysicsWorker<PhysicsProofSession> {
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
}

impl PhysicsWorker<PhysicsSchedule> {
    pub fn spawn_schedule(
        plan: Arc<CompiledPhysicsSchedule>,
        policy: WorkerPolicy,
    ) -> Result<Self, String> {
        let memory = WorkerMemoryPlan::compile_schedule(&plan, policy)?;
        Self::launch(PhysicsSchedule::new(plan)?, policy, memory)
    }

    pub fn spawn_schedule_session(
        session: PhysicsSchedule,
        policy: WorkerPolicy,
    ) -> Result<Self, String> {
        let memory = WorkerMemoryPlan::compile_schedule(session.plan(), policy)?;
        Self::launch(session, policy, memory)
    }
}

impl<S: WorkerRuntime> PhysicsWorker<S> {
    fn launch(session: S, policy: WorkerPolicy, memory: WorkerMemoryPlan) -> Result<Self, String> {
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
    pub fn shutdown(mut self) -> Result<S, WorkerAccessError> {
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

impl<S: WorkerRuntime> Drop for PhysicsWorker<S> {
    fn drop(&mut self) {
        self.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
