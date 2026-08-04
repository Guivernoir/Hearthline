use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpanningTreePortRole {
    Root,
    Designated,
    Alternate,
    Disabled,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpanningTreePortState {
    Forwarding,
    Discarding,
}

impl SpanningTreePortState {
    pub const fn is_forwarding(self) -> bool {
        matches!(self, Self::Forwarding)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScenarioSpanningTreeState {
    pub appliance: String,
    pub interface: String,
    pub connection: String,
    pub protocol: String,
    pub vlan: u16,
    pub root_bridge: String,
    pub root_path_cost: u32,
    pub port_path_cost: u32,
    pub role: SpanningTreePortRole,
    pub state: SpanningTreePortState,
}
