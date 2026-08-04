use std::fmt::{self, Display, Formatter};
use std::net::Ipv4Addr;

use hearthline_model::{Ipv4InterfaceAddress, MacAddress};
use serde::{Deserialize, Serialize};

use crate::appliance::ConfigError;

use super::InterfaceMode;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FirstHopProtocol {
    Vrrp,
}

impl Display for FirstHopProtocol {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("vrrp")
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FirstHopRole {
    Active,
    Standby,
}

impl FirstHopRole {
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }
}

impl Display for FirstHopRole {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => formatter.write_str("active"),
            Self::Standby => formatter.write_str("standby"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FirstHopConfig {
    pub protocol: FirstHopProtocol,
    pub group: u8,
    pub virtual_ip: String,
    pub virtual_mac: String,
    pub priority: u8,
    pub preempt: bool,
    pub initial_role: FirstHopRole,
}

impl FirstHopConfig {
    pub(super) fn validate(
        &self,
        appliance_id: &str,
        interface_id: &str,
        mode: InterfaceMode,
        physical_mac: Option<MacAddress>,
        addresses: &[Ipv4InterfaceAddress],
    ) -> Result<(), ConfigError> {
        if !matches!(mode, InterfaceMode::Routed | InterfaceMode::Svi) {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} interface {interface_id} configures first-hop redundancy on a non-routed interface"
            )));
        }
        if self.group == 0 {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} interface {interface_id} requires a non-zero VRRP group"
            )));
        }
        if !(1..=254).contains(&self.priority) {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} interface {interface_id} VRRP priority must be between 1 and 254"
            )));
        }
        let virtual_ip = self.virtual_ip.parse::<Ipv4Addr>().map_err(|_| {
            ConfigError::new(format!(
                "appliance {appliance_id} interface {interface_id} has invalid first-hop address {}",
                self.virtual_ip
            ))
        })?;
        if addresses.is_empty()
            || !addresses
                .iter()
                .any(|address| address.is_on_link(virtual_ip))
        {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} interface {interface_id} first-hop address {virtual_ip} is not on-link"
            )));
        }
        if addresses
            .iter()
            .any(|address| address.address() == virtual_ip)
        {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} interface {interface_id} first-hop address {virtual_ip} must differ from its physical address"
            )));
        }
        let virtual_mac = self.virtual_mac.parse::<MacAddress>().map_err(|error| {
            ConfigError::new(format!(
                "appliance {appliance_id} interface {interface_id} has invalid first-hop MAC: {error}"
            ))
        })?;
        let expected_mac = MacAddress::new([0x00, 0x00, 0x5e, 0x00, 0x01, self.group]);
        if virtual_mac != expected_mac {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} interface {interface_id} VRRP group {} requires virtual MAC {expected_mac}",
                self.group
            )));
        }
        if physical_mac == Some(virtual_mac) {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} interface {interface_id} physical and virtual MAC addresses must differ"
            )));
        }
        Ok(())
    }
}
