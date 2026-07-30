use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use hearthline_model::{ComponentId, EthernetFrame};
use serde::{Deserialize, Serialize};

use crate::config::{ConfigError, ConfigRepository, InterfaceConfig, Lifecycle, source_revision};
use crate::media::{ConnectionMedium, MediumKind};
use crate::port::{PortDuplex, PortState, PortStateConfig};

pub const CONNECTION_SCHEMA_VERSION: &str = "0.2.0";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionConfig {
    pub schema_version: String,
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub lifecycle: Lifecycle,
    pub transport: TransportKind,
    pub medium: ConnectionMedium,
    pub endpoints: ConnectionEndpoints,
    #[serde(default)]
    pub properties: ConnectionProperties,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl ConnectionConfig {
    pub fn from_yaml(source: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_yaml_ng::from_str(source)
            .map_err(|error| ConfigError::new(format!("invalid connection YAML: {error}")))?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != CONNECTION_SCHEMA_VERSION {
            return Err(ConfigError::new(format!(
                "connection {} uses schema {}, expected {}",
                self.id, self.schema_version, CONNECTION_SCHEMA_VERSION
            )));
        }
        ComponentId::new(self.id.clone()).map_err(|error| ConfigError::new(error.to_string()))?;
        if self.label.trim().is_empty() {
            return Err(ConfigError::new("connection label cannot be empty"));
        }
        for endpoint in [&self.endpoints.a, &self.endpoints.b] {
            ComponentId::new(endpoint.appliance.clone())
                .map_err(|error| ConfigError::new(error.to_string()))?;
            ComponentId::new(endpoint.interface.clone())
                .map_err(|error| ConfigError::new(error.to_string()))?;
        }
        if self.endpoints.a == self.endpoints.b {
            return Err(ConfigError::new(format!(
                "connection {} cannot join an endpoint to itself",
                self.id
            )));
        }
        if self.properties.capacity_mbps == 0 {
            return Err(ConfigError::new(format!(
                "connection {} requires non-zero capacity",
                self.id
            )));
        }
        if self.properties.loss_every == Some(0) {
            return Err(ConfigError::new(format!(
                "connection {} loss interval cannot be zero",
                self.id
            )));
        }
        if !self.transport.accepts(self.medium.kind()) {
            return Err(ConfigError::new(format!(
                "connection {} transport {} is incompatible with {} medium",
                self.id,
                self.transport,
                self.medium.kind()
            )));
        }
        self.medium
            .validate()
            .map_err(|error| ConfigError::new(format!("connection {} medium: {error}", self.id)))?;
        if self
            .medium
            .max_capacity_mbps()
            .is_some_and(|maximum| self.properties.capacity_mbps > maximum)
        {
            return Err(ConfigError::new(format!(
                "connection {} capacity {} Mbps exceeds {} medium limit",
                self.id,
                self.properties.capacity_mbps,
                self.medium.kind()
            )));
        }
        Ok(())
    }

    pub fn connector(
        &self,
        appliances: &ConfigRepository,
    ) -> Result<SimulatedConnector, ConfigError> {
        let endpoint_a = endpoint_port(appliances, &self.endpoints.a)?;
        let endpoint_b = endpoint_port(appliances, &self.endpoints.b)?;
        SimulatedConnector::new_configured(
            ComponentId::new(self.id.clone())
                .map_err(|error| ConfigError::new(error.to_string()))?,
            self.endpoints.clone(),
            self.properties,
            self.medium.clone(),
            endpoint_a.into(),
            endpoint_b.into(),
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum TransportKind {
    Ethernet,
    WirelessLan,
    WideArea,
    FieldIo,
    Virtual,
    Mirror,
    EncryptedIp,
    AnalogTelephone,
}

impl TransportKind {
    const fn accepts(self, medium: MediumKind) -> bool {
        matches!(
            (self, medium),
            (Self::Ethernet, MediumKind::Copper | MediumKind::Fiber)
                | (Self::WirelessLan, MediumKind::Radio)
                | (Self::WideArea, MediumKind::Carrier)
                | (Self::FieldIo, MediumKind::FieldWiring)
                | (Self::Virtual, MediumKind::Virtual)
                | (Self::Mirror, MediumKind::Copper | MediumKind::Fiber)
                | (
                    Self::EncryptedIp,
                    MediumKind::Copper
                        | MediumKind::Fiber
                        | MediumKind::Virtual
                        | MediumKind::Carrier
                )
                | (Self::AnalogTelephone, MediumKind::Telephone)
        )
    }
}

impl Display for TransportKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Ethernet => "ethernet",
            Self::WirelessLan => "wireless-lan",
            Self::WideArea => "wide-area",
            Self::FieldIo => "field-io",
            Self::Virtual => "virtual",
            Self::Mirror => "mirror",
            Self::EncryptedIp => "encrypted-ip",
            Self::AnalogTelephone => "analog-telephone",
        };
        formatter.write_str(value)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConnectionEndpoint {
    pub appliance: String,
    pub interface: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionEndpoints {
    pub a: ConnectionEndpoint,
    pub b: ConnectionEndpoint,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionProperties {
    #[serde(default = "default_capacity")]
    pub capacity_mbps: u64,
    #[serde(default)]
    pub latency_ms: u64,
    #[serde(default)]
    pub loss_every: Option<u64>,
    #[serde(default)]
    pub direction: ConnectionDirection,
    #[serde(default = "default_true")]
    pub operational: bool,
}

impl Default for ConnectionProperties {
    fn default() -> Self {
        Self {
            capacity_mbps: default_capacity(),
            latency_ms: 0,
            loss_every: None,
            direction: ConnectionDirection::Bidirectional,
            operational: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ConnectorPortProfile {
    pub state: PortStateConfig,
    pub speed_mbps: u64,
    pub duplex: PortDuplex,
    pub mtu: u32,
}

impl Default for ConnectorPortProfile {
    fn default() -> Self {
        Self {
            state: PortStateConfig {
                administrative: PortState::Up,
                initial_operational: PortState::Up,
            },
            speed_mbps: u64::MAX,
            duplex: PortDuplex::Full,
            mtu: 1_500,
        }
    }
}

impl From<&InterfaceConfig> for ConnectorPortProfile {
    fn from(interface: &InterfaceConfig) -> Self {
        Self {
            state: interface.state,
            speed_mbps: interface.settings.speed_mbps,
            duplex: interface.settings.duplex,
            mtu: interface.settings.mtu,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ConnectionDirection {
    #[default]
    Bidirectional,
    AToB,
    BToA,
}

impl ConnectionDirection {
    const fn permits_a_to_b(self) -> bool {
        matches!(self, Self::Bidirectional | Self::AToB)
    }

    const fn permits_b_to_a(self) -> bool {
        matches!(self, Self::Bidirectional | Self::BToA)
    }
}

impl Display for ConnectionDirection {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Bidirectional => "bidirectional",
            Self::AToB => "a-to-b",
            Self::BToA => "b-to-a",
        };
        formatter.write_str(value)
    }
}

#[derive(Clone, Debug)]
pub struct LoadedConnection {
    pub config: ConnectionConfig,
    pub source_path: String,
    pub source_yaml: String,
    pub source_file: PathBuf,
}

impl LoadedConnection {
    pub fn revision(&self) -> String {
        source_revision(&self.source_yaml)
    }
}

#[derive(Clone, Debug)]
pub struct ConnectionRepository {
    connections: BTreeMap<String, LoadedConnection>,
}

impl ConnectionRepository {
    pub fn load(
        root: impl AsRef<Path>,
        appliances: &ConfigRepository,
    ) -> Result<Self, ConfigError> {
        Self::load_with_override(root, appliances, None)
    }

    pub fn load_with_override(
        root: impl AsRef<Path>,
        appliances: &ConfigRepository,
        source_override: Option<(&Path, &str)>,
    ) -> Result<Self, ConfigError> {
        let root = root.as_ref();
        let mut paths = Vec::new();
        collect_yaml_paths(root, &mut paths)?;
        paths.sort();
        if paths.is_empty() {
            return Err(ConfigError::new(format!(
                "{} contains no connection YAML files",
                root.display()
            )));
        }
        let source_base = root
            .parent()
            .and_then(Path::parent)
            .or_else(|| root.parent())
            .unwrap_or(root);
        let mut connections = BTreeMap::new();
        let mut endpoint_pairs = BTreeSet::new();
        let mut point_to_point_endpoints = BTreeMap::new();

        for path in paths {
            let source_yaml = if source_override
                .as_ref()
                .is_some_and(|(override_path, _)| *override_path == path)
            {
                source_override
                    .as_ref()
                    .map(|(_, source)| (*source).to_owned())
                    .unwrap_or_default()
            } else {
                fs::read_to_string(&path).map_err(|error| {
                    ConfigError::new(format!("cannot read {}: {error}", path.display()))
                })?
            };
            let config = ConnectionConfig::from_yaml(&source_yaml)
                .map_err(|error| ConfigError::new(format!("{}: {error}", path.display())))?;
            let expected_file = format!("{}.yaml", config.id);
            if path.file_name().and_then(|name| name.to_str()) != Some(expected_file.as_str()) {
                return Err(ConfigError::new(format!(
                    "{} must be named {}",
                    path.display(),
                    expected_file
                )));
            }
            validate_endpoint(appliances, &config, &config.endpoints.a)?;
            validate_endpoint(appliances, &config, &config.endpoints.b)?;
            validate_endpoint_port(appliances, &config)?;

            let mut pair = [
                format!(
                    "{}:{}",
                    config.endpoints.a.appliance, config.endpoints.a.interface
                ),
                format!(
                    "{}:{}",
                    config.endpoints.b.appliance, config.endpoints.b.interface
                ),
            ];
            pair.sort();
            if !endpoint_pairs.insert(pair) {
                return Err(ConfigError::new(format!(
                    "connection {} duplicates an existing endpoint pair",
                    config.id
                )));
            }
            if config.medium.requires_exclusive_endpoints() {
                for endpoint in [&config.endpoints.a, &config.endpoints.b] {
                    let key = format!("{}:{}", endpoint.appliance, endpoint.interface);
                    if let Some(existing) =
                        point_to_point_endpoints.insert(key.clone(), config.id.clone())
                    {
                        return Err(ConfigError::new(format!(
                            "connection {} reuses point-to-point endpoint {} already assigned to {}",
                            config.id, key, existing
                        )));
                    }
                }
            }

            let id = config.id.clone();
            let source_path = path
                .strip_prefix(source_base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if connections
                .insert(
                    id.clone(),
                    LoadedConnection {
                        config,
                        source_path,
                        source_yaml,
                        source_file: path,
                    },
                )
                .is_some()
            {
                return Err(ConfigError::new(format!("duplicate connection id {id}")));
            }
        }
        Ok(Self { connections })
    }

    pub fn get(&self, id: &str) -> Option<&LoadedConnection> {
        self.connections.get(id)
    }

    pub fn connections(&self) -> impl Iterator<Item = &LoadedConnection> {
        self.connections.values()
    }

    pub fn len(&self) -> usize {
        self.connections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }

    pub fn frontend_connections(&self, appliances: &ConfigRepository) -> Vec<FrontendConnection> {
        self.connections
            .values()
            .map(|connection| FrontendConnection::new(connection, appliances))
            .collect()
    }

    pub fn appliance_index(&self) -> BTreeMap<String, Vec<String>> {
        let mut index: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for connection in self.connections.values() {
            for endpoint in [
                &connection.config.endpoints.a,
                &connection.config.endpoints.b,
            ] {
                index
                    .entry(endpoint.appliance.clone())
                    .or_default()
                    .push(connection.config.id.clone());
            }
        }
        index
    }
}

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
    fn new(loaded: &LoadedConnection, appliances: &ConfigRepository) -> Self {
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
            medium_detail: config.medium.detail(),
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
            physical_facts: config.medium.physical_facts(),
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

fn validate_endpoint(
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

fn validate_endpoint_port(
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

fn endpoint_port<'a>(
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

fn negotiated_duplex(a: PortDuplex, b: PortDuplex, medium: MediumKind) -> PortDuplex {
    if medium == MediumKind::Radio || a == PortDuplex::Half || b == PortDuplex::Half {
        PortDuplex::Half
    } else {
        PortDuplex::Full
    }
}

fn collect_yaml_paths(root: &Path, paths: &mut Vec<PathBuf>) -> Result<(), ConfigError> {
    let entries = fs::read_dir(root)
        .map_err(|error| ConfigError::new(format!("cannot read {}: {error}", root.display())))?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            ConfigError::new(format!(
                "cannot read entry under {}: {error}",
                root.display()
            ))
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_yaml_paths(&path, paths)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("yaml") {
            paths.push(path);
        }
    }
    Ok(())
}

const fn default_capacity() -> u64 {
    1_000
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use hearthline_model::{ArpOperation, ArpPacket, MacAddress, NetworkPayload, VlanId};

    use crate::media::{CopperCategory, CopperMedium, CopperWiring, RadioMedium, VirtualMedium};

    use super::*;

    fn endpoint(appliance: &str, interface: &str) -> ConnectionEndpoint {
        ConnectionEndpoint {
            appliance: appliance.into(),
            interface: interface.into(),
        }
    }

    fn frame() -> EthernetFrame {
        EthernetFrame {
            source: MacAddress::new([0x02, 0, 0, 0, 0, 1]),
            destination: MacAddress::new([0x02, 0, 0, 0, 0, 2]),
            vlan: VlanId::new(10).expect("valid VLAN"),
            payload: NetworkPayload::Arp(ArpPacket {
                operation: ArpOperation::Request,
                sender_mac: MacAddress::new([0x02, 0, 0, 0, 0, 1]),
                sender_ip: Ipv4Addr::new(192, 0, 2, 1),
                target_mac: None,
                target_ip: Ipv4Addr::new(192, 0, 2, 2),
            }),
        }
    }

    #[test]
    fn connector_applies_serialization_and_fixed_delay() {
        let a = endpoint("switch-01", "ethernet-1");
        let b = endpoint("router-01", "ethernet-1");
        let mut connector = SimulatedConnector::new(
            ComponentId::new("connection-01").expect("valid ID"),
            ConnectionEndpoints {
                a: a.clone(),
                b: b.clone(),
            },
            ConnectionProperties {
                capacity_mbps: 1,
                latency_ms: 2,
                ..ConnectionProperties::default()
            },
        )
        .expect("valid connector");

        let (destination, transit) = connector
            .transmit(&a, &frame(), 1_500)
            .expect("frame should transit");
        assert_eq!(destination, &b);
        assert_eq!(transit.delay_ms, 14);
    }

    #[test]
    fn connector_enforces_mtu_loss_and_operational_state() {
        let a = endpoint("switch-01", "ethernet-1");
        let b = endpoint("router-01", "ethernet-1");
        let mut connector = SimulatedConnector::new_configured(
            ComponentId::new("connection-01").expect("valid ID"),
            ConnectionEndpoints { a: a.clone(), b },
            ConnectionProperties {
                loss_every: Some(2),
                ..ConnectionProperties::default()
            },
            ConnectionMedium::Virtual {
                config: VirtualMedium {
                    technology: "test bridge".into(),
                },
            },
            ConnectorPortProfile {
                mtu: 100,
                ..ConnectorPortProfile::default()
            },
            ConnectorPortProfile {
                mtu: 100,
                ..ConnectorPortProfile::default()
            },
        )
        .expect("valid connector");

        assert_eq!(
            connector.transmit(&a, &frame(), 101),
            Err(ConnectorDropReason::MtuExceeded {
                frame_bytes: 101,
                mtu: 100
            })
        );
        assert!(connector.transmit(&a, &frame(), 100).is_ok());
        assert_eq!(
            connector.transmit(&a, &frame(), 100),
            Err(ConnectorDropReason::ModeledLoss)
        );
        connector.set_operational(false);
        assert_eq!(
            connector.transmit(&a, &frame(), 100),
            Err(ConnectorDropReason::Down)
        );
    }

    #[test]
    fn physical_media_require_exclusive_ports() {
        assert!(
            ConnectionMedium::Copper {
                config: CopperMedium {
                    wiring: CopperWiring::StraightThrough,
                    category: CopperCategory::Cat6a,
                    length_m: 10.0,
                },
            }
            .requires_exclusive_endpoints()
        );
        assert!(
            !ConnectionMedium::Virtual {
                config: VirtualMedium {
                    technology: "test bridge".into(),
                },
            }
            .requires_exclusive_endpoints()
        );
        assert!(
            !ConnectionMedium::Radio {
                config: RadioMedium {
                    standard: "IEEE 802.11ax".into(),
                    ssid: "test".into(),
                    security: "WPA3-Enterprise".into(),
                    distance_m: 10.0,
                },
            }
            .requires_exclusive_endpoints()
        );
    }

    #[test]
    fn transport_rejects_incompatible_medium() {
        let source = r#"
schema_version: "0.2.0"
id: "invalid-radio-link"
label: "Invalid radio link"
transport: "ethernet"
medium:
  type: "radio"
  standard: "IEEE 802.11ax"
  ssid: "test"
  security: "WPA3-Enterprise"
  distance_m: 10.0
endpoints:
  a:
    appliance: "endpoint-a"
    interface: "radio-0"
  b:
    appliance: "endpoint-b"
    interface: "radio-0"
properties:
  capacity_mbps: 300
"#;
        let error = ConnectionConfig::from_yaml(source).expect_err("must reject mismatch");
        assert!(error.to_string().contains("incompatible"));
    }

    #[test]
    fn half_duplex_does_not_imply_one_way_transit() {
        let a = endpoint("remote-io-01", "field-io-01");
        let b = endpoint("sensor-01", "field-io");
        let mut connector = SimulatedConnector::new_configured(
            ComponentId::new("field-connection-01").expect("valid ID"),
            ConnectionEndpoints {
                a: a.clone(),
                b: b.clone(),
            },
            ConnectionProperties::default(),
            ConnectionMedium::Virtual {
                config: VirtualMedium {
                    technology: "test field bus".into(),
                },
            },
            ConnectorPortProfile {
                duplex: PortDuplex::Half,
                ..ConnectorPortProfile::default()
            },
            ConnectorPortProfile::default(),
        )
        .expect("valid connector");

        assert!(connector.transmit(&a, &frame(), 64).is_ok());
        assert!(connector.transmit(&b, &frame(), 64).is_ok());
    }

    #[test]
    fn connector_rejects_frames_when_either_port_is_down() {
        let a = endpoint("switch-01", "ethernet-1");
        let b = endpoint("router-01", "ethernet-1");
        let mut connector = SimulatedConnector::new_configured(
            ComponentId::new("connection-01").expect("valid ID"),
            ConnectionEndpoints {
                a: a.clone(),
                b: b.clone(),
            },
            ConnectionProperties::default(),
            ConnectionMedium::Virtual {
                config: VirtualMedium {
                    technology: "test bridge".into(),
                },
            },
            ConnectorPortProfile {
                state: PortStateConfig {
                    administrative: PortState::Down,
                    initial_operational: PortState::Down,
                },
                ..ConnectorPortProfile::default()
            },
            ConnectorPortProfile::default(),
        )
        .expect("valid connector");

        assert_eq!(
            connector.transmit(&a, &frame(), 64),
            Err(ConnectorDropReason::SourcePortDown)
        );
        assert_eq!(
            connector.transmit(&b, &frame(), 64),
            Err(ConnectorDropReason::DestinationPortDown)
        );
    }

    #[test]
    fn copper_medium_adds_physical_propagation_delay() {
        let a = endpoint("switch-01", "ethernet-1");
        let b = endpoint("router-01", "ethernet-1");
        let mut connector = SimulatedConnector::new_configured(
            ComponentId::new("connection-01").expect("valid ID"),
            ConnectionEndpoints { a: a.clone(), b },
            ConnectionProperties::default(),
            ConnectionMedium::Copper {
                config: CopperMedium {
                    wiring: CopperWiring::StraightThrough,
                    category: CopperCategory::Cat6a,
                    length_m: 100.0,
                },
            },
            ConnectorPortProfile::default(),
            ConnectorPortProfile::default(),
        )
        .expect("valid connector");

        let (_, transit) = connector
            .transmit(&a, &frame(), 64)
            .expect("frame should transit");
        assert_eq!(transit.physical_delay_us, 1);
    }

    #[test]
    fn radio_medium_resolves_to_half_duplex() {
        assert_eq!(
            negotiated_duplex(PortDuplex::Auto, PortDuplex::Auto, MediumKind::Radio),
            PortDuplex::Half
        );
        assert_eq!(
            negotiated_duplex(PortDuplex::Auto, PortDuplex::Auto, MediumKind::Copper),
            PortDuplex::Full
        );
    }

    #[test]
    fn directional_connector_rejects_reverse_transit() {
        let a = endpoint("switch-01", "span");
        let b = endpoint("sensor-01", "capture");
        let mut connector = SimulatedConnector::new(
            ComponentId::new("mirror-connection-01").expect("valid ID"),
            ConnectionEndpoints {
                a: a.clone(),
                b: b.clone(),
            },
            ConnectionProperties {
                direction: ConnectionDirection::AToB,
                ..ConnectionProperties::default()
            },
        )
        .expect("valid connector");

        assert!(connector.transmit(&a, &frame(), 64).is_ok());
        assert_eq!(
            connector.transmit(&b, &frame(), 64),
            Err(ConnectorDropReason::InvalidEndpoint)
        );
    }
}
