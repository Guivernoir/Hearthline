use core::fmt;

use hearthline_model::ComponentId;
use serde::{Deserialize, Serialize};

use crate::ConfigError;

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

fn require_value(field: &str, value: &str) -> Result<(), ConfigError> {
    if value.trim().is_empty() {
        Err(ConfigError::new(format!("{field} cannot be empty")))
    } else {
        Ok(())
    }
}
