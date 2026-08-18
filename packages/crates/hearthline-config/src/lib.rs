//! Host-side configuration adapters for Hearthline.
//!
//! Filesystem access, YAML parsing, generated frontend catalogs, and editable
//! source documents live here so the deterministic runtime remains independent
//! from an allocator and operating-system services.

mod appliance;
mod connection;
mod hmi;
mod runtime;
mod scenario;
mod workstation;

pub use appliance::{
    APPLIANCE_SCHEMA_VERSION, ApplianceConfig, ApplicationUpstreamConfig, BehaviorConfig,
    ConfigError, ConfigRepository, DnsRecordConfig, FRONTEND_CATALOG_SCHEMA_VERSION,
    FirewallHaConfig, FirewallHaRole, FirewallZoneConfig, FirstHopConfig, FirstHopProtocol,
    FirstHopRole, FrontendAppliance, FrontendApplianceCatalog, FrontendFirewallHa,
    FrontendFirstHop, FrontendInterface, FrontendLinkAggregation, FrontendLinkAggregationGroup,
    FrontendMultiChassis, FrontendSpanningTree, HttpInspectionRuleConfig,
    HttpInspectionTargetConfig, HttpMethodConfig, HttpSiteConfig, InterfaceConfig, InterfaceMode,
    Lifecycle, LinkAggregationConfig, LinkAggregationGroupConfig, LinkAggregationMode,
    LinkAggregationProtocol, ListenerConfig, LoadedAppliance, MouldControlCabinetConfig,
    MouldUtilityCabinetConfig, MouldUtilityCircuitConfig, MultiChassisConfig, MultiChassisRole,
    NatTranslationConfig, OperatorControlMode, OperatorModeSelectorConfig, OperatorParameterConfig,
    OperatorRecipeConfig, OperatorStationConfig, OperatorStationType, PolicyAction,
    PolicyRuleConfig, RenderBinding, RenderMode, RobotArchitectureConfig, RobotFrameConfig,
    RobotHandoffConfig, RobotMotionProfileConfig, RobotPayloadConfig, RobotPoseConfig,
    RobotTaughtPositionConfig, RobotToolConfig, RobotWorkspaceConfig, RouteConfig,
    SpanningTreeConfig, SpanningTreeProtocol, SupervisoryAssetConfig,
    SupervisoryDeploymentNodeConfig, SupervisoryHistoryConfig, SupervisoryIdentityConfig,
    SupervisoryNodeRoleConfig, SupervisoryNodeStateConfig, SupervisoryProfileConfig,
    SupervisoryRepositoryConfig, SupervisoryRoleConfig, SupervisoryTemplateConfig,
    UtilityMediumConfig,
};
pub use connection::{
    CONNECTION_SCHEMA_VERSION, ConnectionConfig, ConnectionDirection, ConnectionEndpoint,
    ConnectionEndpoints, ConnectionProperties, ConnectionRepository, FrontendConnection,
    FrontendConnectionEndpoint, LoadedConnection, TransportKind,
};
pub use hmi::{
    HMI_SCHEMA_VERSION, HmiAction, HmiActionReport, HmiActionStatus, HmiActuator, HmiAlarm,
    HmiAlarmSeverity, HmiAuditEntry, HmiBodyIngredientState, HmiBodyPreparationState,
    HmiBodyQualityCheck, HmiCellGuardState, HmiControlMode, HmiControlProgramDocument,
    HmiControlProgramState, HmiControlStation, HmiDownstreamMaterialEffects,
    HmiGlazePreparationState, HmiGuardedCellState, HmiHandoffStationState, HmiMouldProcessState,
    HmiParameter, HmiPermissive, HmiPreparationTrain, HmiPreparationTrainState, HmiProcessFault,
    HmiProcessPhase, HmiProcessState, HmiRecipe, HmiReturnWaterState, HmiRobotAxis,
    HmiRobotCoordinateSystem, HmiRobotMotionState, HmiRobotPose, HmiRobotProgramLine,
    HmiRobotProgramState, HmiRobotState, HmiRobotTaughtPosition, HmiRobotWorkspace, HmiSafety,
    HmiSession, HmiSessionStore, HmiSignal, HmiSlipPreparationState, HmiSnapshot, HmiStationStatus,
    HmiTraceEntry, HmiWaterPreparationState, HmiWaterQuality, build_forming_telemetry_packet,
};
pub use runtime::{
    ConfiguredAppliance, ConfiguredNetwork, RuntimeDeviceSnapshot, RuntimeFirewallSessionEntry,
    RuntimeMacEntry, RuntimeNeighborEntry, RuntimePatEntry,
};
pub use scenario::{
    InteractiveScenarioSession, LoadedScenario, SCENARIO_REPORT_SCHEMA_VERSION,
    SCENARIO_SCHEMA_VERSION, SECURITY_EVENT_SCHEMA_VERSION, ScenarioApplicationConfig,
    ScenarioConfig, ScenarioConnectionOverride, ScenarioConnectionState, ScenarioContinuityConfig,
    ScenarioContinuityFault, ScenarioContinuityReport, ScenarioExpectation,
    ScenarioExpectationMode, ScenarioExpectedOutcome, ScenarioFirewallHaOverride,
    ScenarioFirewallHaState, ScenarioFirstHopOverride, ScenarioFirstHopState,
    ScenarioHaIsolationConfig, ScenarioHaIsolationReport, ScenarioHttpDocument, ScenarioHttpMethod,
    ScenarioHttpResponse, ScenarioLinkAggregationState, ScenarioLocalAutonomyConfig,
    ScenarioLocalAutonomyReport, ScenarioPacketConfig, ScenarioRecoveryConfig, ScenarioReport,
    ScenarioRepository, ScenarioSecurityConfig, ScenarioSecurityEvent, ScenarioSpanningTreeState,
    ScenarioStatistics, ScenarioStatus, ScenarioSummary, ScenarioTraceEntry, ScenarioTraceKind,
    ScenarioTransportConfig, SecurityDisposition, SecuritySeverity, SpanningTreePortRole,
    SpanningTreePortState, run_scenario, run_scenario_with_overrides,
    run_scenario_with_state_overrides,
};
pub use workstation::{
    BrowserNavigation, WORKSTATION_DNS_TTL_MS, WORKSTATION_SCHEMA_VERSION, WorkstationAction,
    WorkstationActionKind, WorkstationActionReport, WorkstationActionStatus, WorkstationArpEntry,
    WorkstationDnsCacheEntry, WorkstationInterface, WorkstationNetworkState, WorkstationProfile,
    WorkstationSession, run_workstation_action, run_workstation_action_with_session,
    workstation_profile,
};
