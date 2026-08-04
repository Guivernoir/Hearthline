use std::net::Ipv4Addr;

use hearthline_model::{ComponentId, PortId, VlanId};

use crate::appliance::{ConfigError, InterfaceConfig};

pub(super) fn interface_vlan(interface: &InterfaceConfig) -> Result<VlanId, ConfigError> {
    vlan_id(interface.vlans.first().copied().unwrap_or(1))
}

pub(super) fn component_id(value: &str) -> Result<ComponentId, ConfigError> {
    ComponentId::new(value).map_err(|error| ConfigError::new(error.to_string()))
}

pub(super) fn port_id(value: &str) -> Result<PortId, ConfigError> {
    PortId::new(value).map_err(|error| ConfigError::new(error.to_string()))
}

pub(super) fn vlan_id(value: u16) -> Result<VlanId, ConfigError> {
    VlanId::new(value).ok_or_else(|| ConfigError::new(format!("invalid VLAN {value}")))
}

pub(in crate::runtime) fn parse_ipv4(value: &str, field: &str) -> Result<Ipv4Addr, ConfigError> {
    value
        .parse()
        .map_err(|_| ConfigError::new(format!("invalid {field} {value}")))
}
