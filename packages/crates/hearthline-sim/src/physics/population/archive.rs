use std::io::{self, BufRead, Write};
use std::sync::Arc;

use hearthline_physics::NUMERICAL_CONTRACT;
use hearthline_project::physics::{CompiledPhysicsProof, PhysicsStorageLayout};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::super::checkpoint::{HashWriter, hex_digest, read_line};
use super::super::{PHYSICS_REPLAY_DOCUMENT_LIMIT, PhysicsCheckpoint};
use super::checkpoint::CheckpointView;
use super::{PhysicsProofPopulation, PopulationMemoryPlan};

pub const POPULATION_SNAPSHOT_SCHEMA: &str = "0.1.0";
const HEADER_LIMIT: usize = 4096;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Header<'a> {
    #[serde(borrow)]
    schema_version: &'a str,
    #[serde(borrow)]
    numerical_contract: &'a str,
    #[serde(borrow)]
    model_digest: &'a str,
    instances: u64,
    values_per_instance: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Footer<'a> {
    #[serde(borrow)]
    sha256: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct SnapshotLoadMemoryPlan {
    pub population: PopulationMemoryPlan,
    pub maximum_frame_bytes: u64,
    pub decoder_envelope_bytes: u64,
    pub total_numerical_bytes: u64,
    pub within_budget: bool,
}

impl SnapshotLoadMemoryPlan {
    pub fn compile(model: &CompiledPhysicsProof, instances: u64) -> Result<Self, String> {
        Self::for_layout(
            model.storage_layout(),
            instances,
            model.config().allocation_budget_bytes,
        )
    }

    pub(super) fn for_layout(
        layout: PhysicsStorageLayout,
        instances: u64,
        budget: u64,
    ) -> Result<Self, String> {
        let population = PopulationMemoryPlan::for_layout(layout, instances, budget)?;
        // Signed i64 decimal, separators, nested conserved arrays and fixed metadata.
        let maximum_frame_bytes = layout
            .checkpoint_field_bytes
            .checked_div(8)
            .and_then(|n| n.checked_mul(22))
            .and_then(|n| n.checked_add(HEADER_LIMIT as u64))
            .ok_or("physics snapshot frame size overflow")?;
        if maximum_frame_bytes > PHYSICS_REPLAY_DOCUMENT_LIMIT {
            return Err(
                "physics population snapshot frame exceeds the 16 MiB adapter limit".into(),
            );
        }
        // One bounded source frame plus transient owned serde integer arrays.
        // Integer tokens need at least two source bytes (value and delimiter);
        // charge 8x the frame for geometric Vec capacity and decoding overhead.
        let decoder_envelope_bytes = maximum_frame_bytes
            .checked_mul(9)
            .and_then(|n| n.checked_add(2 * HEADER_LIMIT as u64))
            .ok_or("physics snapshot decoder size overflow")?;
        let total_numerical_bytes = population
            .total_numerical_bytes
            .checked_add(decoder_envelope_bytes)
            .ok_or("physics snapshot load size overflow")?;
        Ok(Self {
            maximum_frame_bytes,
            decoder_envelope_bytes,
            total_numerical_bytes,
            within_budget: total_numerical_bytes <= population.reviewed_budget_bytes,
            population,
        })
    }
}

impl PhysicsProofPopulation {
    /// Emits a header, one existing-format checkpoint per instance, and a footer
    /// hashing all preceding bytes. Valid capture allocates no temporary fields.
    pub fn write_snapshot(&mut self, output: impl Write) -> Result<(), serde_json::Error> {
        let model = &self.worker.model;
        let memory = SnapshotLoadMemoryPlan::compile(model, self.instance_count() as u64)
            .map_err(|error| serde_json::Error::io(io::Error::other(error)))?;
        if !memory.within_budget {
            return Err(serde_json::Error::io(io::Error::other(
                "physics snapshot cannot be restored within its reviewed budget",
            )));
        }
        let mut output = HashWriter::new(output);
        serde_json::to_writer(
            &mut output,
            &Header {
                schema_version: POPULATION_SNAPSHOT_SCHEMA,
                numerical_contract: NUMERICAL_CONTRACT,
                model_digest: model.digest(),
                instances: self.instance_count() as u64,
                values_per_instance: model.storage_layout().checkpoint_field_bytes / 8,
            },
        )?;
        output.write_all(b"\n").map_err(serde_json::Error::io)?;
        for instance in 0..self.instance_count() {
            self.load(instance)
                .map_err(|error| serde_json::Error::io(io::Error::other(error)))?;
            let view = CheckpointView::new(&self.worker);
            serde_json::to_writer(&mut output, &view)?;
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

    /// Load a new, unpublished population. No live bank is modified on failure.
    /// Only one bounded source row and its decoded checkpoint are retained.
    pub fn read_snapshot(
        model: Arc<CompiledPhysicsProof>,
        mut input: impl BufRead,
    ) -> Result<Self, String> {
        let mut row = Vec::new();
        row.try_reserve_exact(HEADER_LIMIT)
            .map_err(|e| e.to_string())?;
        read_line(&mut input, &mut row, HEADER_LIMIT)?;
        let mut hash = Sha256::new();
        hash.update(&row);
        let header: Header<'_> = serde_json::from_slice(&row).map_err(|e| e.to_string())?;
        if header.schema_version != POPULATION_SNAPSHOT_SCHEMA
            || header.numerical_contract != NUMERICAL_CONTRACT
            || header.model_digest != model.digest()
            || header.values_per_instance != model.storage_layout().checkpoint_field_bytes / 8
        {
            return Err("physics population snapshot model or contract mismatch".into());
        }
        let count = usize::try_from(header.instances).map_err(|e| e.to_string())?;
        let memory = SnapshotLoadMemoryPlan::compile(&model, header.instances)?;
        if !memory.within_budget {
            return Err("physics snapshot load exceeds its budget with 25% reserve".into());
        }
        let mut population = Self::new(model, count)?;
        row.clear();
        let maximum = usize::try_from(memory.maximum_frame_bytes).map_err(|e| e.to_string())?;
        row.try_reserve_exact(maximum).map_err(|e| e.to_string())?;
        for instance in 0..count {
            read_line(&mut input, &mut row, maximum)?;
            hash.update(&row);
            let checkpoint: PhysicsCheckpoint =
                serde_json::from_slice(&row).map_err(|e| e.to_string())?;
            population
                .restore(instance, &checkpoint)
                .map_err(|e| format!("physics snapshot instance {instance}: {e}"))?;
        }
        read_line(&mut input, &mut row, HEADER_LIMIT)?;
        let footer: Footer<'_> = serde_json::from_slice(&row).map_err(|e| e.to_string())?;
        if footer.sha256.as_bytes() != hex_digest(hash.finalize().into()) {
            return Err("physics population snapshot aggregate digest mismatch".into());
        }
        if !input.fill_buf().map_err(|e| e.to_string())?.is_empty() {
            return Err("physics population snapshot has trailing data".into());
        }
        Ok(population)
    }
}
