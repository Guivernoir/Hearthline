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

    pub fn first_difference(&self, expected: &Self) -> Option<String> {
        for (field, equal) in [
            (
                "schema_version",
                self.schema_version == expected.schema_version,
            ),
            (
                "compiler_version",
                self.compiler_version == expected.compiler_version,
            ),
            (
                "schema_versions",
                self.schema_versions == expected.schema_versions,
            ),
            (
                "partition_assignments",
                self.partition_assignments == expected.partition_assignments,
            ),
            ("capacity", self.capacity == expected.capacity),
            (
                "update_reason",
                self.update_reason == expected.update_reason,
            ),
        ] {
            if !equal {
                return Some(field.into());
            }
        }
        digest_difference("sources", &self.sources, &expected.sources)
            .or_else(|| {
                digest_difference(
                    "object_digests",
                    &self.object_digests,
                    &expected.object_digests,
                )
            })
            .or_else(|| {
                digest_difference(
                    "generated_catalogs",
                    &self.generated_catalogs,
                    &expected.generated_catalogs,
                )
            })
            .or_else(|| {
                (self.project_digest != expected.project_digest).then(|| "project_digest".into())
            })
    }
}

fn digest_difference(
    field: &str,
    locked: &[SourceDigest],
    compiled: &[SourceDigest],
) -> Option<String> {
    if locked.len() != compiled.len() {
        return Some(format!(
            "{field}.length (locked {}, compiled {})",
            locked.len(),
            compiled.len()
        ));
    }
    locked
        .iter()
        .zip(compiled)
        .enumerate()
        .find_map(|(index, (locked, compiled))| {
            if locked == compiled {
                None
            } else {
                Some(format!(
                    "{field}[{index}] (locked {}:{} {}, compiled {}:{} {})",
                    locked.kind,
                    locked.path,
                    locked.sha256,
                    compiled.kind,
                    compiled.path,
                    compiled.sha256
                ))
            }
        })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelLockStatus {
    Current,
    Missing,
    Stale,
}
