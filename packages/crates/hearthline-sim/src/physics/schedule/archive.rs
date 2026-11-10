use std::cmp::Reverse;
use std::io::{self, BufRead, Write};
use std::sync::Arc;

use hearthline_project::physics::schedule::{CompiledPhysicsSchedule, PHYSICS_SCHEDULE_CONTRACT};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::diagnostic::FailureRecord;
use super::{PhysicsEvent, PhysicsSchedule, ScheduleError, ScheduleMemoryPlan};
use crate::physics::PhysicsCheckpoint;
use crate::physics::checkpoint::{HashWriter, hex_digest, read_line};

pub const SCHEDULE_SNAPSHOT_SCHEMA: &str = "0.1.0";
const HEADER_LIMIT: usize = 4096;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Header<'a> {
    #[serde(borrow)]
    schema_version: &'a str,
    #[serde(borrow)]
    schedule_contract: &'a str,
    #[serde(borrow)]
    model_digest: &'a str,
    instances: usize,
    time_us: u64,
    committed_events: u64,
    failed_event: Option<FailureRecord>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Footer<'a> {
    #[serde(borrow)]
    sha256: &'a str,
}

impl PhysicsSchedule {
    /// Captures every spatial field in instance identity order. Valid output to
    /// a preallocated/streaming writer does not allocate on the simulation side.
    pub fn write_snapshot(&mut self, output: impl Write) -> Result<(), serde_json::Error> {
        if !self.memory.snapshot_within_budget {
            return Err(serde_json::Error::io(io::Error::other(
                "physics schedule snapshot cannot be restored within its reviewed budget",
            )));
        }
        let mut output = HashWriter::new(output);
        serde_json::to_writer(
            &mut output,
            &Header {
                schema_version: SCHEDULE_SNAPSHOT_SCHEMA,
                schedule_contract: PHYSICS_SCHEDULE_CONTRACT,
                model_digest: self.plan.digest(),
                instances: self.plan.instances().len(),
                time_us: self.time_us,
                committed_events: self.committed_events,
                failed_event: self.failure.map(FailureRecord::from),
            },
        )?;
        output.write_all(b"\n").map_err(serde_json::Error::io)?;
        for binding in self.plan.instances() {
            self.populations[binding.profile].write_checkpoint(binding.slot, &mut output)?;
            output.write_all(b"\n").map_err(serde_json::Error::io)?;
        }
        let (mut output, digest) = output.finish();
        serde_json::to_writer(
            &mut output,
            &Footer {
                sha256: std::str::from_utf8(&digest).expect("hexadecimal digest"),
            },
        )?;
        output.write_all(b"\n").map_err(serde_json::Error::io)
    }

    pub fn state_digest(&mut self) -> Result<[u8; 64], serde_json::Error> {
        let mut writer = HashWriter::new(io::sink());
        self.write_snapshot(&mut writer)?;
        Ok(writer.finish().1)
    }

    /// Restores an unpublished runtime. It verifies that per-instance clocks
    /// form one chronological prefix, not merely individually valid snapshots.
    pub fn read_snapshot(
        plan: Arc<CompiledPhysicsSchedule>,
        mut input: impl BufRead,
    ) -> Result<Self, String> {
        let memory = ScheduleMemoryPlan::compile(&plan)?;
        if !memory.snapshot_within_budget {
            return Err("physics schedule snapshot load exceeds its reviewed budget".into());
        }
        let mut row = Vec::new();
        row.try_reserve_exact(HEADER_LIMIT)
            .map_err(|e| e.to_string())?;
        read_line(&mut input, &mut row, HEADER_LIMIT)?;
        let mut hash = Sha256::new();
        hash.update(&row);
        let header: Header<'_> = serde_json::from_slice(&row).map_err(|e| e.to_string())?;
        if header.schema_version != SCHEDULE_SNAPSHOT_SCHEMA
            || header.schedule_contract != PHYSICS_SCHEDULE_CONTRACT
            || header.model_digest != plan.digest()
            || header.instances != plan.instances().len()
            || header.committed_events > plan.total_events()
            || header.time_us > plan.horizon_us()
        {
            return Err("physics schedule snapshot model, contract or bounds mismatch".into());
        }
        let (time_us, events, failure) =
            (header.time_us, header.committed_events, header.failed_event);
        let mut restored = Self::new(plan)?;
        row.clear();
        let maximum =
            usize::try_from(memory.maximum_snapshot_frame_bytes).map_err(|e| e.to_string())?;
        row.try_reserve_exact(maximum).map_err(|e| e.to_string())?;
        for binding in restored.plan.instances() {
            read_line(&mut input, &mut row, maximum)?;
            hash.update(&row);
            let checkpoint: PhysicsCheckpoint =
                serde_json::from_slice(&row).map_err(|e| e.to_string())?;
            restored.populations[binding.profile].restore(binding.slot, &checkpoint)?;
        }
        read_line(&mut input, &mut row, HEADER_LIMIT)?;
        let footer: Footer<'_> = serde_json::from_slice(&row).map_err(|e| e.to_string())?;
        if footer.sha256.as_bytes() != hex_digest(hash.finalize().into()) {
            return Err("physics schedule snapshot aggregate digest mismatch".into());
        }
        if !input.fill_buf().map_err(|e| e.to_string())?.is_empty() {
            return Err("physics schedule snapshot has trailing data".into());
        }
        restored.restore_frontier(time_us, events)?;
        if let Some(failure) = failure {
            if restored.next_event() != Some(failure.event) {
                return Err("physics schedule failure is not its next event".into());
            }
            // Diagnostics are derived from the pinned numerical model. A saved
            // fault must actually recur; an arbitrary halted flag is not trusted.
            if !matches!(restored.step_next(), Err(ScheduleError::Failed(f)) if FailureRecord::from(f) == failure)
            {
                return Err("physics schedule saved numerical failure did not recur".into());
            }
        }
        Ok(restored)
    }

    fn restore_frontier(&mut self, time_us: u64, events: u64) -> Result<(), String> {
        self.pending.clear();
        let mut latest = None;
        let mut count = 0_u64;
        for (instance, binding) in self.plan.instances().iter().enumerate() {
            let step = self.populations[binding.profile]
                .completed_steps(binding.slot)
                .map_err(|e| e.to_string())?;
            let model = &self.plan.profiles()[binding.profile];
            count = count
                .checked_add(u64::from(step))
                .ok_or("physics schedule snapshot count overflow")?;
            if step > 0 {
                let event = PhysicsEvent {
                    time_us: u64::from(step) * model.timestep_us(),
                    instance,
                };
                latest = Some(latest.map_or(event, |previous: PhysicsEvent| previous.max(event)));
            }
            if step < model.config().steps {
                self.pending.push(Reverse(PhysicsEvent {
                    time_us: (u64::from(step) + 1) * model.timestep_us(),
                    instance,
                }));
            }
        }
        if count != events
            || latest.map_or(0, |event| event.time_us) != time_us
            || latest
                .zip(self.next_event())
                .is_some_and(|(last, next)| last >= next)
        {
            return Err("physics schedule snapshot is not a committed chronological prefix".into());
        }
        self.time_us = time_us;
        self.committed_events = events;
        Ok(())
    }
}
