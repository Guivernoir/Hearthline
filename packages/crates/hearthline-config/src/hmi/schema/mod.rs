use serde::{Deserialize, Serialize};

mod body_preparation;
mod cell;
mod operator;
mod phases;
mod robot;
mod supervisory;

pub use cell::{HmiCellGuardState, HmiGuardedCellState, HmiHandoffStationState};
pub use operator::{HmiControlMode, HmiControlStation, HmiParameter, HmiRecipe, HmiStationStatus};
pub use robot::{
    HmiRobotArchitecture, HmiRobotAxis, HmiRobotCellState, HmiRobotCoordinateSystem, HmiRobotFrame,
    HmiRobotHandoff, HmiRobotMotionState, HmiRobotPayload, HmiRobotPose, HmiRobotProgramLine,
    HmiRobotProgramState, HmiRobotState, HmiRobotTaughtPosition, HmiRobotTool, HmiRobotWorkspace,
};
pub use supervisory::{
    HmiSupervisoryAsset, HmiSupervisoryEvent, HmiSupervisoryIdentity, HmiSupervisoryNode,
    HmiSupervisoryRepository, HmiSupervisorySample, HmiSupervisoryState, HmiSupervisoryTag,
    HmiSupervisoryTemplate,
};

pub const HMI_SCHEMA_VERSION: &str = "0.9.0";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiSnapshot {
    pub schema_version: &'static str,
    pub id: String,
    pub label: String,
    pub environment: String,
    pub zone: String,
    pub role: String,
    pub interface_kind: String,
    pub controller: String,
    pub remote_io: String,
    pub remote_io_stations: Vec<String>,
    pub permissions: Vec<String>,
    pub sequence: u64,
    pub control_program: Option<HmiControlProgramState>,
    pub control_station: Option<HmiControlStation>,
    pub station_status: Vec<HmiStationStatus>,
    pub parameters: Vec<HmiParameter>,
    pub recipes: Vec<HmiRecipe>,
    pub active_recipe: Option<String>,
    pub process: Option<HmiProcessState>,
    pub body_preparation: Option<HmiBodyPreparationState>,
    pub moulds: Vec<HmiMouldProcessState>,
    pub robot: Option<HmiRobotState>,
    pub guarded_cell: Option<HmiGuardedCellState>,
    pub supervisory: Option<HmiSupervisoryState>,
    pub signals: Vec<HmiSignal>,
    pub actuators: Vec<HmiActuator>,
    pub safety: Vec<HmiSafety>,
    pub alarms: Vec<HmiAlarm>,
    pub audit: Vec<HmiAuditEntry>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiMouldProcessState {
    pub target: String,
    pub label: String,
    pub phase: &'static str,
    pub operating_state: &'static str,
    pub running: bool,
    pub production_enabled: bool,
    pub paused: bool,
    pub stop_request: Option<&'static str>,
    pub phase_elapsed_ms: u64,
    pub scan_count: u64,
    pub cycle_count: u64,
    pub fault: Option<&'static str>,
    pub target_duration_ms: u64,
    pub casting_pressure_bar: f64,
    pub setpoints_bound: bool,
    pub control_cabinet: Option<HmiMouldControlCabinet>,
    pub utility_cabinet: Option<HmiMouldUtilityCabinet>,
    pub phases: &'static [HmiProcessPhase],
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiMouldControlCabinet {
    pub remote_io: String,
    pub enclosure_rating: String,
    pub control_voltage_vdc: u16,
    pub safety_relay: String,
    pub modules: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiMouldUtilityCircuit {
    pub id: String,
    pub label: String,
    pub medium: String,
    pub source: String,
    pub nominal_pressure: Option<f64>,
    pub state: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiMouldUtilityCabinet {
    pub actuator: String,
    pub enclosure_rating: String,
    pub control_voltage_vdc: u16,
    pub isolation_state: String,
    pub active_state: String,
    pub circuits: Vec<HmiMouldUtilityCircuit>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiControlProgramState {
    pub language: &'static str,
    pub program: String,
    pub task: String,
    pub source_path: String,
    pub binding_path: String,
    pub revision: String,
    pub current_step: i64,
    pub scan_interval_ms: u64,
    pub watchdog_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiControlProgramDocument {
    pub schema_version: &'static str,
    pub controller: String,
    pub language: &'static str,
    pub program: String,
    pub task: String,
    pub source_path: String,
    pub binding_path: String,
    pub revision: String,
    pub source: String,
    pub binding_yaml: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiProcessState {
    pub model: &'static str,
    pub phase: &'static str,
    pub running: bool,
    pub phase_elapsed_ms: u64,
    pub scan_count: u64,
    pub cycle_count: u64,
    pub fault: Option<&'static str>,
    pub phases: &'static [HmiProcessPhase],
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiProcessPhase {
    pub key: &'static str,
    pub label: &'static str,
}

pub(super) use phases::{
    FORMING_PHASES, GLAZE_PREPARATION_PHASES, RETURN_WATER_PHASES, SLIP_PREPARATION_PHASES,
    WATER_PREPARATION_PHASES,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiSignal {
    pub component_id: String,
    pub label: String,
    pub tag: String,
    pub unit: String,
    pub minimum: f64,
    pub maximum: f64,
    pub value: f64,
    pub quality_good: bool,
    pub timestamp_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiActuator {
    pub component_id: String,
    pub label: String,
    pub command_tag: String,
    pub feedback_tag: Option<String>,
    pub safe_state: String,
    pub states: Vec<String>,
    pub current_state: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiSafety {
    pub component_id: String,
    pub label: String,
    pub permissives: Vec<HmiPermissive>,
    pub trip_latched: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiPermissive {
    pub tag: String,
    pub satisfied: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiAlarm {
    pub id: String,
    pub code: String,
    pub source: String,
    pub message: String,
    pub severity: HmiAlarmSeverity,
    pub active: bool,
    pub acknowledged: bool,
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiAlarmSeverity {
    Warning,
    Trip,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiAuditEntry {
    pub sequence: u64,
    pub action: String,
    pub target: String,
    pub result: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum HmiAction {
    Command {
        tag: String,
        value: String,
    },
    StartProcess,
    HoldProcess,
    StartPreparationTrain {
        train: HmiPreparationTrain,
    },
    HoldPreparationTrain {
        train: HmiPreparationTrain,
    },
    SetWaterPumpFailure {
        pump_id: String,
        failed: bool,
    },
    DispatchWaterPumpMaintenance {
        pump_id: String,
    },
    StartMould,
    StopMouldAfterPhase,
    EndMouldAfterCycle,
    ResetProcess,
    SetProcessFault {
        fault: HmiProcessFault,
        active: bool,
    },
    ResetSafety {
        safety_id: String,
    },
    SetGuardDoor {
        open: bool,
    },
    AcknowledgeAlarm {
        alarm_id: String,
    },
    SetControlMode {
        mode: HmiControlMode,
        #[serde(default)]
        password: Option<String>,
    },
    SetParameter {
        parameter_id: String,
        value: f64,
    },
    SelectRecipe {
        recipe_id: String,
    },
    SetRobotMotionEnable {
        enabled: bool,
    },
    MoveRobot {
        target: HmiRobotPose,
        speed_percent: f64,
    },
    MoveRobotToPosition {
        position_id: String,
        speed_percent: f64,
    },
    JogRobot {
        coordinate_system: HmiRobotCoordinateSystem,
        axis: HmiRobotAxis,
        increment: f64,
        speed_percent: f64,
    },
    TeachRobotPosition {
        position_id: String,
        label: String,
    },
    RunRobotProgram,
    PauseRobotProgram,
    StepRobotProgram,
    ResetRobotProgram,
    LoadRobotProgram {
        name: String,
        source: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiProcessFault {
    SlipSupplyLoss,
    CompressedAirLoss,
    MouldOverpressure,
    VacuumLoss,
    RobotPickupFailure,
    IngredientShortage,
    MixerOverload,
    ScreenBlocked,
    QualityOutOfSpec,
    TransferNoFlow,
    RawWaterQuality,
    WaterFilterBlocked,
    ReturnWaterContamination,
    GlazeMillOverload,
    GlazeQualityOutOfSpec,
    SlipPipelineLeak,
    WaterToSlipLeak,
    WaterToGlazeLeak,
    GlazePipelineLeak,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiPreparationTrain {
    Slip,
    Water,
    ReturnWater,
    Glaze,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiActionStatus {
    Applied,
    Completed,
    Denied,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiActionReport {
    pub schema_version: &'static str,
    pub status: HmiActionStatus,
    pub message: String,
    pub trace: Vec<HmiTraceEntry>,
    pub snapshot: HmiSnapshot,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiTraceEntry {
    pub sequence: usize,
    pub component: String,
    pub stage: String,
    pub detail: String,
}
pub use body_preparation::{
    HmiBodyIngredientState, HmiBodyPreparationPipelineState, HmiBodyPreparationState,
    HmiBodyQualityCheck, HmiDownstreamMaterialEffects, HmiGlazePreparationState,
    HmiHandoffPipelineState, HmiPreparationTrainState, HmiReturnWaterState,
    HmiSlipPreparationState, HmiWaterNetworkState, HmiWaterPreparationState, HmiWaterPumpState,
    HmiWaterQuality, HmiWaterRouteState,
};
