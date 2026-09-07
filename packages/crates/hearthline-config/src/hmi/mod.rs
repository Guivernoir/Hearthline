mod profile_validation;
mod schema;
mod validation;

pub use schema::{
    FORMING_PHASES, GLAZE_PREPARATION_PHASES, HMI_SCHEMA_VERSION, HmiAction, HmiActionReport,
    HmiActionStatus, HmiActuator, HmiAlarm, HmiAlarmSeverity, HmiAuditEntry,
    HmiBodyIngredientState, HmiBodyPreparationPipelineState, HmiBodyPreparationState,
    HmiBodyQualityCheck, HmiCellGuardState, HmiControlMode, HmiControlProgramDocument,
    HmiControlProgramState, HmiControlStation, HmiDownstreamMaterialEffects,
    HmiGlazePreparationState, HmiGuardedCellState, HmiHandoffPipelineState, HmiHandoffStationState,
    HmiMouldControlCabinet, HmiMouldProcessState, HmiMouldUtilityCabinet, HmiMouldUtilityCircuit,
    HmiParameter, HmiPermissive, HmiPreparationTrain, HmiPreparationTrainState, HmiProcessFault,
    HmiProcessPhase, HmiProcessState, HmiRecipe, HmiReturnWaterState, HmiRobotArchitecture,
    HmiRobotAxis, HmiRobotCellState, HmiRobotCoordinateSystem, HmiRobotFrame, HmiRobotHandoff,
    HmiRobotMotionState, HmiRobotPayload, HmiRobotPose, HmiRobotProgramLine, HmiRobotProgramState,
    HmiRobotState, HmiRobotTaughtPosition, HmiRobotTool, HmiRobotWorkspace, HmiSafety, HmiSignal,
    HmiSlipPreparationState, HmiSnapshot, HmiStationStatus, HmiSupervisoryAsset,
    HmiSupervisoryEvent, HmiSupervisoryIdentity, HmiSupervisoryNode, HmiSupervisoryRepository,
    HmiSupervisorySample, HmiSupervisoryState, HmiSupervisoryTag, HmiSupervisoryTemplate,
    HmiTraceEntry, HmiWaterNetworkState, HmiWaterPreparationState, HmiWaterPumpState,
    HmiWaterQuality, HmiWaterRouteState, RETURN_WATER_PHASES, SLIP_PREPARATION_PHASES,
    WATER_PREPARATION_PHASES,
};
pub(crate) use validation::{validate_behavior, validate_repository};
