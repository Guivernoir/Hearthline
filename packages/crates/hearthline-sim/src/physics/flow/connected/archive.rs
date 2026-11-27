use hearthline_physics::flow::connected::{
    CONNECTED_ADVANCE_CONTRACT, CONNECTED_FLOW_CONTRACT, CellInventory, ConnectedAdvanceError,
    ConnectedAdvanceReport, ConnectedReport,
};
use hearthline_physics::{NUMERICAL_CONTRACT, Scalar};
use hearthline_project::physics::flow::connected::CompiledConnectedGas;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, BufRead, Write};
use std::sync::Arc;

use super::{ConnectedGasSession, ConnectedSessionMemory, add, project};
use crate::physics::checkpoint::{HashWriter, NumericalCause, hex_digest, read_line};

pub const CONNECTED_SNAPSHOT_SCHEMA: &str = "0.2.0";
pub(super) const ROW_LIMIT: usize = 4096;
// One row, up to two rows of JSON string scratch, two concurrently formatted
// errors at two rows each, and one row of fixed decoder/error overhead. DTOs
// contain only borrowed strings, fixed arrays and direct-map numerical causes.
pub(super) const DECODER_BYTES: usize = 8 * ROW_LIMIT;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Report {
    iterations: u32,
    evaluations: u64,
    residual_raw: i64,
    boundary_exchange_raw: [i64; 5],
}
impl From<ConnectedReport> for Report {
    fn from(report: ConnectedReport) -> Self {
        Self {
            iterations: report.iterations,
            evaluations: report.evaluations,
            residual_raw: report.residual.raw(),
            boundary_exchange_raw: raw(report.boundary_exchange),
        }
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Advance {
    completed_raw: i64,
    substeps: u32,
    attempts: u32,
    accepted_iterations: u64,
    accepted_evaluations: u64,
    maximum_residual_raw: i64,
    boundary_exchange_raw: [i64; 5],
}
impl From<ConnectedAdvanceReport> for Advance {
    fn from(r: ConnectedAdvanceReport) -> Self {
        Self {
            completed_raw: r.completed.raw(),
            substeps: r.substeps,
            attempts: r.attempts,
            accepted_iterations: r.accepted_iterations,
            accepted_evaluations: r.accepted_evaluations,
            maximum_residual_raw: r.maximum_residual.raw(),
            boundary_exchange_raw: raw(r.boundary_exchange),
        }
    }
}
impl Advance {
    fn report(self, last: ConnectedReport) -> ConnectedAdvanceReport {
        ConnectedAdvanceReport {
            completed: Scalar::from_raw(self.completed_raw),
            substeps: self.substeps,
            attempts: self.attempts,
            accepted_iterations: self.accepted_iterations,
            accepted_evaluations: self.accepted_evaluations,
            maximum_residual: Scalar::from_raw(self.maximum_residual_raw),
            boundary_exchange: inventory(self.boundary_exchange_raw),
            last,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RetryFailure {
    cause: NumericalCause,
    attempts: u32,
    substeps: u32,
    completed_substeps: u32,
    smallest_step_raw: i64,
}
impl From<ConnectedAdvanceError> for RetryFailure {
    fn from(e: ConnectedAdvanceError) -> Self {
        Self {
            cause: e.cause.into(),
            attempts: e.attempts,
            substeps: e.substeps,
            completed_substeps: e.completed_substeps,
            smallest_step_raw: e.smallest_step.raw(),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Header<'a> {
    #[serde(borrow)]
    schema_version: &'a str,
    #[serde(borrow)]
    numerical_contract: &'a str,
    #[serde(borrow)]
    flow_contract: &'a str,
    #[serde(borrow)]
    model_digest: &'a str,
    cells: u64,
    step: u32,
    time_us: u64,
    exchange_raw: [i64; 5],
    last_report: Option<Report>,
    failure: Option<NumericalCause>,
    #[serde(borrow)]
    advance_contract: &'a str,
    advance: Option<Advance>,
    retry_failure: Option<RetryFailure>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreviousHeader<'a> {
    #[serde(borrow)]
    schema_version: &'a str,
    #[serde(borrow)]
    numerical_contract: &'a str,
    #[serde(borrow)]
    flow_contract: &'a str,
    #[serde(borrow)]
    model_digest: &'a str,
    cells: u64,
    step: u32,
    time_us: u64,
    exchange_raw: [i64; 5],
    last_report: Option<Report>,
    failure: Option<NumericalCause>,
}

fn decode_header<'a>(row: &'a [u8], model: &CompiledConnectedGas) -> Result<Header<'a>, String> {
    #[derive(Deserialize)]
    struct Version<'a> {
        #[serde(borrow)]
        schema_version: &'a str,
    }
    let version: Version<'_> = serde_json::from_slice(row).map_err(|e| e.to_string())?;
    match version.schema_version {
        CONNECTED_SNAPSHOT_SCHEMA => serde_json::from_slice(row).map_err(|e| e.to_string()),
        "0.1.0" if model.retries().is_none() => {
            let h: PreviousHeader<'_> = serde_json::from_slice(row).map_err(|e| e.to_string())?;
            Ok(Header {
                schema_version: h.schema_version,
                numerical_contract: h.numerical_contract,
                flow_contract: h.flow_contract,
                model_digest: h.model_digest,
                cells: h.cells,
                step: h.step,
                time_us: h.time_us,
                exchange_raw: h.exchange_raw,
                last_report: h.last_report,
                failure: h.failure,
                advance_contract: CONNECTED_ADVANCE_CONTRACT,
                advance: None,
                retry_failure: None,
            })
        }
        _ => Err("unsupported connected snapshot schema/model combination".into()),
    }
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Cell {
    index: u64,
    inventory_raw: [i64; 5],
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Footer<'a> {
    #[serde(borrow)]
    sha256: &'a str,
}

fn raw(value: CellInventory) -> [i64; 5] {
    value.components().map(Scalar::raw)
}
fn inventory(raw: [i64; 5]) -> CellInventory {
    CellInventory::from_components(raw.map(Scalar::from_raw))
}
fn json_io(message: impl Into<String>) -> serde_json::Error {
    serde_json::Error::io(io::Error::other(message.into()))
}

impl ConnectedGasSession {
    fn write_fields(
        &self,
        output: impl Write,
    ) -> Result<(impl Write, [u8; 64]), serde_json::Error> {
        let mut writer = HashWriter::new(output);
        serde_json::to_writer(
            &mut writer,
            &Header {
                schema_version: CONNECTED_SNAPSHOT_SCHEMA,
                numerical_contract: NUMERICAL_CONTRACT,
                flow_contract: CONNECTED_FLOW_CONTRACT,
                model_digest: self.model.digest(),
                cells: self.state.len() as u64,
                step: self.step,
                time_us: self.time_us(),
                exchange_raw: raw(self.exchange),
                last_report: self.last_report.map(Report::from),
                failure: self.failure.map(NumericalCause::from),
                advance_contract: CONNECTED_ADVANCE_CONTRACT,
                advance: self.last_advance.map(Advance::from),
                retry_failure: self.retry_failure.map(RetryFailure::from),
            },
        )?;
        writer.write_all(b"\n").map_err(serde_json::Error::io)?;
        for (index, value) in self.state.iter().enumerate() {
            serde_json::to_writer(
                &mut writer,
                &Cell {
                    index: index as u64,
                    inventory_raw: raw(*value),
                },
            )?;
            writer.write_all(b"\n").map_err(serde_json::Error::io)?;
        }
        Ok(writer.finish())
    }
    pub fn state_digest(&self) -> Result<[u8; 64], serde_json::Error> {
        Ok(self.write_fields(io::sink())?.1)
    }
    pub fn write_snapshot(&self, output: impl Write) -> Result<(), serde_json::Error> {
        self.write_snapshot_digest(output).map(|_| ())
    }
    pub(super) fn write_snapshot_digest(
        &self,
        output: impl Write,
    ) -> Result<[u8; 64], serde_json::Error> {
        if self.memory.restore_bytes > self.memory.reviewed_budget_bytes {
            return Err(json_io(
                "connected snapshot cannot be restored inside budget",
            ));
        }
        let (mut output, digest) = self.write_fields(output)?;
        serde_json::to_writer(
            &mut output,
            &Footer {
                sha256: std::str::from_utf8(&digest).expect("hex digest"),
            },
        )?;
        output.write_all(b"\n").map_err(serde_json::Error::io)?;
        Ok(digest)
    }
    pub fn read_snapshot(
        model: Arc<CompiledConnectedGas>,
        mut input: impl BufRead,
    ) -> Result<Self, String> {
        let memory = ConnectedSessionMemory::compile(&model)?;
        if memory.restore_bytes > memory.reviewed_budget_bytes {
            return Err("connected snapshot restore exceeds reviewed budget".into());
        }
        let mut row = Vec::new();
        row.try_reserve_exact(ROW_LIMIT)
            .map_err(|e| e.to_string())?;
        read_line(&mut input, &mut row, ROW_LIMIT)?;
        let mut hash = Sha256::new();
        hash.update(&row);
        let header = decode_header(&row, &model)?;
        if header.numerical_contract != NUMERICAL_CONTRACT
            || header.flow_contract != CONNECTED_FLOW_CONTRACT
            || header.advance_contract != CONNECTED_ADVANCE_CONTRACT
            || header.model_digest != model.digest()
            || header.cells != model.mesh().memory().cells
            || header.step > model.profile().steps
            || header.time_us != u64::from(header.step) * model.profile().timestep_us
        {
            return Err("connected snapshot model, contract or clock mismatch".into());
        }
        let (step, exchange, report, failure, advance, retry_failure) = (
            header.step,
            inventory(header.exchange_raw),
            header.last_report,
            header.failure,
            header.advance,
            header.retry_failure,
        );
        validate_report(&model, step, report)?;
        validate_advance(&model, step, report, advance)?;
        if retry_failure.is_some_and(|r| model.retries().is_none() || Some(r.cause) != failure) {
            return Err("connected snapshot retry failure/model mismatch".into());
        }
        let step_exchange = advance
            .map(|a| a.boundary_exchange_raw)
            .or(report.map(|r| r.boundary_exchange_raw));
        if step == 1 && step_exchange.is_some_and(|raw| inventory(raw) != exchange) {
            return Err("connected first-step and cumulative ledgers differ".into());
        }
        let mut session = Self::new(model)?;
        for (index, field) in session.state.iter_mut().enumerate() {
            read_line(&mut input, &mut row, ROW_LIMIT)?;
            hash.update(&row);
            let cell: Cell = serde_json::from_slice(&row).map_err(|e| e.to_string())?;
            if cell.index != index as u64 {
                return Err("connected snapshot cell order mismatch".into());
            }
            *field = inventory(cell.inventory_raw);
        }
        read_line(&mut input, &mut row, ROW_LIMIT)?;
        let footer: Footer<'_> = serde_json::from_slice(&row).map_err(|e| e.to_string())?;
        if footer.sha256.as_bytes() != hex_digest(hash.finalize().into()) {
            return Err("connected snapshot digest mismatch".into());
        }
        if !input.fill_buf().map_err(|e| e.to_string())?.is_empty() {
            return Err("connected snapshot trailing data".into());
        }
        if step == 0 {
            session
                .model
                .initialize(&mut session.candidate)
                .map_err(|e| e.to_string())?;
            if session.state != session.candidate || exchange != CellInventory::default() {
                return Err("connected initial snapshot differs from model".into());
            }
        }
        let expected =
            add(session.model.initial_inventory(), exchange).map_err(|e| e.to_string())?;
        if session
            .model
            .solver()
            .map_err(|e| e.to_string())?
            .inventory(&session.state)
            .map_err(|e| e.to_string())?
            != expected
        {
            return Err("connected snapshot inventory ledger mismatch".into());
        }
        session.ranges = project(&session.model, &session.state).map_err(|e| e.to_string())?;
        session.step = step;
        session.exchange = exchange;
        session.last_report = report.map(|r| ConnectedReport {
            iterations: r.iterations,
            evaluations: r.evaluations,
            residual: Scalar::from_raw(r.residual_raw),
            boundary_exchange: inventory(r.boundary_exchange_raw),
            balance_error: CellInventory::default(),
        });
        session.last_advance = match (advance, session.last_report) {
            (Some(advance), Some(last)) => Some(advance.report(last)),
            _ => None,
        };
        if let Some(expected) = failure {
            let Err(super::ConnectedStepError::Failed(actual)) = session.step_next() else {
                return Err("connected snapshot failure does not recur".into());
            };
            if NumericalCause::from(actual) != expected
                || session.retry_failure.map(RetryFailure::from) != retry_failure
            {
                return Err("connected snapshot failure diagnostic differs".into());
            }
        }
        Ok(session)
    }
}

fn validate_report(
    model: &CompiledConnectedGas,
    step: u32,
    report: Option<Report>,
) -> Result<(), String> {
    if (step == 0) != report.is_none() {
        return Err("connected snapshot report/step mismatch".into());
    }
    if let Some(report) = report
        && (report.iterations == 0
            || report.iterations > model.profile().limits.iterations
            || report.evaluations <= u64::from(report.iterations)
            || report.evaluations > 2 * u64::from(report.iterations)
            || report.residual_raw < 0
            || report.residual_raw > model.profile().limits.residual.raw())
    {
        return Err("connected snapshot solver report exceeds its contract".into());
    }
    Ok(())
}

fn validate_advance(
    model: &CompiledConnectedGas,
    step: u32,
    last: Option<Report>,
    advance: Option<Advance>,
) -> Result<(), String> {
    if advance.is_some() != (step > 0 && model.retries().is_some()) {
        return Err("connected snapshot retry report/model mismatch".into());
    }
    let (Some(policy), Some(advance), Some(last)) = (model.retries(), advance, last) else {
        return Ok(());
    };
    if !advance.substeps.is_power_of_two()
        || advance.substeps > policy.maximum_substeps
        || advance.attempts != advance.substeps.trailing_zeros() + 1
        || advance.completed_raw != model.solver_dt().raw()
        || advance.completed_raw / i64::from(advance.substeps) < policy.minimum_step.raw()
        || advance.maximum_residual_raw < last.residual_raw
        || advance.maximum_residual_raw > model.profile().limits.residual.raw()
    {
        return Err("connected snapshot retry horizon exceeds its contract".into());
    }
    let others = u128::from(advance.substeps - 1);
    let iterations = u128::from(advance.accepted_iterations);
    let evaluations = u128::from(advance.accepted_evaluations);
    if iterations < u128::from(last.iterations) + others
        || iterations
            > u128::from(last.iterations) + others * u128::from(model.profile().limits.iterations)
    {
        return Err("connected snapshot retry iteration count exceeds its contract".into());
    }
    let other_iterations = iterations - u128::from(last.iterations);
    if evaluations < u128::from(last.evaluations) + other_iterations + others
        || evaluations > u128::from(last.evaluations) + 2 * other_iterations
        || advance.accepted_evaluations > model.evaluation_bound()
        || (advance.substeps == 1
            && (advance.boundary_exchange_raw != last.boundary_exchange_raw
                || advance.maximum_residual_raw != last.residual_raw
                || advance.accepted_evaluations != last.evaluations))
    {
        return Err("connected snapshot retry work/ledger exceeds its contract".into());
    }
    Ok(())
}
