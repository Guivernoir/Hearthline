use serde::{Deserialize, Serialize};

use crate::ConfigError;

use super::super::{ScenarioExpectation, require_value};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioRecoveryConfig {
    pub label: String,
    pub summary: String,
    #[serde(default)]
    pub connection_overrides: Vec<crate::ScenarioConnectionOverride>,
    #[serde(default)]
    pub first_hop_overrides: Vec<crate::ScenarioFirstHopOverride>,
    #[serde(default)]
    pub firewall_ha_overrides: Vec<crate::ScenarioFirewallHaOverride>,
    pub expectation: ScenarioExpectation,
}

impl ScenarioRecoveryConfig {
    pub(in crate::scenario::schema) fn validate(&self) -> Result<(), ConfigError> {
        require_value("scenario recovery label", &self.label)?;
        require_value("scenario recovery summary", &self.summary)?;
        if self.connection_overrides.is_empty()
            && self.first_hop_overrides.is_empty()
            && self.firewall_ha_overrides.is_empty()
        {
            return Err(ConfigError::new(
                "scenario recovery requires a connection or first-hop override",
            ));
        }
        crate::scenario::connection::validate_connection_override_syntax(
            &self.connection_overrides,
        )?;
        crate::scenario::first_hop::validate_first_hop_override_syntax(&self.first_hop_overrides)?;
        crate::scenario::firewall_ha::validate_firewall_ha_override_syntax(
            &self.firewall_ha_overrides,
        )?;
        self.expectation.validate()
    }

    pub(in crate::scenario::schema) fn matches(
        &self,
        connection_states: &[crate::ScenarioConnectionState],
        first_hop_states: &[crate::ScenarioFirstHopState],
        firewall_ha_states: &[crate::ScenarioFirewallHaState],
    ) -> bool {
        self.connection_overrides.iter().all(|override_state| {
            connection_states.iter().any(|state| {
                state.id == override_state.connection
                    && state.operational == override_state.operational
            })
        }) && self.first_hop_overrides.iter().all(|override_state| {
            first_hop_states.iter().any(|state| {
                state.appliance == override_state.appliance
                    && state.interface == override_state.interface
                    && state.role == override_state.role
            })
        }) && self.firewall_ha_overrides.iter().all(|override_state| {
            firewall_ha_states.iter().any(|state| {
                state.appliance == override_state.appliance && state.role == override_state.role
            })
        })
    }
}
