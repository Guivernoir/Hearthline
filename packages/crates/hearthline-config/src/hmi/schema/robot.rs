use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HmiRobotPose {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
    pub p: f64,
    pub r: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiRobotCoordinateSystem {
    World,
    Joint,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiRobotAxis {
    X,
    Y,
    Z,
    W,
    P,
    R,
    J1,
    J2,
    J3,
    J4,
    J5,
    J6,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiRobotMotionState {
    pub active: bool,
    pub kind: &'static str,
    pub progress_percent: f64,
    pub elapsed_ms: u64,
    pub duration_ms: u64,
    pub speed_percent: f64,
    pub target_pose: HmiRobotPose,
    pub target_joints: [f64; 6],
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiRobotProgramLine {
    pub number: u16,
    pub source: String,
    pub operation: Option<String>,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiRobotProgramState {
    pub name: String,
    pub source_path: String,
    pub revision: String,
    pub running: bool,
    pub paused: bool,
    pub active_line: Option<u16>,
    pub cycle_count: u64,
    pub source: String,
    pub lines: Vec<HmiRobotProgramLine>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiRobotTaughtPosition {
    pub id: String,
    pub label: String,
    pub pose: HmiRobotPose,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiRobotWorkspace {
    pub minimum: HmiRobotPose,
    pub maximum: HmiRobotPose,
    pub joint_minimum: [f64; 6],
    pub joint_maximum: [f64; 6],
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiRobotArchitecture {
    pub controller: String,
    pub manipulator: String,
    pub pendant: String,
    pub safety_interface: String,
    pub cell_controller: String,
    pub servo_axes: u8,
    pub motion_group: String,
    pub interpolation_cycle_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiRobotFrame {
    pub id: String,
    pub label: String,
    pub parent: Option<String>,
    pub pose: HmiRobotPose,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiRobotPayload {
    pub id: String,
    pub label: String,
    pub mass_kg: f64,
    pub center_of_mass_mm: [f64; 3],
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiRobotTool {
    pub id: String,
    pub label: String,
    pub tcp: HmiRobotPose,
    pub payload: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiRobotHandoff {
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiRobotCellState {
    pub active_mould: Option<String>,
    pub queued_moulds: Vec<String>,
    pub stage: &'static str,
    pub completed_handoffs: u64,
    pub active_program: Option<String>,
    pub fault_code: Option<String>,
    pub fault_message: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiRobotState {
    pub coordinate_system: &'static str,
    pub motion_enabled: bool,
    pub pose: HmiRobotPose,
    pub joints: [f64; 6],
    pub gripper_closed: bool,
    pub automatic_command: String,
    pub controller_state: &'static str,
    pub active_user_frame: String,
    pub active_tool: String,
    pub active_payload: String,
    pub architecture: HmiRobotArchitecture,
    pub frames: Vec<HmiRobotFrame>,
    pub payloads: Vec<HmiRobotPayload>,
    pub tools: Vec<HmiRobotTool>,
    pub handoffs: Vec<HmiRobotHandoff>,
    pub cell: HmiRobotCellState,
    pub motion: HmiRobotMotionState,
    pub program: HmiRobotProgramState,
    pub taught_positions: Vec<HmiRobotTaughtPosition>,
    pub workspace: HmiRobotWorkspace,
}
