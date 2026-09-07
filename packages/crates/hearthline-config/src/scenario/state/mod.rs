pub(crate) mod firewall_ha;
pub(crate) mod first_hop;
pub(crate) mod link_aggregation;
pub(crate) mod spanning_tree;
mod topology;

pub use firewall_ha::scenario_firewall_ha_states;
pub use firewall_ha::{ScenarioFirewallHaOverride, ScenarioFirewallHaState};
pub use first_hop::scenario_first_hop_states;
pub use first_hop::{ScenarioFirstHopOverride, ScenarioFirstHopState};
pub use link_aggregation::ScenarioLinkAggregationState;
pub use link_aggregation::scenario_link_aggregation_states;
pub use spanning_tree::scenario_spanning_tree_states;
pub use spanning_tree::{ScenarioSpanningTreeState, SpanningTreePortRole, SpanningTreePortState};
pub use topology::{
    LocalControlTopology, ScenarioConnectionOverride, ScenarioConnectionState,
    local_control_topology, scenario_connection_states,
};
pub(crate) use topology::{connection, local_autonomy};
