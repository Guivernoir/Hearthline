use std::collections::{BTreeMap, BTreeSet};

use hearthline_model::ComponentId;
use serde::{Deserialize, Serialize};

use crate::{ConfigError, ConfigRepository, FirstHopRole};

use crate::scenario::ScenarioConfig;

const MAX_FIRST_HOP_OVERRIDES: usize = 64;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioFirstHopOverride {
    pub appliance: String,
    pub interface: String,
    pub role: FirstHopRole,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScenarioFirstHopState {
    pub appliance: String,
    pub interface: String,
    pub protocol: String,
    pub group: u8,
    pub virtual_ip: String,
    pub virtual_mac: String,
    pub priority: u8,
    pub preempt: bool,
    pub configured_role: FirstHopRole,
    pub role: FirstHopRole,
}

pub(crate) fn validate_first_hop_override_syntax(
    overrides: &[ScenarioFirstHopOverride],
) -> Result<(), ConfigError> {
    if overrides.len() > MAX_FIRST_HOP_OVERRIDES {
        return Err(ConfigError::new(format!(
            "scenario first-hop overrides exceed the {MAX_FIRST_HOP_OVERRIDES}-entry limit"
        )));
    }
    let mut identities = BTreeSet::new();
    for state in overrides {
        ComponentId::new(&state.appliance).map_err(|error| ConfigError::new(error.to_string()))?;
        ComponentId::new(&state.interface).map_err(|error| ConfigError::new(error.to_string()))?;
        if !identities.insert((&state.appliance, &state.interface)) {
            return Err(ConfigError::new(format!(
                "scenario repeats first-hop override {}:{}",
                state.appliance, state.interface
            )));
        }
    }
    Ok(())
}

pub(crate) fn scenario_first_hop_states(
    scenario: &ScenarioConfig,
    appliances: &ConfigRepository,
    request_overrides: Option<&[ScenarioFirstHopOverride]>,
) -> Result<Vec<ScenarioFirstHopState>, ConfigError> {
    validate_first_hop_override_syntax(&scenario.first_hop_overrides)?;
    if let Some(overrides) = request_overrides {
        validate_first_hop_override_syntax(overrides)?;
    }
    let mut states = Vec::new();
    for appliance_id in &scenario.participants {
        let appliance = appliances
            .get(appliance_id)
            .expect("scenario participant existence is validated before state projection");
        for interface in &appliance.config.interfaces {
            let Some(first_hop) = &interface.first_hop else {
                continue;
            };
            states.push(ScenarioFirstHopState {
                appliance: appliance_id.clone(),
                interface: interface.id.clone(),
                protocol: first_hop.protocol.to_string(),
                group: first_hop.group,
                virtual_ip: first_hop.virtual_ip.clone(),
                virtual_mac: first_hop.virtual_mac.clone(),
                priority: first_hop.priority,
                preempt: first_hop.preempt,
                configured_role: first_hop.initial_role,
                role: first_hop.initial_role,
            });
        }
    }
    let indexes = states
        .iter()
        .enumerate()
        .map(|(index, state)| ((state.appliance.clone(), state.interface.clone()), index))
        .collect::<BTreeMap<_, _>>();
    apply_overrides(
        &mut states,
        &indexes,
        &scenario.first_hop_overrides,
        &scenario.id,
    )?;
    if let Some(overrides) = request_overrides {
        apply_overrides(&mut states, &indexes, overrides, &scenario.id)?;
    }
    validate_no_split_brain(&states, &scenario.id)?;
    Ok(states)
}

fn apply_overrides(
    states: &mut [ScenarioFirstHopState],
    indexes: &BTreeMap<(String, String), usize>,
    overrides: &[ScenarioFirstHopOverride],
    scenario_id: &str,
) -> Result<(), ConfigError> {
    for state in overrides {
        let key = (state.appliance.clone(), state.interface.clone());
        let index = indexes.get(&key).ok_or_else(|| {
            ConfigError::new(format!(
                "scenario {scenario_id} first-hop override {}:{} is not in its selected topology",
                state.appliance, state.interface
            ))
        })?;
        states[*index].role = state.role;
    }
    Ok(())
}

fn validate_no_split_brain(
    states: &[ScenarioFirstHopState],
    scenario_id: &str,
) -> Result<(), ConfigError> {
    let mut active = BTreeSet::new();
    for state in states
        .iter()
        .filter(|state| state.role == FirstHopRole::Active)
    {
        let identity = (
            state.protocol.as_str(),
            state.group,
            state.virtual_ip.as_str(),
            state.virtual_mac.as_str(),
        );
        if !active.insert(identity) {
            return Err(ConfigError::new(format!(
                "scenario {scenario_id} activates more than one member of {} group {}",
                state.protocol, state.group
            )));
        }
    }
    Ok(())
}
