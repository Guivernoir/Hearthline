mod report;
mod runner;
mod runtime;
mod security;

pub use report::{
    SCENARIO_REPORT_SCHEMA_VERSION, ScenarioContinuityReport, ScenarioHaIsolationReport,
    ScenarioHttpDocument, ScenarioHttpResponse, ScenarioLocalAutonomyReport, ScenarioReport,
    ScenarioStatistics, ScenarioStatus, ScenarioTraceEntry, ScenarioTraceKind,
};
#[doc(hidden)]
pub use runner::is_interactive_scenario;
pub use runner::{
    InteractiveScenarioSession, run_scenario, run_scenario_with_overrides,
    run_scenario_with_state_overrides,
};
pub use runtime::{ScenarioRuntimeSnapshots, ScenarioStateSnapshot};
pub use security::{SECURITY_EVENT_SCHEMA_VERSION, ScenarioSecurityEvent, SecurityDisposition};

pub use hearthline_config::{
    ScenarioConfig, ScenarioConnectionOverride, ScenarioConnectionState, ScenarioContinuityFault,
    ScenarioExpectation, ScenarioExpectationMode, ScenarioExpectedOutcome,
    ScenarioFirewallHaOverride, ScenarioFirewallHaState, ScenarioFirstHopOverride,
    ScenarioFirstHopState, ScenarioLinkAggregationState, ScenarioPacketConfig, ScenarioRepository,
    ScenarioSpanningTreeState,
};

pub(crate) mod connection {
    pub use hearthline_config::scenario_connection_states;
}
pub(crate) mod firewall_ha {
    pub use hearthline_config::scenario_firewall_ha_states;
}
pub(crate) mod first_hop {
    pub use hearthline_config::scenario_first_hop_states;
}
pub(crate) mod link_aggregation {
    pub use hearthline_config::scenario_link_aggregation_states;
}
pub(crate) mod spanning_tree {
    pub use hearthline_config::scenario_spanning_tree_states;
}
pub(crate) mod local_autonomy {
    pub use hearthline_config::local_control_topology;
}
