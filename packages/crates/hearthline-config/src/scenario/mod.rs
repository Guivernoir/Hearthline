mod repository;
mod schema;
mod security;
mod state;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScenarioExpectationMode {
    Baseline,
    Recovery,
    Continuity,
    Isolation,
    Autonomy,
}

pub use repository::{LoadedScenario, ScenarioRepository};
pub use schema::{
    SCENARIO_SCHEMA_VERSION, ScenarioApplicationConfig, ScenarioConfig, ScenarioContinuityConfig,
    ScenarioContinuityFault, ScenarioExpectation, ScenarioExpectedOutcome,
    ScenarioHaIsolationConfig, ScenarioHttpMethod, ScenarioLocalAutonomyConfig,
    ScenarioPacketConfig, ScenarioRecoveryConfig, ScenarioSummary, ScenarioTransportConfig,
    TelemetryIdentity, retarget_telemetry_packet, telemetry_identity,
};
pub use security::{ScenarioSecurityConfig, SecuritySeverity};
#[doc(hidden)]
pub use state::{
    LocalControlTopology, local_control_topology, scenario_connection_states,
    scenario_firewall_ha_states, scenario_first_hop_states, scenario_link_aggregation_states,
    scenario_spanning_tree_states,
};
pub use state::{
    ScenarioConnectionOverride, ScenarioConnectionState, ScenarioFirewallHaOverride,
    ScenarioFirewallHaState, ScenarioFirstHopOverride, ScenarioFirstHopState,
    ScenarioLinkAggregationState, ScenarioSpanningTreeState, SpanningTreePortRole,
    SpanningTreePortState,
};
pub(crate) use state::{connection, firewall_ha, first_hop, link_aggregation, spanning_tree};
