use std::collections::BTreeMap;

use serde::Serialize;

use crate::connection::FrontendConnection;

use super::{
    FirewallHaConfig, FirstHopConfig, InterfaceConfig, LinkAggregationConfig,
    LinkAggregationGroupConfig, LoadedAppliance, MultiChassisConfig, SpanningTreeConfig,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendApplianceCatalog {
    pub schema_version: &'static str,
    pub appliance_schema_version: &'static str,
    pub generation_status: &'static str,
    pub generated_by: &'static str,
    pub appliance_source_root: &'static str,
    pub connection_source_root: &'static str,
    pub appliances: Vec<FrontendAppliance>,
    pub node_index: BTreeMap<String, Vec<String>>,
    pub connections: Vec<FrontendConnection>,
    pub appliance_connection_index: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendAppliance {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub behavior_family: String,
    pub site: String,
    pub environment: String,
    pub zone: String,
    pub role: String,
    pub summary: String,
    pub lifecycle: String,
    pub tags: Vec<String>,
    pub default_gateway: Option<String>,
    pub spanning_tree: Option<FrontendSpanningTree>,
    pub link_aggregation: Option<FrontendLinkAggregation>,
    pub multi_chassis: Option<FrontendMultiChassis>,
    pub firewall_ha: Option<FrontendFirewallHa>,
    pub source_path: String,
    pub source_yaml: String,
    pub revision: String,
    pub addresses: Vec<String>,
    pub interface_count: usize,
    pub interfaces: Vec<FrontendInterface>,
    pub services: Vec<String>,
    pub behavior_facts: Vec<String>,
}

impl From<&LoadedAppliance> for FrontendAppliance {
    fn from(loaded: &LoadedAppliance) -> Self {
        let config = &loaded.config;
        Self {
            id: config.id.clone(),
            label: config.label.clone(),
            kind: config.kind.to_string(),
            behavior_family: config.behavior_family().to_string(),
            site: config.site.clone(),
            environment: config.environment.clone(),
            zone: config.zone.clone(),
            role: config.role.clone(),
            summary: config.summary.clone(),
            lifecycle: config.lifecycle.to_string(),
            tags: config.tags.clone(),
            default_gateway: config.default_gateway.clone(),
            spanning_tree: config
                .spanning_tree
                .as_ref()
                .map(FrontendSpanningTree::from),
            link_aggregation: config
                .link_aggregation
                .as_ref()
                .map(FrontendLinkAggregation::from),
            multi_chassis: config
                .multi_chassis
                .as_ref()
                .map(FrontendMultiChassis::from),
            firewall_ha: config.firewall_ha.as_ref().map(FrontendFirewallHa::from),
            source_path: loaded.source_path.clone(),
            source_yaml: loaded.source_yaml.clone(),
            revision: loaded.revision(),
            addresses: config
                .interfaces
                .iter()
                .flat_map(|interface| interface.addresses.iter().cloned())
                .collect(),
            interface_count: config.interfaces.len(),
            interfaces: config
                .interfaces
                .iter()
                .map(FrontendInterface::from)
                .collect(),
            services: config.behavior.services(),
            behavior_facts: config.behavior.facts(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendFirewallHa {
    pub domain: String,
    pub peer: String,
    pub role: String,
    pub sync_interface: String,
    pub monitored_interfaces: Vec<String>,
    pub session_sync: bool,
    pub heartbeat_interval_ms: u64,
    pub failure_hold_ms: u64,
}

impl From<&FirewallHaConfig> for FrontendFirewallHa {
    fn from(config: &FirewallHaConfig) -> Self {
        Self {
            domain: config.domain.clone(),
            peer: config.peer.clone(),
            role: config.role.to_string(),
            sync_interface: config.sync_interface.clone(),
            monitored_interfaces: config.monitored_interfaces.clone(),
            session_sync: config.session_sync,
            heartbeat_interval_ms: config.heartbeat_interval_ms,
            failure_hold_ms: config.failure_hold_ms,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendLinkAggregation {
    pub system_mac: String,
    pub groups: Vec<FrontendLinkAggregationGroup>,
}

impl From<&LinkAggregationConfig> for FrontendLinkAggregation {
    fn from(config: &LinkAggregationConfig) -> Self {
        Self {
            system_mac: config.system_mac.clone(),
            groups: config
                .groups
                .iter()
                .map(FrontendLinkAggregationGroup::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendLinkAggregationGroup {
    pub id: String,
    pub logical_id: String,
    pub protocol: String,
    pub mode: String,
    pub minimum_active_members: u8,
    pub members: Vec<String>,
}

impl From<&LinkAggregationGroupConfig> for FrontendLinkAggregationGroup {
    fn from(config: &LinkAggregationGroupConfig) -> Self {
        Self {
            id: config.id.clone(),
            logical_id: config.logical_id.clone(),
            protocol: config.protocol.to_string(),
            mode: config.mode.to_string(),
            minimum_active_members: config.minimum_active_members,
            members: config.members.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendMultiChassis {
    pub domain: String,
    pub peer: String,
    pub peer_link: String,
    pub role: String,
}

impl From<&MultiChassisConfig> for FrontendMultiChassis {
    fn from(config: &MultiChassisConfig) -> Self {
        Self {
            domain: config.domain.clone(),
            peer: config.peer.clone(),
            peer_link: config.peer_link.clone(),
            role: config.role.to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendSpanningTree {
    pub protocol: String,
    pub bridge_priority: u16,
    pub bridge_mac: String,
}

impl From<&SpanningTreeConfig> for FrontendSpanningTree {
    fn from(config: &SpanningTreeConfig) -> Self {
        Self {
            protocol: config.protocol.to_string(),
            bridge_priority: config.bridge_priority,
            bridge_mac: config.bridge_mac.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendInterface {
    pub id: String,
    pub hardware: String,
    pub mac_address: Option<String>,
    pub mode: String,
    pub administrative_state: String,
    pub initial_operational_state: String,
    pub speed_mbps: u64,
    pub duplex: String,
    pub mtu: u32,
    pub addresses: Vec<String>,
    pub vlans: Vec<u16>,
    pub supported_media: Vec<String>,
    pub first_hop: Option<FrontendFirstHop>,
}

impl From<&InterfaceConfig> for FrontendInterface {
    fn from(interface: &InterfaceConfig) -> Self {
        Self {
            id: interface.id.clone(),
            hardware: interface.hardware.to_string(),
            mac_address: interface.mac_address.clone(),
            mode: interface.mode.to_string(),
            administrative_state: interface.state.administrative.to_string(),
            initial_operational_state: interface.state.initial_operational.to_string(),
            speed_mbps: interface.settings.speed_mbps,
            duplex: interface.settings.duplex.to_string(),
            mtu: interface.settings.mtu,
            addresses: interface.addresses.clone(),
            vlans: interface.vlans.clone(),
            supported_media: interface
                .hardware
                .supported_media()
                .iter()
                .map(ToString::to_string)
                .collect(),
            first_hop: interface.first_hop.as_ref().map(FrontendFirstHop::from),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendFirstHop {
    pub protocol: String,
    pub group: u8,
    pub virtual_ip: String,
    pub virtual_mac: String,
    pub priority: u8,
    pub preempt: bool,
    pub initial_role: String,
}

impl From<&FirstHopConfig> for FrontendFirstHop {
    fn from(config: &FirstHopConfig) -> Self {
        Self {
            protocol: config.protocol.to_string(),
            group: config.group,
            virtual_ip: config.virtual_ip.clone(),
            virtual_mac: config.virtual_mac.clone(),
            priority: config.priority,
            preempt: config.preempt,
            initial_role: config.initial_role.to_string(),
        }
    }
}
