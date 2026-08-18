mod actions;
mod builder;
mod robot;
mod schema;
mod state;
mod validation;

pub use builder::support::build_forming_telemetry_packet;
pub use schema::{
    HMI_SCHEMA_VERSION, HmiAction, HmiActionReport, HmiActionStatus, HmiActuator, HmiAlarm,
    HmiAlarmSeverity, HmiAuditEntry, HmiBodyIngredientState, HmiBodyPreparationPipelineState,
    HmiBodyPreparationState, HmiBodyQualityCheck, HmiCellGuardState, HmiControlMode,
    HmiControlProgramDocument, HmiControlProgramState, HmiControlStation,
    HmiDownstreamMaterialEffects, HmiGlazePreparationState, HmiGuardedCellState,
    HmiHandoffPipelineState, HmiHandoffStationState, HmiMouldControlCabinet, HmiMouldProcessState,
    HmiMouldUtilityCabinet, HmiMouldUtilityCircuit, HmiParameter, HmiPermissive,
    HmiPreparationTrain, HmiPreparationTrainState, HmiProcessFault, HmiProcessPhase,
    HmiProcessState, HmiRecipe, HmiReturnWaterState, HmiRobotArchitecture, HmiRobotAxis,
    HmiRobotCellState, HmiRobotCoordinateSystem, HmiRobotFrame, HmiRobotHandoff,
    HmiRobotMotionState, HmiRobotPayload, HmiRobotPose, HmiRobotProgramLine, HmiRobotProgramState,
    HmiRobotState, HmiRobotTaughtPosition, HmiRobotTool, HmiRobotWorkspace, HmiSafety, HmiSignal,
    HmiSlipPreparationState, HmiSnapshot, HmiStationStatus, HmiSupervisoryAsset,
    HmiSupervisoryEvent, HmiSupervisoryIdentity, HmiSupervisoryNode, HmiSupervisoryRepository,
    HmiSupervisorySample, HmiSupervisoryState, HmiSupervisoryTag, HmiSupervisoryTemplate,
    HmiTraceEntry, HmiWaterNetworkState, HmiWaterPreparationState, HmiWaterPumpState,
    HmiWaterQuality, HmiWaterRouteState,
};
pub use state::{HmiSession, HmiSessionStore};
pub(crate) use validation::{validate_behavior, validate_repository};
