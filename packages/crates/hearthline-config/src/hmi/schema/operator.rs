use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiControlMode {
    Manual,
    Auto,
    Setup,
}

impl HmiControlMode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Auto => "auto",
            Self::Setup => "setup",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiControlStation {
    pub station_type: String,
    pub target: String,
    pub positions: Vec<HmiControlMode>,
    pub selected_mode: HmiControlMode,
    pub setup_authenticated: bool,
    pub sensor_bypass_active: bool,
    pub bypassed_permissives: Vec<String>,
    pub retained_protections: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiStationStatus {
    pub station_id: String,
    pub label: String,
    pub station_type: String,
    pub target: String,
    pub selected_mode: HmiControlMode,
    pub setup_authenticated: bool,
    pub sensor_bypass_active: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiParameter {
    pub id: String,
    pub label: String,
    pub target: String,
    pub unit: String,
    pub minimum: f64,
    pub maximum: f64,
    pub step: f64,
    pub value: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiRecipe {
    pub id: String,
    pub label: String,
    pub description: String,
}
