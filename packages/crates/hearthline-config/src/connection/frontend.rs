use serde::Serialize;

use crate::appliance::{ConfigRepository, InterfaceConfig};

use super::{ConnectionEndpoint, LoadedConnection, endpoint_port, negotiated_duplex};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendConnection {
    pub id: String,
    pub label: String,
    pub lifecycle: String,
    pub transport: String,
    pub medium: String,
    pub medium_detail: String,
    pub endpoint_a: FrontendConnectionEndpoint,
    pub endpoint_b: FrontendConnectionEndpoint,
    pub capacity_mbps: u64,
    pub effective_mtu: u32,
    pub latency_ms: u64,
    pub physical_delay_us: u64,
    pub loss_every: Option<u64>,
    pub negotiated_duplex: String,
    pub direction: String,
    pub configured_operational: bool,
    pub initial_operational: bool,
    pub physical_facts: Vec<String>,
    pub tags: Vec<String>,
    pub source_path: String,
    pub source_yaml: String,
    pub revision: String,
}

impl FrontendConnection {
    pub(super) fn new(loaded: &LoadedConnection, appliances: &ConfigRepository) -> Self {
        let config = &loaded.config;
        let interface_a = endpoint_port(appliances, &config.endpoints.a)
            .expect("validated connection endpoint A must exist");
        let interface_b = endpoint_port(appliances, &config.endpoints.b)
            .expect("validated connection endpoint B must exist");
        Self {
            id: config.id.clone(),
            label: config.label.clone(),
            lifecycle: config.lifecycle.to_string(),
            transport: config.transport.to_string(),
            medium: config.medium.kind().to_string(),
            medium_detail: config.medium.detail().to_string(),
            endpoint_a: FrontendConnectionEndpoint::new(&config.endpoints.a, interface_a),
            endpoint_b: FrontendConnectionEndpoint::new(&config.endpoints.b, interface_b),
            capacity_mbps: config.properties.capacity_mbps,
            effective_mtu: interface_a.settings.mtu.min(interface_b.settings.mtu),
            latency_ms: config.properties.latency_ms,
            physical_delay_us: config.medium.propagation_delay_us(),
            loss_every: config.properties.loss_every,
            negotiated_duplex: negotiated_duplex(
                interface_a.settings.duplex,
                interface_b.settings.duplex,
                config.medium.kind(),
            )
            .to_string(),
            direction: config.properties.direction.to_string(),
            configured_operational: config.properties.operational,
            initial_operational: config.properties.operational
                && interface_a.state.initially_usable()
                && interface_b.state.initially_usable(),
            physical_facts: config
                .medium
                .physical_facts()
                .into_iter()
                .map(|fact| fact.to_string())
                .collect(),
            tags: config.tags.clone(),
            source_path: loaded.source_path.clone(),
            source_yaml: loaded.source_yaml.clone(),
            revision: loaded.revision(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendConnectionEndpoint {
    pub appliance: String,
    pub interface: String,
    pub hardware: String,
    pub administrative_state: String,
    pub initial_operational_state: String,
    pub speed_mbps: u64,
    pub duplex: String,
    pub mtu: u32,
}

impl FrontendConnectionEndpoint {
    fn new(endpoint: &ConnectionEndpoint, interface: &InterfaceConfig) -> Self {
        Self {
            appliance: endpoint.appliance.clone(),
            interface: endpoint.interface.clone(),
            hardware: interface.hardware.to_string(),
            administrative_state: interface.state.administrative.to_string(),
            initial_operational_state: interface.state.initial_operational.to_string(),
            speed_mbps: interface.settings.speed_mbps,
            duplex: interface.settings.duplex.to_string(),
            mtu: interface.settings.mtu,
        }
    }
}
