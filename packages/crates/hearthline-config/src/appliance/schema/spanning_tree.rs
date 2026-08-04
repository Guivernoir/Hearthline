use std::fmt::{self, Display, Formatter};

use hearthline_model::{ComponentKind, MacAddress};
use serde::Deserialize;

use super::ConfigError;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpanningTreeConfig {
    pub protocol: SpanningTreeProtocol,
    pub bridge_priority: u16,
    pub bridge_mac: String,
}

impl SpanningTreeConfig {
    pub(super) fn validate(
        &self,
        appliance_id: &str,
        kind: ComponentKind,
    ) -> Result<(), ConfigError> {
        if !matches!(
            kind,
            ComponentKind::Layer2Switch | ComponentKind::Layer3Switch
        ) {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} kind {kind} cannot run spanning tree"
            )));
        }
        if self.bridge_priority > 61_440 || !self.bridge_priority.is_multiple_of(4_096) {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} spanning-tree priority {} must be a multiple of 4096 between 0 and 61440",
                self.bridge_priority
            )));
        }
        let bridge_mac = self.bridge_mac.parse::<MacAddress>().map_err(|error| {
            ConfigError::new(format!(
                "appliance {appliance_id} has invalid spanning-tree bridge MAC: {error}"
            ))
        })?;
        if !bridge_mac.is_unicast() {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} spanning-tree bridge MAC must be unicast"
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum SpanningTreeProtocol {
    RapidPvst,
}

impl Display for SpanningTreeProtocol {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::RapidPvst => formatter.write_str("rapid-pvst"),
        }
    }
}
