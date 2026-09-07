use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{
    BehaviorConfig, ConfigError, ConfigRepository, ConnectionRepository, ScenarioExpectedOutcome,
};

use super::connection::ScenarioConnectionState;
use crate::scenario::ScenarioConfig;

#[derive(Clone, Debug)]
pub struct LocalControlTopology {
    pub controller: String,
    pub remote_io: String,
    pub connections: Vec<String>,
}

pub(crate) fn validate_local_autonomy_state(
    scenario: &ScenarioConfig,
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    connection_states: &[ScenarioConnectionState],
) -> Result<(), ConfigError> {
    let Some(_) = &scenario.local_autonomy else {
        return Ok(());
    };
    if scenario.expectation.outcome != ScenarioExpectedOutcome::Dropped {
        return Err(ConfigError::new(format!(
            "scenario {} local autonomy requires a northbound drop expectation",
            scenario.id
        )));
    }
    if scenario.connection_overrides.len() < 2
        || scenario
            .connection_overrides
            .iter()
            .any(|state| state.operational)
    {
        return Err(ConfigError::new(format!(
            "scenario {} local autonomy requires at least two failed inter-site connections",
            scenario.id
        )));
    }
    for state in &scenario.connection_overrides {
        let connection = connections
            .get(&state.connection)
            .expect("selected scenario connection override was validated");
        if !connection.config.tags.iter().any(|tag| tag == "inter-site") {
            return Err(ConfigError::new(format!(
                "scenario {} local-autonomy outage connection {} is not tagged inter-site",
                scenario.id, state.connection
            )));
        }
    }

    let contract = local_control_contract(scenario, appliances)?;
    for participant in [&contract.controller] {
        if !scenario
            .participants
            .iter()
            .any(|candidate| candidate == participant)
        {
            return Err(ConfigError::new(format!(
                "scenario {} local control reference {participant} is not a participant",
                scenario.id
            )));
        }
    }
    local_control_topology(scenario, appliances, connections, connection_states).map(|_| ())
}

pub fn local_control_topology(
    scenario: &ScenarioConfig,
    appliance_repository: &ConfigRepository,
    connections: &ConnectionRepository,
    connection_states: &[ScenarioConnectionState],
) -> Result<LocalControlTopology, ConfigError> {
    let autonomy = scenario
        .local_autonomy
        .as_ref()
        .expect("local control topology requires an autonomy contract");
    let participants = scenario
        .participants
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let operational = connection_states
        .iter()
        .map(|state| (state.id.as_str(), state.operational))
        .collect::<BTreeMap<_, _>>();
    let mut adjacency: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for loaded in connections.connections() {
        if !operational
            .get(loaded.config.id.as_str())
            .copied()
            .unwrap_or(false)
        {
            continue;
        }
        let a = &loaded.config.endpoints.a.appliance;
        let b = &loaded.config.endpoints.b.appliance;
        if !participants.contains(a) || !participants.contains(b) {
            continue;
        }
        adjacency
            .entry(a.clone())
            .or_default()
            .push((b.clone(), loaded.config.id.clone()));
        adjacency
            .entry(b.clone())
            .or_default()
            .push((a.clone(), loaded.config.id.clone()));
    }

    let contract = local_control_contract(scenario, appliance_repository)?;
    let (remote_io, remote_io_path) = contract
        .remote_io
        .iter()
        .filter(|candidate| participants.contains(*candidate))
        .filter_map(|candidate| {
            find_connection_path(&scenario.id, &adjacency, candidate, &autonomy.actuator)
                .ok()
                .map(|path| (candidate.clone(), path))
        })
        .min_by_key(|(candidate, path)| (path.len(), candidate.clone()))
        .ok_or_else(|| {
            ConfigError::new(format!(
                "scenario {} has no participating remote I/O path to actuator {}",
                scenario.id, autonomy.actuator
            ))
        })?;
    let stages = [
        (autonomy.hmi.as_str(), contract.controller.as_str()),
        (contract.controller.as_str(), remote_io.as_str()),
        (autonomy.hmi.as_str(), autonomy.safety_interface.as_str()),
    ];
    let mut path_connections = remote_io_path.into_iter().collect::<BTreeSet<_>>();
    for (source, destination) in stages {
        path_connections.extend(find_connection_path(
            &scenario.id,
            &adjacency,
            source,
            destination,
        )?);
    }
    Ok(LocalControlTopology {
        controller: contract.controller,
        remote_io,
        connections: path_connections.into_iter().collect(),
    })
}

struct LocalControlContract {
    controller: String,
    remote_io: Vec<String>,
}

fn local_control_contract(
    scenario: &ScenarioConfig,
    appliances: &ConfigRepository,
) -> Result<LocalControlContract, ConfigError> {
    let autonomy = scenario
        .local_autonomy
        .as_ref()
        .expect("local control contract requires an autonomy contract");
    let hmi = appliances.get(&autonomy.hmi).ok_or_else(|| {
        ConfigError::new(format!(
            "scenario {} references unknown HMI {}",
            scenario.id, autonomy.hmi
        ))
    })?;
    let BehaviorConfig::OperatorInterface {
        controller,
        command_tags,
        safety_components,
        ..
    } = &hmi.config.behavior
    else {
        return Err(ConfigError::new(format!(
            "scenario {} local control endpoint {} is not an operator interface",
            scenario.id, autonomy.hmi
        )));
    };
    if !command_tags.contains(&autonomy.command_tag) {
        return Err(ConfigError::new(format!(
            "scenario {} HMI {} is not authorized for command tag {}",
            scenario.id, autonomy.hmi, autonomy.command_tag
        )));
    }
    if !safety_components.is_empty() && !safety_components.contains(&autonomy.safety_interface) {
        return Err(ConfigError::new(format!(
            "scenario {} HMI {} does not own safety interface {}",
            scenario.id, autonomy.hmi, autonomy.safety_interface
        )));
    }
    let safety = appliances.get(&autonomy.safety_interface).ok_or_else(|| {
        ConfigError::new(format!(
            "scenario {} references unknown safety interface {}",
            scenario.id, autonomy.safety_interface
        ))
    })?;
    let BehaviorConfig::Safety {
        permissives,
        latched_trip,
        initially_permissive,
    } = &safety.config.behavior
    else {
        return Err(ConfigError::new(format!(
            "scenario {} local safety reference {} is not a safety interface",
            scenario.id, autonomy.safety_interface
        )));
    };
    if !latched_trip
        || permissives
            .iter()
            .any(|tag| !initially_permissive.contains(tag))
    {
        return Err(ConfigError::new(format!(
            "scenario {} local-autonomy safety circuit must start latched with healthy permissives",
            scenario.id
        )));
    }
    let actuator = appliances.get(&autonomy.actuator).ok_or_else(|| {
        ConfigError::new(format!(
            "scenario {} references unknown actuator {}",
            scenario.id, autonomy.actuator
        ))
    })?;
    let BehaviorConfig::FieldActuator {
        command_tag,
        states,
        ..
    } = &actuator.config.behavior
    else {
        return Err(ConfigError::new(format!(
            "scenario {} local actuator reference {} is not an actuator",
            scenario.id, autonomy.actuator
        )));
    };
    if command_tag != &autonomy.command_tag {
        return Err(ConfigError::new(format!(
            "scenario {} actuator {} owns command tag {}, not {}",
            scenario.id, autonomy.actuator, command_tag, autonomy.command_tag
        )));
    }
    if !states.contains(&autonomy.command_value)
        || !states.contains(&autonomy.expected_actuator_state)
    {
        return Err(ConfigError::new(format!(
            "scenario {} local-autonomy command and expected state must belong to actuator {}",
            scenario.id, autonomy.actuator
        )));
    }
    let remote_io = appliances
        .appliances()
        .filter(|candidate| {
            candidate.config.environment == hmi.config.environment
                && candidate.config.zone == hmi.config.zone
                && matches!(
                    &candidate.config.behavior,
                    BehaviorConfig::RemoteIo {
                        controller: assigned,
                        channels,
                        ..
                    } if assigned == controller && channels.contains(&autonomy.actuator)
                )
        })
        .map(|candidate| candidate.config.id.clone())
        .collect::<Vec<_>>();
    if remote_io.is_empty() {
        return Err(ConfigError::new(format!(
            "scenario {} has no remote I/O assigned to controller {} for actuator {}",
            scenario.id, controller, autonomy.actuator
        )));
    }
    Ok(LocalControlContract {
        controller: controller.clone(),
        remote_io,
    })
}

fn find_connection_path(
    scenario_id: &str,
    adjacency: &BTreeMap<String, Vec<(String, String)>>,
    source: &str,
    destination: &str,
) -> Result<Vec<String>, ConfigError> {
    let mut pending = VecDeque::from([source.to_owned()]);
    let mut visited = BTreeSet::from([source.to_owned()]);
    let mut previous: BTreeMap<String, (String, String)> = BTreeMap::new();
    while let Some(current) = pending.pop_front() {
        if current == destination {
            break;
        }
        for (neighbor, connection) in adjacency.get(&current).into_iter().flatten() {
            if visited.insert(neighbor.clone()) {
                previous.insert(neighbor.clone(), (current.clone(), connection.clone()));
                pending.push_back(neighbor.clone());
            }
        }
    }
    if !visited.contains(destination) {
        return Err(ConfigError::new(format!(
            "scenario {scenario_id} has no operational local-control path from {source} to {destination}"
        )));
    }
    let mut cursor = destination.to_owned();
    let mut path = Vec::new();
    while cursor != source {
        let (parent, connection) = previous
            .get(&cursor)
            .expect("reachable local-control node has a predecessor");
        path.push(connection.clone());
        cursor.clone_from(parent);
    }
    path.reverse();
    Ok(path)
}
