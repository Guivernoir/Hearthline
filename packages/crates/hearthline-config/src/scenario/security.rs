use core::fmt;

use hearthline_engine::{Effect, TraceEntry};
use hearthline_model::ComponentId;
use serde::{Deserialize, Serialize};

use crate::ConfigError;

use super::ScenarioPacketConfig;

pub const SECURITY_EVENT_SCHEMA_VERSION: &str = "0.1.0";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioSecurityConfig {
    pub tactic: String,
    pub technique: String,
    pub severity: SecuritySeverity,
    pub detector: String,
    pub defender: String,
    pub control: String,
}

impl ScenarioSecurityConfig {
    pub(super) fn validate(&self) -> Result<(), ConfigError> {
        require_value("security tactic", &self.tactic)?;
        require_value("security technique", &self.technique)?;
        require_value("security control", &self.control)?;
        ComponentId::new(&self.detector).map_err(|error| ConfigError::new(error.to_string()))?;
        ComponentId::new(&self.defender).map_err(|error| ConfigError::new(error.to_string()))?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecuritySeverity {
    Informational,
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for SecuritySeverity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Informational => "informational",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecurityDisposition {
    Prevented,
    ControlFailed,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScenarioSecurityEvent {
    pub schema_version: &'static str,
    pub scenario_id: String,
    pub tactic: String,
    pub technique: String,
    pub severity: SecuritySeverity,
    pub detector: String,
    pub defender: String,
    pub control: String,
    pub disposition: SecurityDisposition,
    pub source_ip: String,
    pub destination_ip: String,
    pub observed_at_us: u64,
    pub evidence: String,
}

impl ScenarioSecurityEvent {
    pub(super) fn from_trace(
        scenario_id: String,
        config: ScenarioSecurityConfig,
        packet: &ScenarioPacketConfig,
        expectation_met: bool,
        trace: &[TraceEntry],
    ) -> Self {
        let detection = trace.iter().rev().find(|entry| {
            entry.component.as_str() == config.detector && matches!(entry.effect, Effect::Drop(_))
        });
        let disposition = if expectation_met && detection.is_some() {
            SecurityDisposition::Prevented
        } else {
            SecurityDisposition::ControlFailed
        };
        let evidence = detection.map_or_else(
            || "No configured detector drop was present in the simulation trace".into(),
            |entry| match &entry.effect {
                Effect::Drop(reason) => reason.to_string(),
                _ => unreachable!("detection search only accepts drop effects"),
            },
        );
        Self {
            schema_version: SECURITY_EVENT_SCHEMA_VERSION,
            scenario_id,
            tactic: config.tactic,
            technique: config.technique,
            severity: config.severity,
            detector: config.detector,
            defender: config.defender,
            control: config.control,
            disposition,
            source_ip: packet.source_ip.clone(),
            destination_ip: packet.destination_ip.clone(),
            observed_at_us: detection
                .or_else(|| trace.last())
                .map_or(0, |entry| entry.time_us),
            evidence,
        }
    }
}

fn require_value(field: &str, value: &str) -> Result<(), ConfigError> {
    if value.trim().is_empty() {
        Err(ConfigError::new(format!("{field} cannot be empty")))
    } else {
        Ok(())
    }
}
