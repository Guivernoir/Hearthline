//! Host-side configuration adapters for Hearthline.
//!
//! Filesystem access, YAML parsing, generated frontend catalogs, and editable
//! source documents live here so the deterministic runtime remains independent
//! from an allocator and operating-system services.

mod appliance;
mod connection;
mod hmi;
mod scenario;
mod service;

pub use appliance::{
    APPLIANCE_SCHEMA_VERSION, ApplianceConfig, ApplicationUpstreamConfig, BehaviorConfig,
    ConfigError, ConfigRepository, DnsRecordConfig, FRONTEND_CATALOG_SCHEMA_VERSION,
    FirewallHaConfig, FirewallHaRole, FirewallZoneConfig, FirstHopConfig, FirstHopProtocol,
    FirstHopRole, FrontendAppliance, FrontendApplianceCatalog, FrontendFirewallHa,
    FrontendFirstHop, FrontendInterface, FrontendLinkAggregation, FrontendLinkAggregationGroup,
    FrontendMultiChassis, FrontendProcessArea, FrontendProcessEquipment, FrontendProcessView,
    FrontendSpanningTree, HttpInspectionRuleConfig, HttpInspectionTargetConfig, HttpMethodConfig,
    HttpSiteConfig, InterfaceConfig, InterfaceMode, Lifecycle, LinkAggregationConfig,
    LinkAggregationGroupConfig, LinkAggregationMode, LinkAggregationProtocol, ListenerConfig,
    LoadedAppliance, MouldControlCabinetConfig, MouldUtilityCabinetConfig,
    MouldUtilityCircuitConfig, MultiChassisConfig, MultiChassisRole, NatTranslationConfig,
    OperatorControlMode, OperatorModeSelectorConfig, OperatorParameterConfig, OperatorRecipeConfig,
    OperatorStationConfig, OperatorStationType, PROCESS_VIEW_SCHEMA_VERSION, PolicyAction,
    PolicyRuleConfig, ProcessEdge, ProcessPosition, ProcessSupportNode, ProcessViewConfig,
    RUNTIME_CAPACITY_SCHEMA_VERSION, RenderBinding, RenderMode, RobotArchitectureConfig,
    RobotFrameConfig, RobotHandoffConfig, RobotMotionProfileConfig, RobotPayloadConfig,
    RobotPoseConfig, RobotTaughtPositionConfig, RobotToolConfig, RobotWorkspaceConfig, RouteConfig,
    RuntimeCapacityManifest, RuntimePartitionRule, RuntimePartitioningConfig,
    RuntimeWorkloadEnvelope, SpanningTreeConfig, SpanningTreeProtocol, SupervisoryAssetConfig,
    SupervisoryDeploymentNodeConfig, SupervisoryHistoryConfig, SupervisoryIdentityConfig,
    SupervisoryNodeRoleConfig, SupervisoryNodeStateConfig, SupervisoryProfileConfig,
    SupervisoryRepositoryConfig, SupervisoryRoleConfig, SupervisoryTemplateConfig,
    UtilityMediumConfig, canonical_source_text, source_revision,
};
pub use connection::{
    CONNECTION_SCHEMA_VERSION, ConnectionConfig, ConnectionDirection, ConnectionEndpoint,
    ConnectionEndpoints, ConnectionProperties, ConnectionRepository, FrontendConnection,
    FrontendConnectionEndpoint, LoadedConnection, TransportKind,
};
pub use hmi::{
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
pub use scenario::{
    LoadedScenario, SCENARIO_SCHEMA_VERSION, ScenarioApplicationConfig, ScenarioConfig,
    ScenarioConnectionOverride, ScenarioConnectionState, ScenarioContinuityConfig,
    ScenarioContinuityFault, ScenarioExpectation, ScenarioExpectationMode, ScenarioExpectedOutcome,
    ScenarioFirewallHaOverride, ScenarioFirewallHaState, ScenarioFirstHopOverride,
    ScenarioFirstHopState, ScenarioHaIsolationConfig, ScenarioHttpMethod,
    ScenarioLinkAggregationState, ScenarioLocalAutonomyConfig, ScenarioPacketConfig,
    ScenarioRecoveryConfig, ScenarioRepository, ScenarioSecurityConfig, ScenarioSpanningTreeState,
    ScenarioSummary, ScenarioTransportConfig, SecuritySeverity, SpanningTreePortRole,
    SpanningTreePortState, TelemetryIdentity, retarget_telemetry_packet, telemetry_identity,
};
#[doc(hidden)]
pub use scenario::{
    LocalControlTopology, local_control_topology, scenario_connection_states,
    scenario_firewall_ha_states, scenario_first_hop_states, scenario_link_aggregation_states,
    scenario_spanning_tree_states,
};
pub use service::{parse_service_kind, service_name};
