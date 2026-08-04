use hearthline_model::ComponentId;
use serde::{Deserialize, Serialize};

use crate::ConfigError;

use super::super::require_value;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioLocalAutonomyConfig {
    pub hmi: String,
    pub safety_interface: String,
    pub actuator: String,
    pub command_tag: String,
    pub command_value: String,
    pub expected_actuator_state: String,
}

impl ScenarioLocalAutonomyConfig {
    pub(in crate::scenario::schema) fn validate(&self) -> Result<(), ConfigError> {
        for component in [&self.hmi, &self.safety_interface, &self.actuator] {
            ComponentId::new(component).map_err(|error| ConfigError::new(error.to_string()))?;
        }
        require_value("local-autonomy command tag", &self.command_tag)?;
        require_value("local-autonomy command value", &self.command_value)?;
        require_value(
            "local-autonomy expected actuator state",
            &self.expected_actuator_state,
        )?;
        if self.hmi == self.safety_interface
            || self.hmi == self.actuator
            || self.safety_interface == self.actuator
        {
            return Err(ConfigError::new(
                "local-autonomy HMI, safety interface, and actuator must be distinct",
            ));
        }
        Ok(())
    }
}
