use std::collections::BTreeSet;
use std::fmt::{self, Display, Formatter};
use std::net::Ipv4Addr;

use hearthline_model::{
    BehaviorFamily, ComponentId, ComponentKind, Ipv4InterfaceAddress, MacAddress, VlanId,
};
use serde::Deserialize;

use hearthline_engine::{
    PortHardwareKind, PortSettings, PortState, PortStateConfig, appliance_supports_port,
};

use super::{BehaviorConfig, ConfigError, deserialize_component_kind, require_text};

mod firewall_ha;
mod first_hop;
mod link_aggregation;
mod multi_chassis;
mod presentation;
mod spanning_tree;

pub use firewall_ha::{FirewallHaConfig, FirewallHaRole};
pub use first_hop::{FirstHopConfig, FirstHopProtocol, FirstHopRole};
pub use link_aggregation::{
    LinkAggregationConfig, LinkAggregationGroupConfig, LinkAggregationMode, LinkAggregationProtocol,
};
pub use multi_chassis::{MultiChassisConfig, MultiChassisRole};
pub use presentation::{Lifecycle, RenderBinding, RenderMode};
pub use spanning_tree::{SpanningTreeConfig, SpanningTreeProtocol};

pub const APPLIANCE_SCHEMA_VERSION: &str = "0.9.0";
pub const FRONTEND_CATALOG_SCHEMA_VERSION: &str = "0.8.0";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplianceConfig {
    pub schema_version: String,
    pub id: String,
    pub label: String,
    #[serde(deserialize_with = "deserialize_component_kind")]
    pub kind: ComponentKind,
    pub site: String,
    pub environment: String,
    pub zone: String,
    pub role: String,
    pub summary: String,
    #[serde(default)]
    pub lifecycle: Lifecycle,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub default_gateway: Option<String>,
    #[serde(default)]
    pub spanning_tree: Option<SpanningTreeConfig>,
    #[serde(default)]
    pub link_aggregation: Option<LinkAggregationConfig>,
    #[serde(default)]
    pub multi_chassis: Option<MultiChassisConfig>,
    #[serde(default)]
    pub firewall_ha: Option<FirewallHaConfig>,
    #[serde(default)]
    pub render: Vec<RenderBinding>,
    #[serde(default)]
    pub interfaces: Vec<InterfaceConfig>,
    pub behavior: BehaviorConfig,
}

impl ApplianceConfig {
    pub fn from_yaml(source: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_yaml_ng::from_str(source)
            .map_err(|error| ConfigError::new(format!("invalid YAML: {error}")))?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != APPLIANCE_SCHEMA_VERSION {
            return Err(ConfigError::new(format!(
                "appliance {} uses schema {}, expected {}",
                self.id, self.schema_version, APPLIANCE_SCHEMA_VERSION
            )));
        }
        ComponentId::new(&self.id).map_err(|error| ConfigError::new(error.to_string()))?;
        require_text("label", &self.label)?;
        require_text("site", &self.site)?;
        require_text("environment", &self.environment)?;
        require_text("zone", &self.zone)?;
        require_text("role", &self.role)?;
        require_text("summary", &self.summary)?;

        let expected_family = self.kind.behavior_family();
        let configured_family = self.behavior.family();
        if expected_family != configured_family {
            return Err(ConfigError::new(format!(
                "appliance {} kind {} requires behavior family {}, not {}",
                self.id, self.kind, expected_family, configured_family
            )));
        }
        if let Some(spanning_tree) = &self.spanning_tree {
            spanning_tree.validate(&self.id, self.kind)?;
            if matches!(
                self.behavior,
                BehaviorConfig::EthernetSwitch {
                    spanning_tree: false,
                    ..
                }
            ) {
                return Err(ConfigError::new(format!(
                    "appliance {} defines a spanning-tree bridge while switching behavior disables the protocol",
                    self.id
                )));
            }
        }
        if let Some(link_aggregation) = &self.link_aggregation {
            link_aggregation.validate(&self.id, self.kind, &self.interfaces)?;
        }
        if let Some(multi_chassis) = &self.multi_chassis {
            multi_chassis.validate(
                &self.id,
                self.kind,
                &self.interfaces,
                self.link_aggregation.as_ref(),
            )?;
        }
        if let Some(firewall_ha) = &self.firewall_ha {
            firewall_ha.validate(&self.id, self.kind, &self.interfaces)?;
        }
        if let BehaviorConfig::Endpoint {
            hostname,
            dns_servers,
            ..
        } = &self.behavior
        {
            if let Some(hostname) = hostname {
                require_text("endpoint hostname", hostname)?;
            }
            for server in dns_servers {
                server.parse::<Ipv4Addr>().map_err(|_| {
                    ConfigError::new(format!(
                        "appliance {} has invalid DNS server {}",
                        self.id, server
                    ))
                })?;
            }
        }
        let dns_records = self.behavior.dns_records();
        if self.kind == ComponentKind::DnsServer && dns_records.is_empty() {
            return Err(ConfigError::new(format!(
                "DNS appliance {} requires at least one authoritative record",
                self.id
            )));
        }
        if self.kind != ComponentKind::DnsServer && !dns_records.is_empty() {
            return Err(ConfigError::new(format!(
                "appliance {} defines DNS records but is not a DNS server",
                self.id
            )));
        }

        let mut interface_ids = BTreeSet::new();
        let mut interface_macs = BTreeSet::new();
        let mut assigned_addresses = BTreeSet::new();
        let mut parsed_addresses = Vec::new();
        let mut svi_vlans = BTreeSet::new();
        for interface in &self.interfaces {
            ComponentId::new(&interface.id).map_err(|error| {
                ConfigError::new(format!(
                    "appliance {} has invalid interface id: {error}",
                    self.id
                ))
            })?;
            if !interface_ids.insert(&interface.id) {
                return Err(ConfigError::new(format!(
                    "appliance {} repeats interface {}",
                    self.id, interface.id
                )));
            }
            if !appliance_supports_port(self.kind, interface.hardware) {
                return Err(ConfigError::new(format!(
                    "appliance {} kind {} does not support {} port {}",
                    self.id, self.kind, interface.hardware, interface.id
                )));
            }
            if interface.mode == InterfaceMode::Svi {
                if self.kind != ComponentKind::Layer3Switch
                    || interface.hardware != PortHardwareKind::VirtualNic
                    || interface.mac_address.is_none()
                    || interface.addresses.is_empty()
                    || interface.vlans.len() != 1
                {
                    return Err(ConfigError::new(format!(
                        "appliance {} SVI {} requires a layer-3 switch, virtual NIC, MAC address, IPv4 address, and exactly one VLAN",
                        self.id, interface.id
                    )));
                }
                if !svi_vlans.insert(interface.vlans[0]) {
                    return Err(ConfigError::new(format!(
                        "appliance {} defines more than one SVI for VLAN {}",
                        self.id, interface.vlans[0]
                    )));
                }
            }
            let parsed_mac = interface
                .mac_address
                .as_deref()
                .map(|value| {
                    value.parse::<MacAddress>().map_err(|error| {
                        ConfigError::new(format!(
                            "appliance {} port {} has invalid MAC address: {error}",
                            self.id, interface.id
                        ))
                    })
                })
                .transpose()?;
            if let Some(mac) = parsed_mac {
                if !mac.is_unicast() {
                    return Err(ConfigError::new(format!(
                        "appliance {} port {} requires a unicast MAC address",
                        self.id, interface.id
                    )));
                }
                if !interface_macs.insert(mac) {
                    return Err(ConfigError::new(format!(
                        "appliance {} repeats MAC address {mac}",
                        self.id
                    )));
                }
            }
            if interface.state.administrative == PortState::Down
                && interface.state.initial_operational == PortState::Up
            {
                return Err(ConfigError::new(format!(
                    "appliance {} port {} cannot be operationally up while administratively down",
                    self.id, interface.id
                )));
            }
            interface.settings.validate().map_err(|error| {
                ConfigError::new(format!(
                    "appliance {} port {}: {error}",
                    self.id, interface.id
                ))
            })?;
            if interface.settings.mtu > u32::from(u16::MAX) {
                return Err(ConfigError::new(format!(
                    "appliance {} port {} MTU exceeds the runtime limit",
                    self.id, interface.id
                )));
            }
            let mut interface_addresses = Vec::new();
            for value in &interface.addresses {
                let address = value.parse::<Ipv4InterfaceAddress>().map_err(|error| {
                    ConfigError::new(format!(
                        "appliance {} port {} has invalid address {}: {error}",
                        self.id, interface.id, value
                    ))
                })?;
                if !assigned_addresses.insert(address.address()) {
                    return Err(ConfigError::new(format!(
                        "appliance {} repeats IPv4 address {}",
                        self.id,
                        address.address()
                    )));
                }
                interface_addresses.push(address);
                parsed_addresses.push(address);
            }
            if let Some(first_hop) = &interface.first_hop {
                first_hop.validate(
                    &self.id,
                    &interface.id,
                    interface.mode,
                    parsed_mac,
                    &interface_addresses,
                )?;
            }
            let mut vlans = BTreeSet::new();
            for vlan in &interface.vlans {
                if VlanId::new(*vlan).is_none() {
                    return Err(ConfigError::new(format!(
                        "appliance {} port {} has invalid VLAN {}",
                        self.id, interface.id, vlan
                    )));
                }
                if !vlans.insert(*vlan) {
                    return Err(ConfigError::new(format!(
                        "appliance {} port {} repeats VLAN {}",
                        self.id, interface.id, vlan
                    )));
                }
            }
        }
        if let Some(value) = &self.default_gateway {
            let gateway = value.parse::<Ipv4Addr>().map_err(|_| {
                ConfigError::new(format!(
                    "appliance {} has invalid default gateway {}",
                    self.id, value
                ))
            })?;
            if assigned_addresses.contains(&gateway) {
                return Err(ConfigError::new(format!(
                    "appliance {} default gateway cannot be a local address",
                    self.id
                )));
            }
            if !parsed_addresses
                .iter()
                .any(|address| address.is_on_link(gateway))
            {
                return Err(ConfigError::new(format!(
                    "appliance {} default gateway {} is not on-link",
                    self.id, gateway
                )));
            }
        }

        let mut bindings = BTreeSet::new();
        for binding in &self.render {
            require_text("render.view", &binding.view)?;
            require_text("render.node", &binding.node)?;
            if !bindings.insert((&binding.view, &binding.node, binding.mode)) {
                return Err(ConfigError::new(format!(
                    "appliance {} repeats render binding {}:{}:{:?}",
                    self.id, binding.view, binding.node, binding.mode
                )));
            }
        }

        self.behavior.validate(&self.id)
    }

    pub fn behavior_family(&self) -> BehaviorFamily {
        self.behavior.family()
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterfaceConfig {
    pub id: String,
    pub hardware: PortHardwareKind,
    #[serde(default)]
    pub mac_address: Option<String>,
    pub state: PortStateConfig,
    pub settings: PortSettings,
    pub mode: InterfaceMode,
    #[serde(default)]
    pub addresses: Vec<String>,
    #[serde(default)]
    pub vlans: Vec<u16>,
    #[serde(default)]
    pub first_hop: Option<FirstHopConfig>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RouteConfig {
    pub destination: String,
    pub next_hop: Option<String>,
    pub interface: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NatTranslationConfig {
    pub public_address: String,
    pub private_address: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyAction {
    Permit,
    Deny,
}

impl Display for PolicyAction {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Permit => formatter.write_str("permit"),
            Self::Deny => formatter.write_str("deny"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PolicyRuleConfig {
    pub name: String,
    pub action: PolicyAction,
    #[serde(default)]
    pub source_zone: Option<String>,
    #[serde(default)]
    pub destination_zone: Option<String>,
    pub source: String,
    pub destination: String,
    pub service: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FirewallZoneConfig {
    pub interface: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListenerConfig {
    pub protocol: String,
    pub port: u16,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationUpstreamConfig {
    pub id: String,
    pub address: String,
}

impl ApplicationUpstreamConfig {
    pub(super) fn validate(&self, appliance_id: &str) -> Result<(), ConfigError> {
        ComponentId::new(&self.id).map_err(|error| ConfigError::new(error.to_string()))?;
        self.address.parse::<Ipv4Addr>().map_err(|_| {
            ConfigError::new(format!(
                "application gateway {appliance_id} has invalid upstream address {}",
                self.address
            ))
        })?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpSiteConfig {
    pub host: String,
    pub title: String,
    pub heading: String,
    pub body: String,
}

impl HttpSiteConfig {
    pub(super) fn validate(&self, appliance_id: &str) -> Result<(), ConfigError> {
        require_text("HTTP site host", &self.host)?;
        require_text("HTTP site title", &self.title)?;
        require_text("HTTP site heading", &self.heading)?;
        require_text("HTTP site body", &self.body)?;
        if self.host.len() > 128
            || self.title.len() > 96
            || self.heading.len() > 128
            || self.body.len() > 256
        {
            return Err(ConfigError::new(format!(
                "HTTP site content on {appliance_id} exceeds the runtime text capacity"
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum InterfaceMode {
    Access,
    Trunk,
    Routed,
    Svi,
    Transparent,
    Management,
    Monitor,
    FieldIo,
}

impl Display for InterfaceMode {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Access => "access",
            Self::Trunk => "trunk",
            Self::Routed => "routed",
            Self::Svi => "svi",
            Self::Transparent => "transparent",
            Self::Management => "management",
            Self::Monitor => "monitor",
            Self::FieldIo => "field-io",
        };
        formatter.write_str(value)
    }
}
