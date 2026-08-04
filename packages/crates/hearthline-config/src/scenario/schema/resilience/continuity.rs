use hearthline_model::ComponentId;
use serde::{Deserialize, Serialize};

use crate::ConfigError;

use super::super::{ScenarioExpectation, ScenarioPacketConfig, ScenarioTransportConfig};

const MAX_CONTINUITY_FAULTS: usize = 4;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ScenarioContinuityFault {
    SyncLinkLoss { at_us: u64 },
    StandbySessionLoss { at_us: u64 },
}

impl ScenarioContinuityFault {
    pub const fn at_us(self) -> u64 {
        match self {
            Self::SyncLinkLoss { at_us } | Self::StandbySessionLoss { at_us } => at_us,
        }
    }

    pub const fn is_sync_link_loss(self) -> bool {
        matches!(self, Self::SyncLinkLoss { .. })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioContinuityConfig {
    pub failed_appliance: String,
    pub failure_at_us: u64,
    pub continuation_at_us: u64,
    pub source: String,
    pub packet: ScenarioPacketConfig,
    #[serde(default)]
    pub faults: Vec<ScenarioContinuityFault>,
    #[serde(default)]
    pub connection_overrides: Vec<crate::ScenarioConnectionOverride>,
    pub expectation: ScenarioExpectation,
}

impl ScenarioContinuityConfig {
    pub(in crate::scenario::schema) fn validate(
        &self,
        opening: &ScenarioPacketConfig,
    ) -> Result<(), ConfigError> {
        ComponentId::new(&self.failed_appliance)
            .map_err(|error| ConfigError::new(error.to_string()))?;
        ComponentId::new(&self.source).map_err(|error| ConfigError::new(error.to_string()))?;
        if self.failure_at_us == 0 || self.continuation_at_us <= self.failure_at_us {
            return Err(ConfigError::new(
                "scenario continuity requires a non-zero failure time and a later continuation time",
            ));
        }
        self.packet.validate()?;
        self.expectation.validate()?;
        if self.faults.len() > MAX_CONTINUITY_FAULTS {
            return Err(ConfigError::new(format!(
                "scenario continuity faults exceed the {MAX_CONTINUITY_FAULTS}-entry limit"
            )));
        }
        let mut previous_at_us = 0;
        let mut sync_loss = false;
        let mut session_loss = false;
        for fault in &self.faults {
            let at_us = fault.at_us();
            if at_us == 0 || at_us >= self.failure_at_us || at_us <= previous_at_us {
                return Err(ConfigError::new(
                    "scenario continuity faults must be strictly ordered between opening and appliance failure",
                ));
            }
            let repeated = match fault {
                ScenarioContinuityFault::SyncLinkLoss { .. } => {
                    core::mem::replace(&mut sync_loss, true)
                }
                ScenarioContinuityFault::StandbySessionLoss { .. } => {
                    core::mem::replace(&mut session_loss, true)
                }
            };
            if repeated {
                return Err(ConfigError::new(
                    "scenario continuity cannot repeat the same fault type",
                ));
            }
            previous_at_us = at_us;
        }

        let ScenarioTransportConfig::Tcp {
            syn: true,
            ack: false,
            fin: false,
            rst: false,
            ..
        } = opening.transport
        else {
            return Err(ConfigError::new(
                "scenario continuity opening packet must be a TCP SYN",
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
                "scenario continuity packet must be a reverse TCP ACK",
            ));
        };
        let opening_flow = opening.ipv4_packet()?.flow_key();
        let continuation_flow = self.packet.ipv4_packet()?.flow_key();
        if continuation_flow != opening_flow.reverse() {
            return Err(ConfigError::new(
                "scenario continuity packet must reverse the opening TCP flow",
            ));
        }
        Ok(())
    }
}
