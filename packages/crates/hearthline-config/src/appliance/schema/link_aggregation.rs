use std::collections::BTreeSet;
use std::fmt::{self, Display, Formatter};

use hearthline_model::{ComponentId, ComponentKind, MacAddress};
use serde::Deserialize;

use super::{InterfaceConfig, InterfaceMode};
use crate::appliance::{ConfigError, require_text};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkAggregationConfig {
    pub system_mac: String,
    pub groups: Vec<LinkAggregationGroupConfig>,
}

impl LinkAggregationConfig {
    pub(super) fn validate(
        &self,
        appliance_id: &str,
        kind: ComponentKind,
        interfaces: &[InterfaceConfig],
    ) -> Result<(), ConfigError> {
        if !matches!(
            kind,
            ComponentKind::Layer2Switch | ComponentKind::Layer3Switch
        ) {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} kind {kind} cannot define link aggregation"
            )));
        }
        let system_mac = self.system_mac.parse::<MacAddress>().map_err(|error| {
            ConfigError::new(format!(
                "appliance {appliance_id} has invalid LACP system MAC: {error}"
            ))
        })?;
        if !system_mac.is_unicast() {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} requires a unicast LACP system MAC"
            )));
        }
        if self.groups.is_empty() {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} link aggregation requires at least one group"
            )));
        }

        let mut group_ids = BTreeSet::new();
        let mut logical_ids = BTreeSet::new();
        let mut assigned_members = BTreeSet::new();
        for group in &self.groups {
            group.validate(appliance_id, interfaces)?;
            if !group_ids.insert(&group.id) {
                return Err(ConfigError::new(format!(
                    "appliance {appliance_id} repeats link aggregation group {}",
                    group.id
                )));
            }
            if !logical_ids.insert(&group.logical_id) {
                return Err(ConfigError::new(format!(
                    "appliance {appliance_id} repeats logical aggregate {}",
                    group.logical_id
                )));
            }
            for member in &group.members {
                if !assigned_members.insert(member) {
                    return Err(ConfigError::new(format!(
                        "appliance {appliance_id} assigns interface {member} to more than one aggregate"
                    )));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkAggregationGroupConfig {
    pub id: String,
    pub logical_id: String,
    pub protocol: LinkAggregationProtocol,
    pub mode: LinkAggregationMode,
    #[serde(default = "default_minimum_active_members")]
    pub minimum_active_members: u8,
    pub members: Vec<String>,
}

impl LinkAggregationGroupConfig {
    fn validate(
        &self,
        appliance_id: &str,
        interfaces: &[InterfaceConfig],
    ) -> Result<(), ConfigError> {
        ComponentId::new(&self.id).map_err(|error| ConfigError::new(error.to_string()))?;
        ComponentId::new(&self.logical_id).map_err(|error| ConfigError::new(error.to_string()))?;
        require_text("link aggregation group id", &self.id)?;
        require_text("logical aggregate id", &self.logical_id)?;
        if self.members.is_empty() {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} aggregate {} requires at least one member",
                self.id
            )));
        }
        if self.minimum_active_members == 0
            || usize::from(self.minimum_active_members) > self.members.len()
        {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} aggregate {} minimum active members must be between 1 and {}",
                self.id,
                self.members.len()
            )));
        }
        let mut members = BTreeSet::new();
        for member in &self.members {
            ComponentId::new(member).map_err(|error| ConfigError::new(error.to_string()))?;
            if !members.insert(member) {
                return Err(ConfigError::new(format!(
                    "appliance {appliance_id} aggregate {} repeats member {member}",
                    self.id
                )));
            }
            let interface = interfaces
                .iter()
                .find(|interface| interface.id == *member)
                .ok_or_else(|| {
                    ConfigError::new(format!(
                        "appliance {appliance_id} aggregate {} references unknown interface {member}",
                        self.id
                    ))
                })?;
            if !matches!(interface.mode, InterfaceMode::Access | InterfaceMode::Trunk) {
                return Err(ConfigError::new(format!(
                    "appliance {appliance_id} aggregate {} member {member} must be an access or trunk interface",
                    self.id
                )));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum LinkAggregationProtocol {
    Lacp,
}

impl Display for LinkAggregationProtocol {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("lacp")
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum LinkAggregationMode {
    Active,
    Passive,
}

impl Display for LinkAggregationMode {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => formatter.write_str("active"),
            Self::Passive => formatter.write_str("passive"),
        }
    }
}

const fn default_minimum_active_members() -> u8 {
    1
}
