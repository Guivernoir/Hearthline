use hearthline_model::ComponentId;
use serde::{Deserialize, Serialize};

use crate::ConfigError;

use super::super::{ScenarioExpectation, ScenarioPacketConfig, ScenarioTransportConfig};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioHaIsolationConfig {
    pub standby_appliance: String,
    pub isolation_at_us: u64,
    pub continuation_at_us: u64,
    pub source: String,
    pub packet: ScenarioPacketConfig,
    pub connection_overrides: Vec<crate::ScenarioConnectionOverride>,
    pub expectation: ScenarioExpectation,
}

impl ScenarioHaIsolationConfig {
    pub(in crate::scenario::schema) fn validate(
        &self,
        opening: &ScenarioPacketConfig,
    ) -> Result<(), ConfigError> {
        ComponentId::new(&self.standby_appliance)
            .map_err(|error| ConfigError::new(error.to_string()))?;
        ComponentId::new(&self.source).map_err(|error| ConfigError::new(error.to_string()))?;
        if self.isolation_at_us == 0 || self.continuation_at_us <= self.isolation_at_us {
            return Err(ConfigError::new(
                "scenario HA isolation requires a non-zero isolation time and a later continuation time",
            ));
        }
        self.packet.validate()?;
        self.expectation.validate()?;
        let ScenarioTransportConfig::Tcp {
            syn: true,
            ack: false,
            fin: false,
            rst: false,
            ..
        } = opening.transport
        else {
            return Err(ConfigError::new(
                "scenario HA isolation opening packet must be a TCP SYN",
            ));
        };
        let ScenarioTransportConfig::Tcp {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            ..
        } = self.packet.transport
        else {
            return Err(ConfigError::new(
                "scenario HA isolation packet must be a reverse TCP ACK",
            ));
        };
        if self.packet.ipv4_packet()?.flow_key() != opening.ipv4_packet()?.flow_key().reverse() {
            return Err(ConfigError::new(
                "scenario HA isolation packet must reverse the opening TCP flow",
            ));
        }
        Ok(())
    }
}
