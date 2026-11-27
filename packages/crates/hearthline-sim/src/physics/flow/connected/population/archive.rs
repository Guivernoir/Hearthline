use std::io::{self, BufRead, Read, Write};
use std::sync::Arc;

use hearthline_project::physics::flow::connected::CompiledConnectedGas;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{ConnectedGasPopulation, ConnectedPopulationMemory, json_error};
use crate::physics::checkpoint::{hex_digest, read_line};

pub const CONNECTED_POPULATION_SCHEMA: &str = "0.1.0";
const CONTRACT: &str = "connected-gas-population/1:ordered-slot-digests:shared-serial-workspace";
const ROW_LIMIT: usize = super::super::archive::ROW_LIMIT;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Header<'a> {
    #[serde(borrow)]
    schema_version: &'a str,
    #[serde(borrow)]
    population_contract: &'a str,
    #[serde(borrow)]
    normalized_child_schema: &'a str,
    #[serde(borrow)]
    model_digest: &'a str,
    instances: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    instance: u64,
    bytes: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Footer<'a> {
    #[serde(borrow)]
    sha256: &'a str,
}

impl ConnectedGasPopulation {
    fn root_hash(&self) -> Sha256 {
        let mut hash = Sha256::new();
        hash.update(CONTRACT);
        hash.update(self.model().digest());
        hash.update((self.instance_count() as u64).to_be_bytes());
        hash
    }
    /// Normalized ordered child-state hashes, not a hash of transport framing.
    pub fn state_digest(&mut self) -> Result<[u8; 64], serde_json::Error> {
        let mut hash = self.root_hash();
        for instance in 0..self.instance_count() {
            hash.update((instance as u64).to_be_bytes());
            hash.update(self.instance_digest(instance)?);
        }
        Ok(hex_digest(hash.finalize().into()))
    }
    /// Streams length-delimited, independently validated child snapshots. Two
    /// serialization passes determine framing without retaining a spatial copy.
    pub fn write_snapshot(&mut self, mut output: impl Write) -> Result<(), serde_json::Error> {
        if !self.memory.restore_within_budget {
            return Err(json_io(
                "gas population snapshot restore exceeds budget with reserve",
            ));
        }
        write_row(
            &mut output,
            &Header {
                schema_version: CONNECTED_POPULATION_SCHEMA,
                population_contract: CONTRACT,
                normalized_child_schema: super::super::CONNECTED_SNAPSHOT_SCHEMA,
                model_digest: self.model().digest(),
                instances: self.instance_count() as u64,
            },
        )?;
        let mut hash = self.root_hash();
        for instance in 0..self.instance_count() {
            self.load(instance).map_err(json_error)?;
            let mut counter = ByteCounter(0);
            self.worker.write_snapshot(&mut counter)?;
            if counter.0 > self.memory.maximum_frame_bytes {
                return Err(json_io("gas population frame exceeds compiled bound"));
            }
            write_row(
                &mut output,
                &Frame {
                    instance: instance as u64,
                    bytes: counter.0,
                },
            )?;
            let digest = self.worker.write_snapshot_digest(&mut output)?;
            hash.update((instance as u64).to_be_bytes());
            hash.update(digest);
        }
        let digest = hex_digest(hash.finalize().into());
        write_row(
            &mut output,
            &Footer {
                sha256: std::str::from_utf8(&digest).expect("hexadecimal digest"),
            },
        )
    }
    pub fn read_snapshot(
        model: Arc<CompiledConnectedGas>,
        mut input: impl BufRead,
    ) -> Result<Self, String> {
        let mut row = Vec::new();
        row.try_reserve_exact(ROW_LIMIT)
            .map_err(|e| e.to_string())?;
        read_line(&mut input, &mut row, ROW_LIMIT)?;
        let header: Header<'_> = serde_json::from_slice(&row).map_err(|e| e.to_string())?;
        if header.schema_version != CONNECTED_POPULATION_SCHEMA
            || header.population_contract != CONTRACT
            || header.normalized_child_schema != super::super::CONNECTED_SNAPSHOT_SCHEMA
            || header.model_digest != model.digest()
        {
            return Err("gas population snapshot model or contract mismatch".into());
        }
        let count = usize::try_from(header.instances).map_err(|e| e.to_string())?;
        let memory = ConnectedPopulationMemory::compile(&model, header.instances)?;
        if !memory.restore_within_budget {
            return Err("gas population snapshot load exceeds budget with reserve".into());
        }
        let mut result = Self::new(model, count)?;
        let mut hash = result.root_hash();
        for instance in 0..count {
            read_line(&mut input, &mut row, ROW_LIMIT)?;
            let frame: Frame = serde_json::from_slice(&row).map_err(|e| e.to_string())?;
            if frame.instance != instance as u64
                || frame.bytes == 0
                || frame.bytes > memory.maximum_frame_bytes
            {
                return Err("gas population snapshot frame order or size mismatch".into());
            }
            let mut frame_input = (&mut input).take(frame.bytes);
            result.restore_instance(instance, &mut frame_input)?;
            if frame_input.limit() != 0 {
                return Err("truncated gas population snapshot frame".into());
            }
            hash.update((instance as u64).to_be_bytes());
            hash.update(
                result
                    .instance_digest(instance)
                    .map_err(|e| e.to_string())?,
            );
        }
        read_line(&mut input, &mut row, ROW_LIMIT)?;
        let footer: Footer<'_> = serde_json::from_slice(&row).map_err(|e| e.to_string())?;
        if footer.sha256.as_bytes() != hex_digest(hash.finalize().into()) {
            return Err("gas population snapshot state digest mismatch".into());
        }
        if !input.fill_buf().map_err(|e| e.to_string())?.is_empty() {
            return Err("gas population snapshot trailing data".into());
        }
        Ok(result)
    }
}

fn write_row(output: &mut impl Write, row: &impl Serialize) -> Result<(), serde_json::Error> {
    serde_json::to_writer(&mut *output, row)?;
    output.write_all(b"\n").map_err(serde_json::Error::io)
}
fn json_io(message: &str) -> serde_json::Error {
    serde_json::Error::io(io::Error::other(message))
}

struct ByteCounter(u64);
impl Write for ByteCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| io::Error::other("gas snapshot byte count overflow"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
