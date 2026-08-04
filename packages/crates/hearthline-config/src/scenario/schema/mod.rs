use hearthline_model::ComponentId;
use serde::{Deserialize, Serialize};

use crate::ConfigError;

mod expectation;
mod packet;
mod resilience;
mod summary;

pub use expectation::{ScenarioExpectation, ScenarioExpectedOutcome};
pub use packet::{
    ScenarioApplicationConfig, ScenarioHttpMethod, ScenarioPacketConfig, ScenarioTransportConfig,
};
pub use resilience::{
    ScenarioContinuityConfig, ScenarioContinuityFault, ScenarioHaIsolationConfig,
    ScenarioLocalAutonomyConfig, ScenarioRecoveryConfig,
};
pub use summary::ScenarioSummary;

pub const SCENARIO_SCHEMA_VERSION: &str = "0.11.0";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioConfig {
    pub schema_version: String,
    pub id: String,
    pub label: String,
    pub summary: String,
    pub category: String,
    pub participants: Vec<String>,
    pub source: String,
    pub packet: ScenarioPacketConfig,
    #[serde(default)]
    pub connection_overrides: Vec<super::ScenarioConnectionOverride>,
    #[serde(default)]
    pub first_hop_overrides: Vec<super::ScenarioFirstHopOverride>,
    #[serde(default)]
    pub firewall_ha_overrides: Vec<super::ScenarioFirewallHaOverride>,
    #[serde(default)]
    pub recovery: Option<ScenarioRecoveryConfig>,
    #[serde(default)]
    pub continuity: Option<ScenarioContinuityConfig>,
    #[serde(default)]
    pub ha_isolation: Option<ScenarioHaIsolationConfig>,
    #[serde(default)]
    pub local_autonomy: Option<ScenarioLocalAutonomyConfig>,
    pub expectation: ScenarioExpectation,
    #[serde(default)]
    pub security: Option<super::ScenarioSecurityConfig>,
    pub event_limit: usize,
}

impl ScenarioConfig {
    pub fn from_yaml(source: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_yaml_ng::from_str(source)
            .map_err(|error| ConfigError::new(format!("invalid scenario YAML: {error}")))?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != SCENARIO_SCHEMA_VERSION {
            return Err(ConfigError::new(format!(
                "scenario {} uses schema {}, expected {}",
                self.id, self.schema_version, SCENARIO_SCHEMA_VERSION
            )));
        }
        ComponentId::new(&self.id).map_err(|error| ConfigError::new(error.to_string()))?;
        require_value("scenario label", &self.label)?;
        require_value("scenario summary", &self.summary)?;
        require_value("scenario category", &self.category)?;
        if self.participants.is_empty() {
            return Err(ConfigError::new(format!(
                "scenario {} requires at least one participant",
                self.id
            )));
        }
        let mut participants = std::collections::BTreeSet::new();
        for participant in &self.participants {
            ComponentId::new(participant).map_err(|error| ConfigError::new(error.to_string()))?;
            if !participants.insert(participant) {
                return Err(ConfigError::new(format!(
                    "scenario {} repeats participant {}",
                    self.id, participant
                )));
            }
        }
        if !participants.contains(&self.source) {
            return Err(ConfigError::new(format!(
                "scenario {} source {} is not a participant",
                self.id, self.source
            )));
        }
        if !participants.contains(&self.expectation.component) {
            return Err(ConfigError::new(format!(
                "scenario {} expectation component {} is not a participant",
                self.id, self.expectation.component
            )));
        }
        self.packet.validate()?;
        super::connection::validate_connection_override_syntax(&self.connection_overrides)?;
        super::first_hop::validate_first_hop_override_syntax(&self.first_hop_overrides)?;
        super::firewall_ha::validate_firewall_ha_override_syntax(&self.firewall_ha_overrides)?;
        self.expectation.validate()?;
        if usize::from(self.recovery.is_some())
            + usize::from(self.continuity.is_some())
            + usize::from(self.ha_isolation.is_some())
            + usize::from(self.local_autonomy.is_some())
            > 1
        {
            return Err(ConfigError::new(format!(
                "scenario {} cannot combine recovery, continuity, HA isolation, and local-autonomy contracts",
                self.id
            )));
        }
        if let Some(recovery) = &self.recovery {
            recovery.validate()?;
            if !participants.contains(&recovery.expectation.component) {
                return Err(ConfigError::new(format!(
                    "scenario {} recovery expectation component {} is not a participant",
                    self.id, recovery.expectation.component
                )));
            }
        }
        if let Some(continuity) = &self.continuity {
            continuity.validate(&self.packet)?;
            super::connection::validate_connection_override_syntax(
                &continuity.connection_overrides,
            )?;
            for (label, participant) in [
                ("failed appliance", &continuity.failed_appliance),
                ("continuation source", &continuity.source),
                (
                    "continuation expectation",
                    &continuity.expectation.component,
                ),
            ] {
                if !participants.contains(participant) {
                    return Err(ConfigError::new(format!(
                        "scenario {} {label} {participant} is not a participant",
                        self.id
                    )));
                }
            }
        }
        if let Some(isolation) = &self.ha_isolation {
            isolation.validate(&self.packet)?;
            super::connection::validate_connection_override_syntax(
                &isolation.connection_overrides,
            )?;
            for participant in [
                &isolation.standby_appliance,
                &isolation.source,
                &isolation.expectation.component,
            ] {
                if !participants.contains(participant) {
                    return Err(ConfigError::new(format!(
                        "scenario {} HA isolation reference {participant} is not a participant",
                        self.id
                    )));
                }
            }
        }
        if let Some(autonomy) = &self.local_autonomy {
            autonomy.validate()?;
            for participant in [
                &autonomy.hmi,
                &autonomy.safety_interface,
                &autonomy.actuator,
            ] {
                if !participants.contains(participant) {
                    return Err(ConfigError::new(format!(
                        "scenario {} local-autonomy reference {participant} is not a participant",
                        self.id
                    )));
                }
            }
        }
        if let Some(security) = &self.security {
            security.validate()?;
            if !participants.contains(&security.detector) {
                return Err(ConfigError::new(format!(
                    "scenario {} security detector {} is not a participant",
                    self.id, security.detector
                )));
            }
        }
        if !(1..=512).contains(&self.event_limit) {
            return Err(ConfigError::new(format!(
                "scenario {} event_limit must be between 1 and 512",
                self.id
            )));
        }
        Ok(())
    }

    pub fn summary(
        &self,
        connection_states: Vec<super::ScenarioConnectionState>,
        first_hop_states: Vec<super::ScenarioFirstHopState>,
        link_aggregation_states: Vec<super::ScenarioLinkAggregationState>,
        spanning_tree_states: Vec<super::ScenarioSpanningTreeState>,
        firewall_ha_states: Vec<super::ScenarioFirewallHaState>,
    ) -> ScenarioSummary {
        ScenarioSummary {
            schema_version: self.schema_version.clone(),
            id: self.id.clone(),
            label: self.label.clone(),
            summary: self.summary.clone(),
            category: self.category.clone(),
            participants: self.participants.clone(),
            source: self.source.clone(),
            packet: self.packet.clone(),
            connection_states,
            first_hop_states,
            link_aggregation_states,
            spanning_tree_states,
            firewall_ha_states,
            recovery: self.recovery.clone(),
            continuity: self.continuity.clone(),
            ha_isolation: self.ha_isolation.clone(),
            local_autonomy: self.local_autonomy.clone(),
            expectation: self.expectation.clone(),
            security: self.security.clone(),
        }
    }

    pub(super) fn active_expectation(
        &self,
        connection_states: &[super::ScenarioConnectionState],
        first_hop_states: &[super::ScenarioFirstHopState],
        firewall_ha_states: &[super::ScenarioFirewallHaState],
    ) -> (super::ScenarioExpectationMode, &ScenarioExpectation) {
        if let Some(recovery) = &self.recovery
            && recovery.matches(connection_states, first_hop_states, firewall_ha_states)
        {
            return (
                super::ScenarioExpectationMode::Recovery,
                &recovery.expectation,
            );
        }
        (super::ScenarioExpectationMode::Baseline, &self.expectation)
    }
}

pub(super) fn require_value(field: &str, value: &str) -> Result<(), ConfigError> {
    if value.trim().is_empty() {
        Err(ConfigError::new(format!("{field} cannot be empty")))
    } else {
        Ok(())
    }
}
