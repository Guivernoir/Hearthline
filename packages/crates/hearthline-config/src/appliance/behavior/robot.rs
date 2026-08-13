use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RobotPoseConfig {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
    pub p: f64,
    pub r: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RobotWorkspaceConfig {
    pub minimum: RobotPoseConfig,
    pub maximum: RobotPoseConfig,
    pub joint_minimum: [f64; 6],
    pub joint_maximum: [f64; 6],
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RobotTaughtPositionConfig {
    pub id: String,
    pub label: String,
    pub pose: RobotPoseConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RobotArchitectureConfig {
    pub manipulator: String,
    pub pendant: String,
    pub safety_interface: String,
    pub cell_controller: String,
    pub servo_axes: u8,
    pub motion_group: String,
    pub interpolation_cycle_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RobotFrameConfig {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub parent: Option<String>,
    pub pose: RobotPoseConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RobotPayloadConfig {
    pub id: String,
    pub label: String,
    pub mass_kg: f64,
    pub center_of_mass_mm: [f64; 3],
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RobotToolConfig {
    pub id: String,
    pub label: String,
    pub tcp: RobotPoseConfig,
    pub payload: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RobotHandoffConfig {
    pub mould: String,
    pub program: String,
    pub user_frame: String,
    pub approach_position: String,
    pub pickup_position: String,
    pub handoff_position: String,
    pub retreat_position: String,
    pub pickup_tolerance_mm: f64,
    pub handoff_tolerance_mm: f64,
    pub orientation_tolerance_deg: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RobotMotionProfileConfig {
    pub architecture: RobotArchitectureConfig,
    pub program_ref: String,
    pub max_linear_speed_mm_s: f64,
    pub max_joint_speed_deg_s: f64,
    pub default_speed_percent: f64,
    pub workspace: RobotWorkspaceConfig,
    pub home: RobotPoseConfig,
    pub taught_positions: Vec<RobotTaughtPositionConfig>,
    pub frames: Vec<RobotFrameConfig>,
    pub payloads: Vec<RobotPayloadConfig>,
    pub tools: Vec<RobotToolConfig>,
    pub active_user_frame: String,
    pub active_tool: String,
    pub active_payload: String,
    pub handoffs: Vec<RobotHandoffConfig>,
}
