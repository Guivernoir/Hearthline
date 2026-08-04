use hearthline_model::ComponentId;
use serde::{Deserialize, Serialize};

use crate::ConfigError;
use crate::runtime::parse_service_kind;

use super::require_value;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioExpectation {
    pub component: String,
    pub outcome: ScenarioExpectedOutcome,
    #[serde(default)]
    pub service: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub reason_contains: Option<String>,
}

impl ScenarioExpectation {
    pub(super) fn validate(&self) -> Result<(), ConfigError> {
        ComponentId::new(&self.component).map_err(|error| ConfigError::new(error.to_string()))?;
        match self.outcome {
            ScenarioExpectedOutcome::Delivered | ScenarioExpectedOutcome::Forwarded => {
                let service = self.service.as_deref().ok_or_else(|| {
                    ConfigError::new("delivered or forwarded scenario expectation requires service")
                })?;
                parse_service_kind(service)?;
                if self.outcome == ScenarioExpectedOutcome::Forwarded {
                    let target = self.target.as_deref().ok_or_else(|| {
                        ConfigError::new("forwarded scenario expectation requires target")
                    })?;
                    ComponentId::new(target)
                        .map_err(|error| ConfigError::new(error.to_string()))?;
                }
            }
            ScenarioExpectedOutcome::Dropped => {
                if let Some(reason) = &self.reason_contains {
                    require_value("expected drop reason", reason)?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScenarioExpectedOutcome {
    Delivered,
    Forwarded,
    Dropped,
}
