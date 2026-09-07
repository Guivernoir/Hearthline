use hearthline_engine::{MediumKind, PortDuplex};

use crate::appliance::{ConfigError, ConfigRepository, InterfaceConfig, InterfaceMode};

use super::{ConnectionConfig, ConnectionEndpoint};

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
