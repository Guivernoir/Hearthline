use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperatorStationType {
    MachinePc,
    MouldPanel,
    RobotJoystick,
}

impl std::fmt::Display for OperatorStationType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::MachinePc => "machine-pc",
            Self::MouldPanel => "mould-panel",
            Self::RobotJoystick => "robot-joystick",
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperatorControlMode {
    Manual,
    Auto,
    Setup,
}

impl std::fmt::Display for OperatorControlMode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Manual => "manual",
            Self::Auto => "auto",
            Self::Setup => "setup",
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorModeSelectorConfig {
    pub positions: Vec<OperatorControlMode>,
    pub initial_position: OperatorControlMode,
    #[serde(default)]
    pub setup_password_sha256: Option<String>,
    #[serde(default)]
    pub bypassed_permissives: Vec<String>,
    #[serde(default)]
    pub retained_protections: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorStationConfig {
    pub station_type: OperatorStationType,
    pub target: String,
    #[serde(default)]
    pub mode_selector: Option<OperatorModeSelectorConfig>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorParameterConfig {
    pub id: String,
    pub label: String,
    pub target: String,
    pub unit: String,
    pub minimum: f64,
    pub maximum: f64,
    pub step: f64,
    pub initial_value: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorRecipeConfig {
    pub id: String,
    pub label: String,
    pub description: String,
}
