use hearthline_config::{
    ConfigError, ConfigRepository, ConnectionConfig, ConnectionDirection, ConnectionEndpoint,
};
use hearthline_engine::{LinkDirection, LinkEndpoint, MediaLink, MediaLinkConfig, SimulatedPort};
use hearthline_model::{ComponentId, PortId};

pub(super) fn build_media_link(
    connection: &ConnectionConfig,
    appliances: &ConfigRepository,
) -> Result<MediaLink, ConfigError> {
    let endpoint_a = runtime_endpoint(appliances, &connection.endpoints.a)?;
    let endpoint_b = runtime_endpoint(appliances, &connection.endpoints.b)?;
    MediaLink::new(
        ComponentId::new(&connection.id).map_err(|error| ConfigError::new(error.to_string()))?,
        endpoint_a,
        endpoint_b,
        MediaLinkConfig {
            capacity_mbps: connection.properties.capacity_mbps,
            latency_ms: connection.properties.latency_ms,
            loss_every: connection.properties.loss_every,
            direction: match connection.properties.direction {
                ConnectionDirection::Bidirectional => LinkDirection::Bidirectional,
                ConnectionDirection::AToB => LinkDirection::AToB,
                ConnectionDirection::BToA => LinkDirection::BToA,
            },
            operational: connection.properties.operational,
        },
        connection.medium.clone(),
    )
    .map_err(|error| ConfigError::new(format!("connection {}: {error}", connection.id)))
}

fn runtime_endpoint(
    appliances: &ConfigRepository,
    endpoint: &ConnectionEndpoint,
) -> Result<LinkEndpoint, ConfigError> {
    let interface = appliances
        .get(&endpoint.appliance)
        .and_then(|appliance| {
            appliance
                .config
                .interfaces
                .iter()
                .find(|interface| interface.id == endpoint.interface)
        })
        .ok_or_else(|| {
            ConfigError::new(format!(
                "missing interface {} on {}",
                endpoint.interface, endpoint.appliance
            ))
        })?;
    Ok(LinkEndpoint {
        component: ComponentId::new(&endpoint.appliance)
            .map_err(|error| ConfigError::new(error.to_string()))?,
        port: PortId::new(&endpoint.interface)
            .map_err(|error| ConfigError::new(error.to_string()))?,
        profile: SimulatedPort {
            hardware: interface.hardware,
            state: interface.state,
            settings: interface.settings,
        },
    })
}
