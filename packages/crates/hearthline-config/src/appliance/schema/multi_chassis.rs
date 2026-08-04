use std::fmt::{self, Display, Formatter};

use hearthline_model::{ComponentId, ComponentKind};
use serde::Deserialize;

use super::{InterfaceConfig, InterfaceMode, LinkAggregationConfig};
use crate::appliance::ConfigError;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MultiChassisConfig {
    pub domain: String,
    pub peer: String,
    pub peer_link: String,
    pub role: MultiChassisRole,
}

impl MultiChassisConfig {
    pub(super) fn validate(
        &self,
        appliance_id: &str,
        kind: ComponentKind,
        interfaces: &[InterfaceConfig],
        link_aggregation: Option<&LinkAggregationConfig>,
    ) -> Result<(), ConfigError> {
        if !matches!(
            kind,
            ComponentKind::Layer2Switch | ComponentKind::Layer3Switch
        ) {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} kind {kind} cannot define a multi-chassis domain"
            )));
        }
        ComponentId::new(&self.domain).map_err(|error| ConfigError::new(error.to_string()))?;
        ComponentId::new(&self.peer).map_err(|error| ConfigError::new(error.to_string()))?;
        ComponentId::new(&self.peer_link).map_err(|error| ConfigError::new(error.to_string()))?;
        if self.peer == appliance_id {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} cannot be its own multi-chassis peer"
            )));
        }
        let peer_link = interfaces
            .iter()
            .find(|interface| interface.id == self.peer_link)
            .ok_or_else(|| {
                ConfigError::new(format!(
                    "appliance {appliance_id} multi-chassis peer link {} does not exist",
                    self.peer_link
                ))
            })?;
        if peer_link.mode != InterfaceMode::Trunk {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} multi-chassis peer link {} must be a trunk",
                self.peer_link
            )));
        }
        let link_aggregation = link_aggregation.ok_or_else(|| {
            ConfigError::new(format!(
                "appliance {appliance_id} multi-chassis domain requires link aggregation"
            ))
        })?;
        if link_aggregation
            .groups
            .iter()
            .any(|group| group.members.contains(&self.peer_link))
        {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} multi-chassis peer link {} cannot also be a downstream aggregate member",
                self.peer_link
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum MultiChassisRole {
    Primary,
    Secondary,
}

impl Display for MultiChassisRole {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primary => formatter.write_str("primary"),
            Self::Secondary => formatter.write_str("secondary"),
        }
    }
}
