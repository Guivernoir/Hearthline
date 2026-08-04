use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

use hearthline_model::{ComponentKind, Ipv4InterfaceAddress};

use crate::{ConfigError, ConfigRepository, ConnectionRepository};

use super::{
    ScenarioConfig, ScenarioConnectionState, ScenarioFirewallHaState, ScenarioFirstHopState,
    ScenarioLinkAggregationState, ScenarioSpanningTreeState, ScenarioSummary,
};

#[derive(Clone, Debug)]
pub struct LoadedScenario {
    pub config: ScenarioConfig,
    pub connection_states: Vec<ScenarioConnectionState>,
    pub first_hop_states: Vec<ScenarioFirstHopState>,
    pub firewall_ha_states: Vec<ScenarioFirewallHaState>,
    pub link_aggregation_states: Vec<ScenarioLinkAggregationState>,
    pub spanning_tree_states: Vec<ScenarioSpanningTreeState>,
    pub source_yaml: String,
    pub source_file: PathBuf,
}

#[derive(Clone, Debug)]
pub struct ScenarioRepository {
    scenarios: BTreeMap<String, LoadedScenario>,
}

impl ScenarioRepository {
    pub fn load(
        root: impl AsRef<Path>,
        appliances: &ConfigRepository,
        connections: &ConnectionRepository,
    ) -> Result<Self, ConfigError> {
        let root = root.as_ref();
        let mut paths = Vec::new();
        collect_yaml_paths(root, &mut paths)?;
        paths.sort();
        if paths.is_empty() {
            return Err(ConfigError::new(format!(
                "{} contains no scenario YAML files",
                root.display()
            )));
        }

        let mut scenarios = BTreeMap::new();
        for path in paths {
            let source_yaml = fs::read_to_string(&path).map_err(|error| {
                ConfigError::new(format!("cannot read {}: {error}", path.display()))
            })?;
            let config = ScenarioConfig::from_yaml(&source_yaml)
                .map_err(|error| ConfigError::new(format!("{}: {error}", path.display())))?;
            let expected_file = format!("{}.yaml", config.id);
            if path.file_name().and_then(|name| name.to_str()) != Some(expected_file.as_str()) {
                return Err(ConfigError::new(format!(
                    "{} must be named {}",
                    path.display(),
                    expected_file
                )));
            }
            validate_project_references(&config, appliances, connections)?;
            let connection_states =
                super::connection::scenario_connection_states(&config, connections, None)?;
            let first_hop_states =
                super::first_hop::scenario_first_hop_states(&config, appliances, None)?;
            let firewall_ha_states = super::firewall_ha::scenario_firewall_ha_states(
                &config,
                appliances,
                connections,
                &connection_states,
                None,
            )?;
            let link_aggregation_states =
                super::link_aggregation::scenario_link_aggregation_states(
                    &config,
                    appliances,
                    connections,
                    &connection_states,
                )?;
            let spanning_tree_states = super::spanning_tree::scenario_spanning_tree_states(
                &config,
                appliances,
                connections,
                &connection_states,
                &link_aggregation_states,
            )?;
            validate_recovery_states(
                &config,
                appliances,
                connections,
                &connection_states,
                &first_hop_states,
            )?;
            validate_continuity_state(
                &config,
                appliances,
                connections,
                &connection_states,
                &firewall_ha_states,
            )?;
            super::firewall_ha::validate_ha_isolation_state(
                &config,
                connections,
                &connection_states,
                &firewall_ha_states,
            )?;
            super::state::local_autonomy::validate_local_autonomy_state(
                &config,
                appliances,
                connections,
                &connection_states,
            )?;
            let id = config.id.clone();
            if scenarios
                .insert(
                    id.clone(),
                    LoadedScenario {
                        config,
                        connection_states,
                        first_hop_states,
                        firewall_ha_states,
                        link_aggregation_states,
                        spanning_tree_states,
                        source_yaml,
                        source_file: path,
                    },
                )
                .is_some()
            {
                return Err(ConfigError::new(format!("duplicate scenario id {id}")));
            }
        }
        Ok(Self { scenarios })
    }

    pub fn get(&self, id: &str) -> Option<&LoadedScenario> {
        self.scenarios.get(id)
    }

    pub fn scenarios(&self) -> impl Iterator<Item = &LoadedScenario> {
        self.scenarios.values()
    }

    pub fn summaries(&self) -> Vec<ScenarioSummary> {
        self.scenarios
            .values()
            .map(|scenario| {
                scenario.config.summary(
                    scenario.connection_states.clone(),
                    scenario.first_hop_states.clone(),
                    scenario.link_aggregation_states.clone(),
                    scenario.spanning_tree_states.clone(),
                    scenario.firewall_ha_states.clone(),
                )
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.scenarios.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scenarios.is_empty()
    }
}

fn validate_recovery_states(
    scenario: &ScenarioConfig,
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    baseline: &[ScenarioConnectionState],
    baseline_first_hop: &[ScenarioFirstHopState],
) -> Result<(), ConfigError> {
    let Some(recovery) = &scenario.recovery else {
        return Ok(());
    };
    let recovered = super::connection::scenario_connection_states(
        scenario,
        connections,
        Some(&recovery.connection_overrides),
    )?;
    let recovered_first_hop = super::first_hop::scenario_first_hop_states(
        scenario,
        appliances,
        Some(&recovery.first_hop_overrides),
    )?;
    let baseline_firewall_ha = super::firewall_ha::scenario_firewall_ha_states(
        scenario,
        appliances,
        connections,
        baseline,
        None,
    )?;
    let recovered_firewall_ha = super::firewall_ha::scenario_firewall_ha_states(
        scenario,
        appliances,
        connections,
        &recovered,
        Some(&recovery.firewall_ha_overrides),
    )?;
    let recovered_link_aggregation = super::link_aggregation::scenario_link_aggregation_states(
        scenario,
        appliances,
        connections,
        &recovered,
    )?;
    super::spanning_tree::scenario_spanning_tree_states(
        scenario,
        appliances,
        connections,
        &recovered,
        &recovered_link_aggregation,
    )?;
    let connections_unchanged = baseline
        .iter()
        .zip(&recovered)
        .all(|(before, after)| before.operational == after.operational);
    let first_hop_unchanged = baseline_first_hop
        .iter()
        .zip(&recovered_first_hop)
        .all(|(before, after)| before.role == after.role);
    let firewall_ha_unchanged = baseline_firewall_ha
        .iter()
        .zip(&recovered_firewall_ha)
        .all(|(before, after)| before.role == after.role);
    if connections_unchanged && first_hop_unchanged && firewall_ha_unchanged {
        return Err(ConfigError::new(format!(
            "scenario {} recovery does not change any selected topology state",
            scenario.id
        )));
    }
    Ok(())
}

fn validate_project_references(
    scenario: &ScenarioConfig,
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
) -> Result<(), ConfigError> {
    let participants = scenario.participants.iter().collect::<BTreeSet<_>>();
    for id in &scenario.participants {
        if appliances.get(id).is_none() {
            return Err(ConfigError::new(format!(
                "scenario {} references unknown appliance {id}",
                scenario.id
            )));
        }
    }
    if let Some(security) = &scenario.security {
        let defender = appliances.get(&security.defender).ok_or_else(|| {
            ConfigError::new(format!(
                "scenario {} references unknown security defender {}",
                scenario.id, security.defender
            ))
        })?;
        if defender.config.kind != ComponentKind::OperationsConsole {
            return Err(ConfigError::new(format!(
                "scenario {} security defender {} is not an operations console",
                scenario.id, security.defender
            )));
        }
    }
    validate_source_address(
        scenario,
        appliances,
        "source",
        &scenario.source,
        &scenario.packet.source_ip,
    )?;
    if let Some(continuity) = &scenario.continuity {
        validate_source_address(
            scenario,
            appliances,
            "continuation source",
            &continuity.source,
            &continuity.packet.source_ip,
        )?;
    }
    if let Some(isolation) = &scenario.ha_isolation {
        validate_source_address(
            scenario,
            appliances,
            "HA isolation source",
            &isolation.source,
            &isolation.packet.source_ip,
        )?;
    }

    let mut adjacency: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for connection in connections.connections().filter(|connection| {
        connection.config.properties.operational
            && participants.contains(&connection.config.endpoints.a.appliance)
            && participants.contains(&connection.config.endpoints.b.appliance)
    }) {
        let a = connection.config.endpoints.a.appliance.as_str();
        let b = connection.config.endpoints.b.appliance.as_str();
        adjacency.entry(a).or_default().push(b);
        adjacency.entry(b).or_default().push(a);
    }
    let mut reached = BTreeSet::new();
    let mut pending = VecDeque::from([scenario.source.as_str()]);
    if let Some(autonomy) = &scenario.local_autonomy {
        pending.push_back(&autonomy.hmi);
    }
    while let Some(current) = pending.pop_front() {
        if !reached.insert(current) {
            continue;
        }
        for neighbor in adjacency.get(current).into_iter().flatten() {
            pending.push_back(neighbor);
        }
    }
    if reached.len() != participants.len() {
        let missing = participants
            .iter()
            .filter(|participant| !reached.contains(participant.as_str()))
            .map(|participant| participant.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(ConfigError::new(format!(
            "scenario {} has participants disconnected from its execution roots: {missing}",
            scenario.id
        )));
    }
    Ok(())
}

fn validate_source_address(
    scenario: &ScenarioConfig,
    appliances: &ConfigRepository,
    label: &str,
    appliance_id: &str,
    address: &str,
) -> Result<(), ConfigError> {
    let appliance = appliances
        .get(appliance_id)
        .expect("scenario participant existence was checked");
    let address: Ipv4Addr = address
        .parse()
        .expect("scenario packet validation parsed the source address");
    if appliance.config.interfaces.iter().any(|interface| {
        interface.addresses.iter().any(|candidate| {
            candidate
                .parse::<Ipv4InterfaceAddress>()
                .is_ok_and(|candidate| candidate.address() == address)
        })
    }) {
        return Ok(());
    }
    Err(ConfigError::new(format!(
        "scenario {} {label} address {address} is not assigned to {appliance_id}",
        scenario.id
    )))
}

fn validate_continuity_state(
    scenario: &ScenarioConfig,
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    baseline_connections: &[ScenarioConnectionState],
    firewall_ha_states: &[ScenarioFirewallHaState],
) -> Result<(), ConfigError> {
    let Some(continuity) = &scenario.continuity else {
        return Ok(());
    };
    if !scenario.first_hop_overrides.is_empty() || !scenario.firewall_ha_overrides.is_empty() {
        return Err(ConfigError::new(format!(
            "scenario {} continuity controls HA and first-hop roles internally",
            scenario.id
        )));
    }
    let failed = firewall_ha_states
        .iter()
        .find(|state| state.appliance == continuity.failed_appliance)
        .ok_or_else(|| {
            ConfigError::new(format!(
                "scenario {} failed appliance {} is not a selected HA firewall",
                scenario.id, continuity.failed_appliance
            ))
        })?;
    let peer = firewall_ha_states
        .iter()
        .find(|state| state.appliance == failed.peer)
        .ok_or_else(|| {
            ConfigError::new(format!(
                "scenario {} does not select HA peer {}",
                scenario.id, failed.peer
            ))
        })?;
    if !failed.role.is_active()
        || peer.role.is_active()
        || !failed.sync_operational
        || !peer.sync_operational
        || !failed.session_sync
        || !peer.session_sync
    {
        return Err(ConfigError::new(format!(
            "scenario {} continuity requires an active failed member, standby peer, operational sync path, and session synchronization",
            scenario.id
        )));
    }
    let ha = appliances
        .get(&failed.appliance)
        .and_then(|appliance| appliance.config.firewall_ha.as_ref())
        .expect("selected HA firewall has HA configuration");
    let minimum_fault_at_us = ha.heartbeat_interval_ms.saturating_mul(1_000);
    if continuity
        .faults
        .iter()
        .any(|fault| fault.at_us() < minimum_fault_at_us)
    {
        return Err(ConfigError::new(format!(
            "scenario {} continuity faults must allow one HA heartbeat interval before injection",
            scenario.id
        )));
    }
    let earliest_continuation = continuity
        .failure_at_us
        .saturating_add(ha.failure_hold_ms.saturating_add(ha.heartbeat_interval_ms) * 1_000);
    if continuity.continuation_at_us < earliest_continuation {
        return Err(ConfigError::new(format!(
            "scenario {} continuation_at_us must allow the configured heartbeat failure hold and one evaluation interval",
            scenario.id
        )));
    }
    let converged_connections = super::connection::scenario_connection_states(
        scenario,
        connections,
        Some(&continuity.connection_overrides),
    )?;
    if baseline_connections
        .iter()
        .zip(&converged_connections)
        .all(|(before, after)| before.operational == after.operational)
    {
        return Err(ConfigError::new(format!(
            "scenario {} continuity connection state does not change at failure",
            scenario.id
        )));
    }
    let expects_sync_loss = continuity
        .faults
        .iter()
        .any(|fault| fault.is_sync_link_loss());
    for state in [failed, peer] {
        let sync_operational = converged_connections
            .iter()
            .find(|connection| connection.id == state.sync_connection)
            .is_some_and(|connection| connection.operational);
        if sync_operational == expects_sync_loss {
            return Err(ConfigError::new(format!(
                "scenario {} continuity must leave HA sync connection {} {} after convergence",
                scenario.id,
                state.sync_connection,
                if expects_sync_loss {
                    "down"
                } else {
                    "operational"
                }
            )));
        }
    }
    Ok(())
}

fn collect_yaml_paths(root: &Path, paths: &mut Vec<PathBuf>) -> Result<(), ConfigError> {
    let entries = fs::read_dir(root)
        .map_err(|error| ConfigError::new(format!("cannot read {}: {error}", root.display())))?;
    for entry in entries {
        let path = entry
            .map_err(|error| ConfigError::new(error.to_string()))?
            .path();
        if path.is_dir() {
            collect_yaml_paths(&path, paths)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("yaml") {
            paths.push(path);
        }
    }
    Ok(())
}
