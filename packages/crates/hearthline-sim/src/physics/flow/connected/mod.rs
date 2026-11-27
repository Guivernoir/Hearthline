//! Exclusive runtime for one jointly solved gas island. No plant state is
//! shared with a client, and no accepted step allocates or partially publishes.

mod archive;
pub use archive::CONNECTED_SNAPSHOT_SCHEMA;
pub mod population;

use crate::physics::{FieldRange, NumericalFieldRanges};
use hearthline_physics::PhysicsError;
use hearthline_physics::flow::connected::{
    CellInventory, ConnectedAdvanceError, ConnectedAdvanceReport, ConnectedAdvanceWorkspace,
    ConnectedReport, ConnectedWorkspace,
};
use hearthline_project::physics::flow::connected::CompiledConnectedGas;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectedStepError {
    Complete,
    Failed(PhysicsError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectedSessionMemory {
    pub runtime_bytes: u64,
    pub snapshot_decoder_bytes: u64,
    pub restore_bytes: u64,
    pub reviewed_budget_bytes: u64,
}

impl ConnectedSessionMemory {
    pub fn compile(model: &CompiledConnectedGas) -> Result<Self, String> {
        let runtime_bytes = model
            .memory()
            .runtime_bytes
            .checked_add(size_of::<ConnectedGasSession>() as u64 + 2 * size_of::<usize>() as u64)
            .ok_or("connected session memory overflow")?;
        let snapshot_decoder_bytes = archive::DECODER_BYTES as u64;
        let restore_bytes = runtime_bytes
            .checked_add(snapshot_decoder_bytes)
            .ok_or("connected restore memory overflow")?;
        let reviewed_budget_bytes =
            u64::try_from(u128::from(model.memory().allocation_budget_bytes) * 3 / 4)
                .map_err(|e| e.to_string())?;
        if runtime_bytes > reviewed_budget_bytes {
            return Err("connected session exceeds budget with reserve".into());
        }
        Ok(Self {
            runtime_bytes,
            snapshot_decoder_bytes,
            restore_bytes,
            reviewed_budget_bytes,
        })
    }
}

pub struct ConnectedGasSession {
    model: Arc<CompiledConnectedGas>,
    state: Box<[CellInventory]>,
    candidate: Box<[CellInventory]>,
    trial: Box<[CellInventory]>,
    next: Box<[CellInventory]>,
    step: u32,
    exchange: CellInventory,
    last_report: Option<ConnectedReport>,
    last_advance: Option<ConnectedAdvanceReport>,
    failure: Option<PhysicsError>,
    retry_failure: Option<ConnectedAdvanceError>,
    ranges: NumericalFieldRanges,
    memory: ConnectedSessionMemory,
}

impl ConnectedGasSession {
    pub fn new(model: Arc<CompiledConnectedGas>) -> Result<Self, String> {
        let memory = ConnectedSessionMemory::compile(&model)?;
        let count = usize::try_from(model.mesh().memory().cells).map_err(|e| e.to_string())?;
        let mut state = buffer(count)?;
        model.initialize(&mut state).map_err(|e| e.to_string())?;
        let ranges = project(&model, &state).map_err(|e| e.to_string())?;
        Ok(Self {
            model,
            state,
            candidate: buffer(count)?,
            trial: buffer(count)?,
            next: buffer(count)?,
            step: 0,
            exchange: CellInventory::default(),
            last_report: None,
            last_advance: None,
            failure: None,
            retry_failure: None,
            ranges,
            memory,
        })
    }
    pub fn model(&self) -> &CompiledConnectedGas {
        &self.model
    }
    pub fn fields(&self) -> &[CellInventory] {
        &self.state
    }
    pub const fn memory(&self) -> &ConnectedSessionMemory {
        &self.memory
    }
    pub const fn step(&self) -> u32 {
        self.step
    }
    pub fn time_us(&self) -> u64 {
        u64::from(self.step) * self.model.profile().timestep_us
    }
    pub fn complete(&self) -> bool {
        self.step == self.model.profile().steps
    }
    pub fn next_time_us(&self) -> Option<u64> {
        (!self.complete()).then(|| (u64::from(self.step) + 1) * self.model.profile().timestep_us)
    }
    pub const fn failure(&self) -> Option<PhysicsError> {
        self.failure
    }
    pub const fn ranges(&self) -> NumericalFieldRanges {
        self.ranges
    }
    pub const fn exchange(&self) -> CellInventory {
        self.exchange
    }
    /// Last implicit substep, not the cumulative exchange of a retried horizon.
    pub const fn last_report(&self) -> Option<ConnectedReport> {
        self.last_report
    }
    pub const fn last_advance(&self) -> Option<ConnectedAdvanceReport> {
        self.last_advance
    }
    pub const fn retry_failure(&self) -> Option<ConnectedAdvanceError> {
        self.retry_failure
    }

    pub fn step_next(&mut self) -> Result<ConnectedAdvanceReport, ConnectedStepError> {
        if let Some(error) = self.failure {
            return Err(ConnectedStepError::Failed(error));
        }
        if self.complete() {
            return Err(ConnectedStepError::Complete);
        }
        match self.try_step() {
            Ok(report) => Ok(report),
            Err(error) => {
                self.failure = Some(error);
                Err(ConnectedStepError::Failed(error))
            }
        }
    }
    fn try_step(&mut self) -> Result<ConnectedAdvanceReport, PhysicsError> {
        let solver = self.model.solver()?;
        let (report, advance, step_exchange) = if let Some(policy) = self.model.retries() {
            let result = solver.advance(
                self.model.solver_dt(),
                policy,
                &self.state,
                &mut ConnectedAdvanceWorkspace {
                    candidate: &mut self.candidate,
                    trial: &mut self.trial,
                    next: &mut self.next,
                },
            );
            let advance = result.map_err(|error| {
                self.retry_failure = Some(error);
                error.cause
            })?;
            (advance.last, Some(advance), advance.boundary_exchange)
        } else {
            self.candidate.copy_from_slice(&self.state);
            let report = solver.solve(
                self.model.solver_dt(),
                &mut self.candidate,
                &mut ConnectedWorkspace {
                    trial: &mut self.trial,
                    next: &mut self.next,
                },
            )?;
            (report, None, report.boundary_exchange)
        };
        let exchange = add(self.exchange, step_exchange)?;
        let expected = add(self.model.initial_inventory(), exchange)?;
        if solver.inventory(&self.candidate)? != expected {
            return Err(PhysicsError::InvalidState);
        }
        let ranges = project(&self.model, &self.candidate)?;
        self.state.copy_from_slice(&self.candidate);
        self.step += 1;
        self.exchange = exchange;
        self.ranges = ranges;
        self.last_report = Some(report);
        self.last_advance = advance;
        Ok(advance.unwrap_or(ConnectedAdvanceReport {
            completed: self.model.solver_dt(),
            substeps: 1,
            attempts: 1,
            accepted_iterations: u64::from(report.iterations),
            accepted_evaluations: report.evaluations,
            maximum_residual: report.residual,
            boundary_exchange: report.boundary_exchange,
            last: report,
        }))
    }
}

fn buffer(count: usize) -> Result<Box<[CellInventory]>, String> {
    let mut field = Vec::new();
    field.try_reserve_exact(count).map_err(|e| e.to_string())?;
    field.resize(count, CellInventory::default());
    Ok(field.into_boxed_slice())
}
fn add(a: CellInventory, b: CellInventory) -> Result<CellInventory, PhysicsError> {
    let mut values = a.components();
    for (value, b) in values.iter_mut().zip(b.components()) {
        *value = value.checked_add(b)?;
    }
    Ok(CellInventory::from_components(values))
}
fn project(
    model: &CompiledConnectedGas,
    state: &[CellInventory],
) -> Result<NumericalFieldRanges, PhysicsError> {
    let mut ranges = [FieldRange {
        minimum_raw: i64::MAX,
        maximum_raw: i64::MIN,
    }; 2];
    for region in model.mesh().regions() {
        let volume = region.grid.volume()?;
        for cell in &state[region.offset..region.offset + region.grid.count()] {
            for (range, value) in ranges.iter_mut().zip(model.readings(*cell, volume)?) {
                range.minimum_raw = range.minimum_raw.min(value.raw());
                range.maximum_raw = range.maximum_raw.max(value.raw());
            }
        }
    }
    Ok(NumericalFieldRanges {
        first_field: ranges[0],
        temperature_k: ranges[1],
    })
}
