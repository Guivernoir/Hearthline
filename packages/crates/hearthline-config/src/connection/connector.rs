use hearthline_model::{ComponentId, EthernetFrame};

use crate::appliance::{ConfigError, ConfigRepository, InterfaceConfig};
use hearthline_engine::{ConnectionMedium, MediumKind, PortDuplex, PortState};

use super::{
    ConnectionConfig, ConnectionEndpoint, ConnectionEndpoints, ConnectionProperties,
    ConnectorPortProfile,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConnectorDropReason {
    Down,
    SourcePortDown,
    DestinationPortDown,
    InvalidEndpoint,
    MtuExceeded { frame_bytes: u32, mtu: u32 },
    ModeledLoss,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectorTransit {
    pub delay_ms: u64,
    pub physical_delay_us: u64,
}

#[derive(Clone, Debug)]
pub struct SimulatedConnector {
    id: ComponentId,
    endpoints: ConnectionEndpoints,
    properties: ConnectionProperties,
    endpoint_a: ConnectorPortProfile,
    endpoint_b: ConnectorPortProfile,
    effective_mtu: u32,
    physical_delay_us: u64,
    frame_count: u64,
}

impl SimulatedConnector {
    pub fn new(
        id: ComponentId,
        endpoints: ConnectionEndpoints,
        properties: ConnectionProperties,
    ) -> Result<Self, ConfigError> {
        if endpoints.a == endpoints.b {
            return Err(ConfigError::new("connector endpoints must differ"));
        }
        Ok(Self {
            id,
            endpoints,
            properties,
            endpoint_a: ConnectorPortProfile::default(),
            endpoint_b: ConnectorPortProfile::default(),
            effective_mtu: 1_500,
            physical_delay_us: 0,
            frame_count: 0,
        })
    }

    pub fn new_configured(
        id: ComponentId,
        endpoints: ConnectionEndpoints,
        properties: ConnectionProperties,
        medium: ConnectionMedium,
        endpoint_a: ConnectorPortProfile,
        endpoint_b: ConnectorPortProfile,
    ) -> Result<Self, ConfigError> {
        if endpoints.a == endpoints.b {
            return Err(ConfigError::new("connector endpoints must differ"));
        }
        Ok(Self {
            id,
            endpoints,
            properties,
            endpoint_a,
            endpoint_b,
            effective_mtu: endpoint_a.mtu.min(endpoint_b.mtu),
            physical_delay_us: medium.propagation_delay_us(),
            frame_count: 0,
        })
    }

    pub fn id(&self) -> &ComponentId {
        &self.id
    }

    pub fn set_operational(&mut self, operational: bool) {
        self.properties.operational = operational;
    }

    pub fn set_port_operational(
        &mut self,
        endpoint: &ConnectionEndpoint,
        operational: PortState,
    ) -> Result<(), ConnectorDropReason> {
        if endpoint == &self.endpoints.a {
            self.endpoint_a.state.initial_operational = operational;
        } else if endpoint == &self.endpoints.b {
            self.endpoint_b.state.initial_operational = operational;
        } else {
            return Err(ConnectorDropReason::InvalidEndpoint);
        }
        Ok(())
    }

    pub fn transmit(
        &mut self,
        source: &ConnectionEndpoint,
        frame: &EthernetFrame,
        frame_bytes: u32,
    ) -> Result<(&ConnectionEndpoint, ConnectorTransit), ConnectorDropReason> {
        if !self.properties.operational {
            return Err(ConnectorDropReason::Down);
        }
        let (destination, source_port, destination_port) =
            if source == &self.endpoints.a && self.properties.direction.permits_a_to_b() {
                (&self.endpoints.b, self.endpoint_a, self.endpoint_b)
            } else if source == &self.endpoints.b && self.properties.direction.permits_b_to_a() {
                (&self.endpoints.a, self.endpoint_b, self.endpoint_a)
            } else {
                return Err(ConnectorDropReason::InvalidEndpoint);
            };
        if !source_port.state.initially_usable() {
            return Err(ConnectorDropReason::SourcePortDown);
        }
        if !destination_port.state.initially_usable() {
            return Err(ConnectorDropReason::DestinationPortDown);
        }
        if frame_bytes > self.effective_mtu {
            return Err(ConnectorDropReason::MtuExceeded {
                frame_bytes,
                mtu: self.effective_mtu,
            });
        }
        self.frame_count += 1;
        if self
            .properties
            .loss_every
            .is_some_and(|interval| self.frame_count.is_multiple_of(interval))
        {
            return Err(ConnectorDropReason::ModeledLoss);
        }
        let _ = frame;
        let serialization_ms = u64::from(frame_bytes)
            .saturating_mul(8)
            .div_ceil(self.properties.capacity_mbps.saturating_mul(1_000));
        Ok((
            destination,
            ConnectorTransit {
                delay_ms: self.properties.latency_ms
                    + serialization_ms
                    + self.physical_delay_us.div_ceil(1_000),
                physical_delay_us: self.physical_delay_us,
            },
        ))
    }
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
