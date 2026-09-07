use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectionStatus {
    Current,
    Stale,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorProjection {
    pub model_revision: String,
    pub captured_at_us: u64,
    pub status: ProjectionStatus,
    pub target: String,
    pub state: BTreeMap<String, i64>,
    pub labels: BTreeMap<String, String>,
    pub active_alarms: Vec<String>,
}
