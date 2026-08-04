use std::collections::BTreeSet;
use std::fmt::{self, Display, Formatter};

use hearthline_model::{ComponentId, ComponentKind, MacAddress};
use serde::{Deserialize, Serialize};

use crate::appliance::ConfigError;

use super::{FirstHopRole, InterfaceConfig, InterfaceMode};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FirewallHaRole {
    Active,
    Standby,
}

impl FirewallHaRole {
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    const fn first_hop_role(self) -> FirstHopRole {
        match self {
            Self::Active => FirstHopRole::Active,
            Self::Standby => FirstHopRole::Standby,
        }
    }
}

impl Display for FirewallHaRole {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => formatter.write_str("active"),
            Self::Standby => formatter.write_str("standby"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FirewallHaConfig {
    pub domain: String,
    pub peer: String,
    pub role: FirewallHaRole,
    pub sync_interface: String,
    pub monitored_interfaces: Vec<String>,
    pub session_sync: bool,
    pub heartbeat_interval_ms: u64,
    pub failure_hold_ms: u64,
}

impl FirewallHaConfig {
    pub(super) fn validate(
        &self,
        appliance_id: &str,
        kind: ComponentKind,
        interfaces: &[InterfaceConfig],
    ) -> Result<(), ConfigError> {
        if kind != ComponentKind::Firewall {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} declares firewall HA but is not a firewall"
            )));
        }
        ComponentId::new(&self.domain).map_err(|error| ConfigError::new(error.to_string()))?;
        ComponentId::new(&self.peer).map_err(|error| ConfigError::new(error.to_string()))?;
        ComponentId::new(&self.sync_interface)
            .map_err(|error| ConfigError::new(error.to_string()))?;
        if self.peer == appliance_id {
            return Err(ConfigError::new(format!(
                "firewall {appliance_id} cannot be its own HA peer"
            )));
        }
        if !self.session_sync {
            return Err(ConfigError::new(format!(
                "stateful firewall HA domain {} must enable session synchronization",
                self.domain
            )));
        }
        if !(100..=60_000).contains(&self.heartbeat_interval_ms) {
            return Err(ConfigError::new(format!(
                "firewall HA domain {} heartbeat_interval_ms must be between 100 and 60000",
                self.domain
            )));
        }
        if self.failure_hold_ms < self.heartbeat_interval_ms.saturating_mul(3)
            || self.failure_hold_ms > 300_000
        {
            return Err(ConfigError::new(format!(
                "firewall HA domain {} failure_hold_ms must cover at least three heartbeats and not exceed 300000",
                self.domain
            )));
        }
        let sync_interface = interfaces
            .iter()
            .find(|interface| interface.id == self.sync_interface)
            .ok_or_else(|| {
                ConfigError::new(format!(
                    "firewall {appliance_id} HA sync interface {} does not exist",
                    self.sync_interface
                ))
            })?;
        if sync_interface.mode != InterfaceMode::Monitor
            || !sync_interface.addresses.is_empty()
            || sync_interface.first_hop.is_some()
        {
            return Err(ConfigError::new(format!(
                "firewall {appliance_id} HA sync interface {} must be an unaddressed monitor port",
                self.sync_interface
            )));
        }
        let sync_mac = sync_interface.mac_address.as_deref().ok_or_else(|| {
            ConfigError::new(format!(
                "firewall {appliance_id} HA sync interface {} requires a MAC address",
                self.sync_interface
            ))
        })?;
        if !sync_mac
            .parse::<MacAddress>()
            .is_ok_and(MacAddress::is_unicast)
        {
            return Err(ConfigError::new(format!(
                "firewall {appliance_id} HA sync interface {} requires a unicast MAC address",
                self.sync_interface
            )));
        }
        if self.monitored_interfaces.is_empty() {
            return Err(ConfigError::new(format!(
                "firewall {appliance_id} HA requires at least one monitored data interface"
            )));
        }
        let mut monitored = BTreeSet::new();
        for interface_id in &self.monitored_interfaces {
            if !monitored.insert(interface_id) {
                return Err(ConfigError::new(format!(
                    "firewall {appliance_id} repeats HA monitored interface {interface_id}"
                )));
            }
            let interface = interfaces
                .iter()
                .find(|interface| interface.id == *interface_id)
                .ok_or_else(|| {
                    ConfigError::new(format!(
                        "firewall {appliance_id} HA monitored interface {interface_id} does not exist"
                    ))
                })?;
            let first_hop = interface.first_hop.as_ref().ok_or_else(|| {
                ConfigError::new(format!(
                    "firewall {appliance_id} HA monitored interface {interface_id} requires a virtual first-hop identity"
                ))
            })?;
            if interface.mode != InterfaceMode::Routed
                || first_hop.initial_role != self.role.first_hop_role()
            {
                return Err(ConfigError::new(format!(
                    "firewall {appliance_id} HA role must match the virtual role on routed interface {interface_id}"
                )));
            }
        }
        if monitored.contains(&self.sync_interface) {
            return Err(ConfigError::new(format!(
                "firewall {appliance_id} cannot monitor its HA sync interface as a data path"
            )));
        }
        Ok(())
    }
}
