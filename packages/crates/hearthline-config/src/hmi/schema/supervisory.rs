use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiSupervisoryState {
    pub namespace: String,
    pub model_id: String,
    pub repository: HmiSupervisoryRepository,
    pub templates: Vec<HmiSupervisoryTemplate>,
    pub assets: Vec<HmiSupervisoryAsset>,
    pub deployment_nodes: Vec<HmiSupervisoryNode>,
    pub identity: HmiSupervisoryIdentity,
    pub tags: Vec<HmiSupervisoryTag>,
    pub events: Vec<HmiSupervisoryEvent>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiSupervisoryRepository {
    pub id: String,
    pub revision: String,
    pub deployed_revision: String,
    pub synchronized: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiSupervisoryTemplate {
    pub id: String,
    pub label: String,
    pub parent: Option<String>,
    pub attributes: Vec<String>,
    pub alarm_capable: bool,
    pub history_capable: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiSupervisoryAsset {
    pub id: String,
    pub label: String,
    pub template: String,
    pub parent: Option<String>,
    pub components: Vec<String>,
    pub historized_tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiSupervisoryNode {
    pub id: String,
    pub label: String,
    pub host: String,
    pub role: String,
    pub state: String,
    pub redundancy_group: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiSupervisoryIdentity {
    pub user: String,
    pub role: String,
    pub authentication: String,
    pub permissions: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiSupervisoryTag {
    pub tag: String,
    pub value: f64,
    pub unit: String,
    pub quality: &'static str,
    pub timestamp_ms: u64,
    pub samples: Vec<HmiSupervisorySample>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiSupervisorySample {
    pub timestamp_ms: u64,
    pub value: f64,
    pub quality_good: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiSupervisoryEvent {
    pub sequence: u64,
    pub category: &'static str,
    pub source: String,
    pub message: String,
    pub state: String,
}
