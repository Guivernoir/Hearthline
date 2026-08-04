use std::collections::{BTreeMap, BTreeSet};

use hearthline_model::ComponentId;
use serde::{Deserialize, Serialize};

use crate::{ConfigError, ConnectionRepository, scenario::ScenarioConfig};

const MAX_CONNECTION_OVERRIDES: usize = 256;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioConnectionOverride {
    pub connection: String,
    pub operational: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScenarioConnectionState {
    pub id: String,
    pub label: String,
    pub endpoint_a: String,
    pub endpoint_b: String,
    pub configured_operational: bool,
    pub operational: bool,
}

pub(crate) fn validate_connection_override_syntax(
    overrides: &[ScenarioConnectionOverride],
) -> Result<(), ConfigError> {
    if overrides.len() > MAX_CONNECTION_OVERRIDES {
        return Err(ConfigError::new(format!(
            "scenario connection overrides exceed the {MAX_CONNECTION_OVERRIDES}-entry limit"
        )));
    }
    let mut ids = BTreeSet::new();
    for state in overrides {
        ComponentId::new(&state.connection).map_err(|error| ConfigError::new(error.to_string()))?;
        if !ids.insert(&state.connection) {
            return Err(ConfigError::new(format!(
                "scenario repeats connection override {}",
                state.connection
            )));
        }
    }
    Ok(())
}

pub(crate) fn scenario_connection_states(
    scenario: &ScenarioConfig,
    connections: &ConnectionRepository,
    request_overrides: Option<&[ScenarioConnectionOverride]>,
) -> Result<Vec<ScenarioConnectionState>, ConfigError> {
    validate_connection_override_syntax(&scenario.connection_overrides)?;
    if let Some(overrides) = request_overrides {
        validate_connection_override_syntax(overrides)?;
    }
    let participants = scenario.participants.iter().collect::<BTreeSet<_>>();
    let mut states = connections
        .connections()
        .filter(|connection| {
            participants.contains(&connection.config.endpoints.a.appliance)
                && participants.contains(&connection.config.endpoints.b.appliance)
        })
        .map(|connection| ScenarioConnectionState {
            id: connection.config.id.clone(),
            label: connection.config.label.clone(),
            endpoint_a: format!(
                "{}:{}",
                connection.config.endpoints.a.appliance, connection.config.endpoints.a.interface
            ),
            endpoint_b: format!(
                "{}:{}",
                connection.config.endpoints.b.appliance, connection.config.endpoints.b.interface
            ),
            configured_operational: connection.config.properties.operational,
            operational: connection.config.properties.operational,
        })
        .collect::<Vec<_>>();
    let indexes = states
        .iter()
        .enumerate()
        .map(|(index, state)| (state.id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    apply_overrides(
        &mut states,
        &indexes,
        &scenario.connection_overrides,
        &scenario.id,
    )?;
    if let Some(overrides) = request_overrides {
        apply_overrides(&mut states, &indexes, overrides, &scenario.id)?;
    }
    Ok(states)
}

fn apply_overrides(
    states: &mut [ScenarioConnectionState],
    indexes: &BTreeMap<String, usize>,
    overrides: &[ScenarioConnectionOverride],
    scenario_id: &str,
) -> Result<(), ConfigError> {
    for state in overrides {
        let index = indexes.get(state.connection.as_str()).ok_or_else(|| {
            ConfigError::new(format!(
                "scenario {scenario_id} connection override {} is not in its selected topology",
                state.connection
            ))
        })?;
        states[*index].operational = state.operational;
    }
    Ok(())
}
