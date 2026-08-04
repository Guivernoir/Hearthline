use serde::{Deserialize, Serialize};

use super::{
    ScenarioContinuityConfig, ScenarioExpectation, ScenarioHaIsolationConfig,
    ScenarioLocalAutonomyConfig, ScenarioPacketConfig, ScenarioRecoveryConfig,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioSummary {
    pub schema_version: String,
    pub id: String,
    pub label: String,
    pub summary: String,
    pub category: String,
    pub participants: Vec<String>,
    pub source: String,
    pub packet: ScenarioPacketConfig,
    pub connection_states: Vec<super::super::ScenarioConnectionState>,
    pub first_hop_states: Vec<super::super::ScenarioFirstHopState>,
    pub link_aggregation_states: Vec<super::super::ScenarioLinkAggregationState>,
    pub spanning_tree_states: Vec<super::super::ScenarioSpanningTreeState>,
    pub firewall_ha_states: Vec<super::super::ScenarioFirewallHaState>,
    pub recovery: Option<ScenarioRecoveryConfig>,
    pub continuity: Option<ScenarioContinuityConfig>,
    pub ha_isolation: Option<ScenarioHaIsolationConfig>,
    pub local_autonomy: Option<ScenarioLocalAutonomyConfig>,
    pub expectation: ScenarioExpectation,
    pub security: Option<super::super::ScenarioSecurityConfig>,
}
