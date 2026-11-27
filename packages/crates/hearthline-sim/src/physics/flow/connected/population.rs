//! Independent accepted gas fields with a shared immutable model and one serial
//! working copy. Connections inside each instance still solve as one island.

use std::io::{self, BufRead, Write};
use std::sync::Arc;

use hearthline_physics::PhysicsError;
use hearthline_physics::flow::connected::{
    CellInventory, ConnectedAdvanceError, ConnectedAdvanceReport, ConnectedReport,
};
use hearthline_project::physics::flow::connected::CompiledConnectedGas;

use super::{ConnectedGasSession, ConnectedSessionMemory, ConnectedStepError};
use crate::physics::NumericalFieldRanges;

mod archive;
pub use archive::CONNECTED_POPULATION_SCHEMA;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectedPopulationWork {
    pub physical_steps: u64,
    pub maximum_solver_evaluations: u64,
    pub maximum_restore_solver_evaluations: u64,
}

impl ConnectedPopulationWork {
    pub fn compile(model: &CompiledConnectedGas, instances: u64) -> Result<Self, String> {
        if instances == 0 {
            return Err("connected population requires at least one instance".into());
        }
        let physical_steps = instances
            .checked_mul(u64::from(model.profile().steps))
            .ok_or("gas population physical-step count overflow")?;
        let maximum_solver_evaluations = physical_steps
            .checked_mul(model.evaluation_bound())
            .ok_or("gas population solver-work count overflow")?;
        // Each child restore may repeat one failed horizon before accepting its
        // diagnostic. Successful history is validated, not rerun from time zero.
        let maximum_restore_solver_evaluations = instances
            .checked_mul(model.evaluation_bound())
            .ok_or("gas population restore-work count overflow")?;
        Ok(Self {
            physical_steps,
            maximum_solver_evaluations,
            maximum_restore_solver_evaluations,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SlotState {
    step: u32,
    exchange: CellInventory,
    last_report: Option<ConnectedReport>,
    last_advance: Option<ConnectedAdvanceReport>,
    failure: Option<PhysicsError>,
    retry_failure: Option<ConnectedAdvanceError>,
    ranges: NumericalFieldRanges,
}

impl SlotState {
    fn capture(session: &ConnectedGasSession) -> Self {
        Self {
            step: session.step,
            exchange: session.exchange,
            last_report: session.last_report,
            last_advance: session.last_advance,
            failure: session.failure,
            retry_failure: session.retry_failure,
            ranges: session.ranges,
        }
    }
    fn load(self, session: &mut ConnectedGasSession) {
        session.step = self.step;
        session.exchange = self.exchange;
        session.last_report = self.last_report;
        session.last_advance = self.last_advance;
        session.failure = self.failure;
        session.retry_failure = self.retry_failure;
        session.ranges = self.ranges;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectedPopulationMemory {
    pub instances: u64,
    pub cells_per_instance: u64,
    pub state_bank_bytes: u64,
    pub slot_metadata_bytes: u64,
    pub shared_runtime_bytes: u64,
    pub total_planned_bytes: u64,
    pub restore_peak_bytes: u64,
    pub maximum_frame_bytes: u64,
    pub reviewed_budget_bytes: u64,
    pub within_budget: bool,
    pub restore_within_budget: bool,
}

impl ConnectedPopulationMemory {
    pub fn compile(model: &CompiledConnectedGas, instances: u64) -> Result<Self, String> {
        if instances == 0 {
            return Err("connected population requires at least one instance".into());
        }
        let cells_per_instance = model.mesh().memory().cells;
        let bank_count = cells_per_instance
            .checked_mul(instances)
            .ok_or("gas population count overflow")?;
        let state_bank_bytes = bank_count
            .checked_mul(size_of::<CellInventory>() as u64)
            .ok_or("gas bank size overflow")?;
        usize::try_from(state_bank_bytes).map_err(|e| e.to_string())?;
        let slot_metadata_bytes = instances
            .checked_mul(size_of::<SlotState>() as u64)
            .ok_or("gas metadata size overflow")?;
        let session = ConnectedSessionMemory::compile(model)?;
        let shared_runtime_bytes = session.runtime_bytes;
        let object_overhead =
            (size_of::<ConnectedGasPopulation>() - size_of::<ConnectedGasSession>()) as u64;
        let total_planned_bytes = shared_runtime_bytes
            .checked_add(state_bank_bytes)
            .and_then(|n| n.checked_add(slot_metadata_bytes))
            .and_then(|n| n.checked_add(object_overhead))
            .ok_or("gas population memory overflow")?;
        // Transactional restore validates one temporary session before publishing
        // its state to the bank. Immutable payloads remain shared through Arc.
        let restore_peak_bytes = total_planned_bytes
            .checked_add(model.memory().field_bytes)
            .and_then(|n| n.checked_add(size_of::<ConnectedGasSession>() as u64))
            .and_then(|n| n.checked_add(session.snapshot_decoder_bytes))
            .and_then(|n| n.checked_add(super::archive::ROW_LIMIT as u64))
            .ok_or("gas population restore size overflow")?;
        let maximum_frame_bytes = cells_per_instance
            .checked_add(2)
            .and_then(|n| n.checked_mul(super::archive::ROW_LIMIT as u64))
            .ok_or("gas snapshot frame size overflow")?;
        let reviewed_budget_bytes = session.reviewed_budget_bytes;
        Ok(Self {
            instances,
            cells_per_instance,
            state_bank_bytes,
            slot_metadata_bytes,
            shared_runtime_bytes,
            total_planned_bytes,
            restore_peak_bytes,
            maximum_frame_bytes,
            reviewed_budget_bytes,
            within_budget: total_planned_bytes <= reviewed_budget_bytes,
            restore_within_budget: restore_peak_bytes <= reviewed_budget_bytes,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectedPopulationError {
    UnknownInstance,
    Step(ConnectedStepError),
}
impl std::fmt::Display for ConnectedPopulationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "connected population: {self:?}")
    }
}
impl std::error::Error for ConnectedPopulationError {}

pub struct ConnectedGasPopulation {
    worker: ConnectedGasSession,
    values: Box<[CellInventory]>,
    metadata: Box<[SlotState]>,
    stride: usize,
    memory: ConnectedPopulationMemory,
    work: ConnectedPopulationWork,
}

impl ConnectedGasPopulation {
    pub fn new(model: Arc<CompiledConnectedGas>, instances: usize) -> Result<Self, String> {
        let work = ConnectedPopulationWork::compile(&model, instances as u64)?;
        let memory = ConnectedPopulationMemory::compile(&model, instances as u64)?;
        if !memory.within_budget {
            return Err("connected population exceeds budget with 25% reserve".into());
        }
        let worker = ConnectedGasSession::new(model)?;
        let stride = worker.state.len();
        let count = usize::try_from(memory.state_bank_bytes / size_of::<CellInventory>() as u64)
            .map_err(|e| e.to_string())?;
        let mut values = super::buffer(count)?;
        for slot in values.chunks_exact_mut(stride) {
            slot.copy_from_slice(&worker.state);
        }
        let mut metadata = Vec::new();
        metadata
            .try_reserve_exact(instances)
            .map_err(|e| e.to_string())?;
        metadata.resize(instances, SlotState::capture(&worker));
        Ok(Self {
            worker,
            values,
            metadata: metadata.into_boxed_slice(),
            stride,
            memory,
            work,
        })
    }
    pub fn instance_count(&self) -> usize {
        self.metadata.len()
    }
    pub fn model(&self) -> &CompiledConnectedGas {
        &self.worker.model
    }
    pub const fn memory(&self) -> &ConnectedPopulationMemory {
        &self.memory
    }
    pub const fn work(&self) -> &ConnectedPopulationWork {
        &self.work
    }
    pub fn completed_steps(&self, instance: usize) -> Result<u32, ConnectedPopulationError> {
        Ok(self.slot(instance)?.step)
    }
    pub fn failure(
        &self,
        instance: usize,
    ) -> Result<Option<PhysicsError>, ConnectedPopulationError> {
        Ok(self.slot(instance)?.failure)
    }
    pub fn retry_failure(
        &self,
        instance: usize,
    ) -> Result<Option<ConnectedAdvanceError>, ConnectedPopulationError> {
        Ok(self.slot(instance)?.retry_failure)
    }
    pub fn fields(&self, instance: usize) -> Result<&[CellInventory], ConnectedPopulationError> {
        self.slot(instance)?;
        Ok(&self.values[instance * self.stride..(instance + 1) * self.stride])
    }
    pub fn ranges(
        &self,
        instance: usize,
    ) -> Result<NumericalFieldRanges, ConnectedPopulationError> {
        Ok(self.slot(instance)?.ranges)
    }
    pub fn step(
        &mut self,
        instance: usize,
    ) -> Result<ConnectedAdvanceReport, ConnectedPopulationError> {
        self.load(instance)?;
        let result = self.worker.step_next();
        self.commit(instance);
        result.map_err(ConnectedPopulationError::Step)
    }
    pub fn instance_digest(&mut self, instance: usize) -> Result<[u8; 64], serde_json::Error> {
        self.load(instance).map_err(json_error)?;
        self.worker.state_digest()
    }
    pub fn write_instance_snapshot(
        &mut self,
        instance: usize,
        output: impl Write,
    ) -> Result<(), serde_json::Error> {
        self.load(instance).map_err(json_error)?;
        self.worker.write_snapshot(output)
    }
    /// Host-only transactional restore. A rejected stream cannot modify a slot.
    pub fn restore_instance(&mut self, instance: usize, input: impl BufRead) -> Result<(), String> {
        self.slot(instance).map_err(|e| e.to_string())?;
        if !self.memory.restore_within_budget {
            return Err("gas population restore exceeds budget with reserve".into());
        }
        let restored = ConnectedGasSession::read_snapshot(Arc::clone(&self.worker.model), input)?;
        let start = instance * self.stride;
        self.values[start..start + self.stride].copy_from_slice(&restored.state);
        self.metadata[instance] = SlotState::capture(&restored);
        Ok(())
    }
    fn slot(&self, instance: usize) -> Result<&SlotState, ConnectedPopulationError> {
        self.metadata
            .get(instance)
            .ok_or(ConnectedPopulationError::UnknownInstance)
    }
    fn load(&mut self, instance: usize) -> Result<(), ConnectedPopulationError> {
        let metadata = *self.slot(instance)?;
        let start = instance * self.stride;
        self.worker
            .state
            .copy_from_slice(&self.values[start..start + self.stride]);
        metadata.load(&mut self.worker);
        Ok(())
    }
    fn commit(&mut self, instance: usize) {
        let start = instance * self.stride;
        self.values[start..start + self.stride].copy_from_slice(&self.worker.state);
        self.metadata[instance] = SlotState::capture(&self.worker);
    }
}
fn json_error(error: ConnectedPopulationError) -> serde_json::Error {
    serde_json::Error::io(io::Error::other(error))
}
