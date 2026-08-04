pub(crate) mod firewall_ha;
pub(crate) mod first_hop;
pub(crate) mod link_aggregation;
pub(crate) mod spanning_tree;
mod topology;

pub use firewall_ha::{ScenarioFirewallHaOverride, ScenarioFirewallHaState};
pub use first_hop::{ScenarioFirstHopOverride, ScenarioFirstHopState};
pub use link_aggregation::ScenarioLinkAggregationState;
pub use spanning_tree::{ScenarioSpanningTreeState, SpanningTreePortRole, SpanningTreePortState};
pub use topology::{ScenarioConnectionOverride, ScenarioConnectionState};
pub(crate) use topology::{connection, local_autonomy};
