use hearthline_physics::flow::Conserved;
use hearthline_physics::{NUMERICAL_CONTRACT, Scalar};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Serialize, Serializer};

use super::super::checkpoint::hash_json;
use super::super::flow::FlowStepCheckpoint;
use super::super::{PhysicsProofSession, buffers::ProofBuffers};

#[derive(Clone, Copy)]
struct RawFields<'a>(&'a [Scalar]);

impl Serialize for RawFields<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for value in self.0 {
            sequence.serialize_element(&value.raw())?;
        }
        sequence.end()
    }
}

struct ConservedFields<'a>(&'a [Conserved]);

impl Serialize for ConservedFields<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for value in self.0 {
            sequence.serialize_element(&value.components().map(Scalar::raw))?;
        }
        sequence.end()
    }
}

#[derive(Serialize)]
struct MechanicalView<'a> {
    displacement_raw: RawFields<'a>,
    stress_raw: RawFields<'a>,
    iterations: u32,
    residual_raw: i64,
}

#[derive(Serialize)]
struct FlowView<'a> {
    conserved_raw: ConservedFields<'a>,
    balance_error_raw: [i64; 5],
    last_step: Option<FlowStepCheckpoint>,
}

pub(super) struct CheckpointView<'a> {
    model_digest: &'a str,
    step: u32,
    time_us: u64,
    first_field_raw: RawFields<'a>,
    temperature_k_raw: RawFields<'a>,
    balance_error_raw: [i64; 2],
    mechanics: Option<MechanicalView<'a>>,
    flow: Option<FlowView<'a>>,
    diffusion: Option<super::super::DiffusionCheckpoint>,
}

impl<'a> CheckpointView<'a> {
    pub fn new(worker: &'a PhysicsProofSession) -> Self {
        let model = &worker.model;
        let mechanics = worker.mechanics.as_ref().map(|mechanics| {
            let report = mechanics.report();
            MechanicalView {
                displacement_raw: RawFields(mechanics.displacement()),
                stress_raw: RawFields(mechanics.stress()),
                iterations: report.iterations,
                residual_raw: report.residual.raw(),
            }
        });
        Self {
            model_digest: model.digest(),
            step: worker.step,
            time_us: u64::from(worker.step) * model.timestep_us(),
            first_field_raw: RawFields(&worker.field),
            temperature_k_raw: RawFields(&worker.temperature),
            balance_error_raw: worker.balance_error.map(Scalar::raw),
            mechanics,
            diffusion: worker.diffusion,
            flow: match &worker.buffers {
                ProofBuffers::Flow(buffers) => Some(FlowView {
                    conserved_raw: ConservedFields(&buffers.state),
                    balance_error_raw: buffers.metadata().balance_error.map(Scalar::raw),
                    last_step: buffers.metadata().last_step,
                }),
                _ => None,
            },
        }
    }

    fn digest(&self) -> Result<[u8; 64], serde_json::Error> {
        let fields = (
            NUMERICAL_CONTRACT,
            self.model_digest,
            self.step,
            self.time_us,
            self.first_field_raw,
            self.temperature_k_raw,
            self.balance_error_raw,
        );
        if let Some(diffusion) = self.diffusion {
            hash_json(&(fields, &self.mechanics, diffusion))
        } else {
            match (&self.mechanics, &self.flow) {
                (mechanics, Some(flow)) => hash_json(&(fields, mechanics, flow)),
                (Some(mechanics), None) => hash_json(&(fields, mechanics)),
                (None, None) => hash_json(&fields),
            }
        }
    }
}

impl Serialize for CheckpointView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let digest = self.digest().map_err(serde::ser::Error::custom)?;
        let digest = std::str::from_utf8(&digest).map_err(serde::ser::Error::custom)?;
        let count = 6
            + usize::from(self.mechanics.is_some())
            + usize::from(self.flow.is_some())
            + usize::from(self.diffusion.is_some());
        let mut map = serializer.serialize_map(Some(count))?;
        map.serialize_entry("step", &self.step)?;
        map.serialize_entry("time_us", &self.time_us)?;
        map.serialize_entry("first_field_raw", &self.first_field_raw)?;
        map.serialize_entry("temperature_k_raw", &self.temperature_k_raw)?;
        map.serialize_entry("balance_error_raw", &self.balance_error_raw)?;
        if let Some(mechanics) = &self.mechanics {
            map.serialize_entry("mechanics", mechanics)?;
        }
        if let Some(flow) = &self.flow {
            map.serialize_entry("flow", flow)?;
        }
        if let Some(diffusion) = &self.diffusion {
            map.serialize_entry("diffusion", diffusion)?;
        }
        map.serialize_entry("digest", digest)?;
        map.end()
    }
}
