use std::collections::BTreeMap;

use serde::Serialize;

use crate::connection::FrontendConnection;

use super::{InterfaceConfig, LoadedAppliance};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendApplianceCatalog {
    pub schema_version: &'static str,
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
pub struct FrontendInterface {
    pub id: String,
    pub hardware: String,
    pub mode: String,
    pub administrative_state: String,
    pub initial_operational_state: String,
    pub speed_mbps: u64,
    pub duplex: String,
    pub mtu: u32,
    pub addresses: Vec<String>,
    pub vlans: Vec<u16>,
    pub supported_media: Vec<String>,
}

impl From<&InterfaceConfig> for FrontendInterface {
    fn from(interface: &InterfaceConfig) -> Self {
        Self {
            id: interface.id.clone(),
            hardware: interface.hardware.to_string(),
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
        }
    }
}
