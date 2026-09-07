use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{CapacityPlan, ProjectError};

pub const MODEL_LOCK_SCHEMA_VERSION: &str = "0.2.0";
pub const PREVIOUS_MODEL_LOCK_SCHEMA_VERSION: &str = "0.1.0";

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceDigest {
    pub path: String,
    pub kind: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PartitionAssignment {
    pub site: String,
    pub cell: String,
    pub source: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelLock {
    pub schema_version: String,
    pub project_digest: String,
    pub compiler_version: String,
    #[serde(default)]
    pub schema_versions: BTreeMap<String, String>,
    pub sources: Vec<SourceDigest>,
    pub object_digests: Vec<SourceDigest>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub partition_assignments: Vec<PartitionAssignment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "partitions")]
    pub(crate) legacy_partitions: Vec<String>,
    pub generated_catalogs: Vec<SourceDigest>,
    pub capacity: CapacityPlan,
    pub update_reason: String,
}

impl ModelLock {
    pub fn from_json(source: &str) -> Result<Self, ProjectError> {
        let mut lock: Self = serde_json::from_str(source)
            .map_err(|error| ProjectError::Lock(format!("invalid model lock: {error}")))?;
        if lock.schema_version == PREVIOUS_MODEL_LOCK_SCHEMA_VERSION {
            lock.schema_version = MODEL_LOCK_SCHEMA_VERSION.into();
            lock.partition_assignments = lock
                .legacy_partitions
                .drain(..)
                .map(|cell| PartitionAssignment {
                    site: "canonical-project".into(),
                    source: "migrated-lock".into(),
                    cell,
                })
                .collect();
        }
        if lock.schema_version != MODEL_LOCK_SCHEMA_VERSION {
            return Err(ProjectError::Lock(format!(
                "model lock schema {} is unsupported",
                lock.schema_version
            )));
        }
        Ok(lock)
    }

    pub fn to_json(&self) -> Result<String, ProjectError> {
        serde_json::to_string_pretty(self)
            .map(|json| format!("{json}\n"))
            .map_err(|error| ProjectError::Lock(error.to_string()))
    }

    pub fn status_against(&self, expected: &Self) -> ModelLockStatus {
        if self == expected {
            ModelLockStatus::Current
        } else {
            ModelLockStatus::Stale
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelLockStatus {
    Current,
    Missing,
    Stale,
}
