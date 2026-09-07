//! Deterministic host orchestration around allocator-free cell runtimes.

pub use hearthline_config::{
    BehaviorConfig, ConfigError, ConfigRepository, ConnectionRepository, FirewallHaRole,
    FirstHopRole, InterfaceConfig, MouldControlCabinetConfig, MouldUtilityCabinetConfig,
    OperatorControlMode, OperatorStationConfig, RobotHandoffConfig, RobotMotionProfileConfig,
    RobotPoseConfig, RobotWorkspaceConfig, ScenarioApplicationConfig, ScenarioConfig,
    ScenarioConnectionOverride, ScenarioConnectionState, ScenarioContinuityConfig,
    ScenarioContinuityFault, ScenarioExpectation, ScenarioExpectedOutcome,
    ScenarioFirewallHaOverride, ScenarioFirewallHaState, ScenarioFirstHopOverride,
    ScenarioFirstHopState, ScenarioHaIsolationConfig, ScenarioHttpMethod,
    ScenarioLinkAggregationState, ScenarioLocalAutonomyConfig, ScenarioPacketConfig,
    ScenarioRecoveryConfig, ScenarioRepository, ScenarioSecurityConfig, ScenarioSpanningTreeState,
    ScenarioTransportConfig, SecuritySeverity, SpanningTreePortRole, SpanningTreePortState,
    SupervisoryProfileConfig,
};

mod conduit;
mod contract;
mod hmi;
mod replay;
mod runtime;
mod scenario;
mod scheduler;
mod session;
mod snapshot;
mod support;

pub use conduit::{ConduitConfig, ConduitMetrics, ConduitOverflow, ConduitQueue};
pub use contract::{
    CONDUIT_OVERLOAD_SCENARIO, ContractScenarioError, run_conduit_overload_contract,
};
pub use hmi::{
    HmiAction, HmiActionReport, HmiActionStatus, HmiSnapshot, HmiTraceEntry, PlantControlRuntime,
    PlantRuntimeOwnership, PlantRuntimeStore, build_forming_telemetry_packet,
    validate_robot_control_program, validate_structured_text_control_program,
};
pub use replay::{
    QUANTIZATION_CONTRACT_VERSION, REPLAY_SCHEMA_VERSION, ReplayArtifact, ReplayCheckpoint,
    ReplayDivergence, ReplayError, ReplayOutcome, ReplayVerifier, RunInput, RunManifest,
};
pub use runtime::{
    ConfiguredAppliance, ConfiguredNetwork, RuntimeDeviceSnapshot, RuntimeFirewallSessionEntry,
    RuntimeLinkSnapshot, RuntimeMacEntry, RuntimeNeighborEntry, RuntimePatEntry,
};
pub use scenario::{
    InteractiveScenarioSession, SCENARIO_REPORT_SCHEMA_VERSION, SECURITY_EVENT_SCHEMA_VERSION,
    ScenarioContinuityReport, ScenarioExpectationMode, ScenarioHaIsolationReport,
    ScenarioHttpDocument, ScenarioHttpResponse, ScenarioLocalAutonomyReport, ScenarioReport,
    ScenarioRuntimeSnapshots, ScenarioSecurityEvent, ScenarioStateSnapshot, ScenarioStatistics,
    ScenarioStatus, ScenarioTraceEntry, ScenarioTraceKind, SecurityDisposition,
    is_interactive_scenario, run_scenario, run_scenario_with_overrides,
    run_scenario_with_state_overrides,
};
pub use scheduler::{
    CellId, CellIdentity, CellRegistration, ConduitId, ScheduledEnvelope, SchedulerError,
    SchedulerMetrics, SiteId, StableScheduler,
};
pub use session::{
    ModelRevision, SESSION_BUILD_STACK_BYTES, SessionCapacityPolicy, SessionError,
    SimulationSession, SimulationSessionStatus,
};
pub use snapshot::{
    CELL_SNAPSHOT_SCHEMA_VERSION, CellSnapshot, ComponentSnapshot, ProjectSnapshot,
    SchedulerStateSnapshot, SnapshotValue,
};
