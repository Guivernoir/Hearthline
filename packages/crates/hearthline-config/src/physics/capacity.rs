use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const PHYSICS_CAPACITY_SCHEMA_VERSION: &str = "0.1.0";

/// A resource study, not an executable plant model or geometry specification.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PhysicsCapacityConfig {
    pub schema_version: String,
    pub id: String,
    pub allocation_budget_bytes: u64,
    pub reserve_percent: u8,
    pub snapshot_copies: u32,
    pub profiles: Vec<CapacityProfile>,
    pub populations: Vec<PhysicsPopulation>,
    pub auxiliary: Vec<AuxiliaryStorage>,
}

impl PhysicsCapacityConfig {
    pub fn from_yaml(source: &str) -> Result<Self, serde_yaml_ng::Error> {
        serde_yaml_ng::from_str(source)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapacityProfile {
    pub id: String,
    pub shape: [u32; 3],
    pub kernel: CapacityKernel,
    /// Optional typed proof source. Shape and kernel must agree with it; its
    /// actual sparse geometry and material storage replace the block estimate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<CapacityModelReference>,
    /// Resolution/geometry assumptions must remain visible in the report.
    pub basis: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapacityModelReference {
    pub id: String,
    pub sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CapacityKernel {
    ScalarTransport,
    CompressibleFlow,
    DryingElasticity,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PhysicsPopulation {
    pub id: String,
    pub area: String,
    pub profile: String,
    pub capacity: u64,
    pub workload: PopulationWorkload,
    pub overflow: PopulationOverflow,
    pub basis: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum PopulationWorkload {
    Fixed {
        count: u64,
    },
    /// Each source emits at most one item per minimum cycle. Residence includes
    /// all holds/rework covered by this pool; an unbounded hold requires backpressure.
    Production {
        sources: u32,
        minimum_cycle_us: u64,
        maximum_residence_us: u64,
        simultaneous_burst: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PopulationOverflow {
    RejectStart,
    Backpressure,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuxiliaryStorage {
    pub resource: AuxiliaryResource,
    pub peak_entries: u64,
    pub capacity_entries: u64,
    pub bytes_per_entry: u64,
    pub overflow: StorageOverflow,
    /// Declared envelope only; not a claim that the future runtime was measured.
    pub basis: String,
}

#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum AuxiliaryResource {
    TransferQueues,
    ControllerAndFaultState,
    Historian,
    Projections,
    HostMetadata,
}

impl AuxiliaryResource {
    pub const ALL: [Self; 5] = [
        Self::TransferQueues,
        Self::ControllerAndFaultState,
        Self::Historian,
        Self::Projections,
        Self::HostMetadata,
    ];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum StorageOverflow {
    RejectStart,
    Backpressure,
    StopRecording,
    DropOldest,
}
