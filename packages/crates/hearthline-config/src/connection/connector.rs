use hearthline_engine::{
    LinkDirection, LinkEndpoint, MediaLink, MediaLinkConfig, MediumKind, PortDuplex, SimulatedPort,
};
use hearthline_model::{ComponentId, PortId};

use crate::appliance::{ConfigError, ConfigRepository, InterfaceConfig, InterfaceMode};

use super::{ConnectionConfig, ConnectionDirection, ConnectionEndpoint};

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

pub(super) fn validate_endpoint(
    appliances: &ConfigRepository,
    connection: &ConnectionConfig,
    endpoint: &ConnectionEndpoint,
) -> Result<(), ConfigError> {
    let appliance = appliances.get(&endpoint.appliance).ok_or_else(|| {
        ConfigError::new(format!(
            "connection {} references missing appliance {}",
            connection.id, endpoint.appliance
        ))
    })?;
    if !appliance
        .config
        .interfaces
        .iter()
        .any(|interface| interface.id == endpoint.interface)
    {
        return Err(ConfigError::new(format!(
            "connection {} references missing interface {} on {}",
            connection.id, endpoint.interface, endpoint.appliance
        )));
    }
    Ok(())
}

pub(super) fn validate_endpoint_port(
    appliances: &ConfigRepository,
    connection: &ConnectionConfig,
) -> Result<(), ConfigError> {
    for endpoint in [&connection.endpoints.a, &connection.endpoints.b] {
        let interface = endpoint_port(appliances, endpoint)?;
        if interface.mode == InterfaceMode::Svi {
            return Err(ConfigError::new(format!(
                "connection {} cannot terminate media on virtual SVI {}:{}",
                connection.id, endpoint.appliance, endpoint.interface
            )));
        }
        if !interface.hardware.supports(connection.medium.kind()) {
            return Err(ConfigError::new(format!(
                "connection {} {} medium is not supported by {} port {} on {}",
                connection.id,
                connection.medium.kind(),
                interface.hardware,
                endpoint.interface,
                endpoint.appliance
            )));
        }
        if connection.properties.capacity_mbps > interface.settings.speed_mbps {
            return Err(ConfigError::new(format!(
                "connection {} capacity {} Mbps exceeds port {}:{} configured speed {} Mbps",
                connection.id,
                connection.properties.capacity_mbps,
                endpoint.appliance,
                endpoint.interface,
                interface.settings.speed_mbps
            )));
        }
    }
    Ok(())
}

pub(super) fn endpoint_port<'a>(
    appliances: &'a ConfigRepository,
    endpoint: &ConnectionEndpoint,
) -> Result<&'a InterfaceConfig, ConfigError> {
    appliances
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
        })
}

pub(super) fn negotiated_duplex(a: PortDuplex, b: PortDuplex, medium: MediumKind) -> PortDuplex {
    if medium == MediumKind::Radio || a == PortDuplex::Half || b == PortDuplex::Half {
        PortDuplex::Half
    } else {
        PortDuplex::Full
    }
}

fn runtime_endpoint(
    appliances: &ConfigRepository,
    endpoint: &ConnectionEndpoint,
) -> Result<LinkEndpoint, ConfigError> {
    let interface = endpoint_port(appliances, endpoint)?;
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
