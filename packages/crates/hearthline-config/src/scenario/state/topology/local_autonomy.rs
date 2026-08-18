use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{
    ConfigError, ConfigRepository, ConnectionRepository, HmiSession, ScenarioExpectedOutcome,
};

use super::connection::ScenarioConnectionState;
use crate::scenario::ScenarioConfig;

#[derive(Clone, Debug)]
pub(crate) struct LocalControlTopology {
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
    let Some(autonomy) = &scenario.local_autonomy else {
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

    let session = HmiSession::from_repository(appliances, &autonomy.hmi)?;
    let snapshot = session.snapshot();
    for participant in [&snapshot.controller] {
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
    let safety = snapshot
        .safety
        .iter()
        .find(|state| state.component_id == autonomy.safety_interface)
        .ok_or_else(|| {
            ConfigError::new(format!(
                "scenario {} HMI {} does not own safety interface {}",
                scenario.id, autonomy.hmi, autonomy.safety_interface
            ))
        })?;
    if !safety.trip_latched || safety.permissives.iter().any(|state| !state.satisfied) {
        return Err(ConfigError::new(format!(
            "scenario {} local-autonomy safety circuit must start latched with healthy permissives",
            scenario.id
        )));
    }
    let actuator = snapshot
        .actuators
        .iter()
        .find(|state| state.component_id == autonomy.actuator)
        .ok_or_else(|| {
            ConfigError::new(format!(
                "scenario {} HMI {} does not control actuator {}",
                scenario.id, autonomy.hmi, autonomy.actuator
            ))
        })?;
    if actuator.command_tag != autonomy.command_tag {
        return Err(ConfigError::new(format!(
            "scenario {} actuator {} owns command tag {}, not {}",
            scenario.id, autonomy.actuator, actuator.command_tag, autonomy.command_tag
        )));
    }
    if !actuator.states.contains(&autonomy.command_value)
        || !actuator.states.contains(&autonomy.expected_actuator_state)
    {
        return Err(ConfigError::new(format!(
            "scenario {} local-autonomy command and expected state must belong to actuator {}",
            scenario.id, autonomy.actuator
        )));
    }
    local_control_topology(scenario, appliances, connections, connection_states).map(|_| ())
}

pub(crate) fn local_control_topology(
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

    let snapshot = HmiSession::from_repository(appliance_repository, &autonomy.hmi)?.snapshot();
    let (remote_io, remote_io_path) = snapshot
        .remote_io_stations
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
        (autonomy.hmi.as_str(), snapshot.controller.as_str()),
        (snapshot.controller.as_str(), remote_io.as_str()),
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
        controller: snapshot.controller,
        remote_io,
        connections: path_connections.into_iter().collect(),
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
