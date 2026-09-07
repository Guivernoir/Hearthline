use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum CapacityResource {
    ProcessPorts,
    ProcessTags,
    HmiCommandTags,
    AppliancePorts,
    SwitchPorts,
    Layer3SwitchPorts,
    UnaddressedPorts,
    CellComponents,
    CellLinks,
    SiteCells,
    InterCellConduits,
    ConduitQueue,
    ScheduledEvents,
    RuntimeObjectBytes,
    RuntimeMemoryBytes,
    RuntimeStackBytes,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapacityStatus {
    Accepted,
    ReviewRequired,
    Rejected,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapacityEvidenceRecord {
    pub source: String,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapacityAssessment {
    pub resource: CapacityResource,
    pub scope: String,
    pub demand: usize,
    pub reviewed_limit: usize,
    pub structural_limit: Option<usize>,
    pub reserve_percent: u8,
    pub overflow: String,
    pub status: CapacityStatus,
    pub evidence: CapacityEvidenceRecord,
}

impl CapacityAssessment {
    pub fn measured(
        resource: CapacityResource,
        scope: impl Into<String>,
        demand: usize,
        reviewed_limit: usize,
        structural_limit: Option<usize>,
        overflow: impl Into<String>,
        evidence: CapacityEvidenceRecord,
    ) -> Self {
        let reserve_basis = structural_limit.unwrap_or(reviewed_limit);
        let reserve_percent = if reserve_basis == 0 || demand > reserve_basis {
            0
        } else {
            ((reserve_basis - demand).saturating_mul(100) / reserve_basis) as u8
        };
        let status =
            if demand > reviewed_limit || structural_limit.is_some_and(|limit| demand > limit) {
                CapacityStatus::Rejected
            } else if reserve_percent < 25 {
                CapacityStatus::ReviewRequired
            } else {
                CapacityStatus::Accepted
            };
        Self {
            resource,
            scope: scope.into(),
            demand,
            reviewed_limit,
            structural_limit,
            reserve_percent,
            overflow: overflow.into(),
            status,
            evidence,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapacityPlan {
    pub assessments: Vec<CapacityAssessment>,
}

impl CapacityPlan {
    pub fn accepted(&self) -> bool {
        self.assessments
            .iter()
            .all(|item| item.status == CapacityStatus::Accepted)
    }

    pub fn rejected(&self) -> impl Iterator<Item = &CapacityAssessment> {
        self.assessments
            .iter()
            .filter(|item| item.status != CapacityStatus::Accepted)
    }
}
