use std::collections::BTreeSet;

use hearthline_model::{ComponentId, ComponentKind};

use crate::{BehaviorConfig, ConfigError, ConfigRepository};

use super::HmiSession;

pub(crate) fn validate_behavior(
    behavior: &BehaviorConfig,
    appliance_id: &str,
) -> Result<(), ConfigError> {
    match behavior {
        BehaviorConfig::OperatorInterface {
            controller,
            permissions,
            command_tags,
        } => {
            ComponentId::new(controller).map_err(|error| ConfigError::new(error.to_string()))?;
            if permissions.is_empty() {
                return Err(ConfigError::new(format!(
                    "operator interface {appliance_id} requires at least one permission"
                )));
            }
            require_unique_values(appliance_id, "permission", permissions)?;
            require_unique_ids(appliance_id, "command tag", command_tags)
        }
        BehaviorConfig::FieldSensor {
            minimum,
            maximum,
            initial_value: Some(initial_value),
            ..
        } if minimum < maximum
            && (!initial_value.is_finite()
                || initial_value < minimum
                || initial_value > maximum) =>
        {
            Err(ConfigError::new(format!(
                "sensor {appliance_id} initial value must be within {minimum}..={maximum}"
            )))
        }
        BehaviorConfig::FieldActuator {
            safe_state, states, ..
        } => {
            require_unique_values(appliance_id, "actuator state", states)?;
            if !states.is_empty() && !states.contains(safe_state) {
                return Err(ConfigError::new(format!(
                    "actuator {appliance_id} safe state {safe_state} is not in its configured states"
                )));
            }
            Ok(())
        }
        BehaviorConfig::Safety {
            permissives,
            initially_permissive,
            ..
        } => {
            require_unique_values(appliance_id, "permissive", permissives)?;
            require_unique_values(appliance_id, "initially permissive", initially_permissive)?;
            if let Some(unknown) = initially_permissive
                .iter()
                .find(|candidate| !permissives.contains(candidate))
            {
                return Err(ConfigError::new(format!(
                    "safety interface {appliance_id} initializes unknown permissive {unknown}"
                )));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(crate) fn validate_repository(appliances: &ConfigRepository) -> Result<(), ConfigError> {
    for loaded in appliances.appliances().filter(|loaded| {
        loaded.config.kind == ComponentKind::Hmi
            && loaded.config.tags.iter().any(|tag| tag == "interactive")
    }) {
        HmiSession::from_repository(appliances, &loaded.config.id)?;
    }
    Ok(())
}

fn require_unique_ids(
    appliance_id: &str,
    field: &str,
    values: &[String],
) -> Result<(), ConfigError> {
    for value in values {
        ComponentId::new(value).map_err(|error| ConfigError::new(error.to_string()))?;
    }
    require_unique_values(appliance_id, field, values)
}

fn require_unique_values(
    appliance_id: &str,
    field: &str,
    values: &[String],
) -> Result<(), ConfigError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if value.trim().is_empty() {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} has an empty {field}"
            )));
        }
        if !seen.insert(value) {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} repeats {field} {value}"
            )));
        }
    }
    Ok(())
}
