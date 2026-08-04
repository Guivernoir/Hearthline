use std::collections::{BTreeMap, BTreeSet};

use hearthline_model::ComponentId;
use serde::{Deserialize, Serialize};

use crate::{ConfigError, ConfigRepository, ConnectionRepository, FirewallHaRole};

use super::ScenarioConnectionState;
use crate::scenario::ScenarioConfig;

const MAX_FIREWALL_HA_OVERRIDES: usize = 16;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioFirewallHaOverride {
    pub appliance: String,
    pub role: FirewallHaRole,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScenarioFirewallHaState {
    pub appliance: String,
    pub peer: String,
    pub domain: String,
    pub configured_role: FirewallHaRole,
    pub role: FirewallHaRole,
    pub sync_interface: String,
    pub sync_connection: String,
    pub sync_operational: bool,
    pub session_sync: bool,
    pub heartbeat_interval_ms: u64,
    pub failure_hold_ms: u64,
    pub monitored_interfaces: Vec<String>,
}

pub(crate) fn validate_firewall_ha_override_syntax(
    overrides: &[ScenarioFirewallHaOverride],
) -> Result<(), ConfigError> {
    if overrides.len() > MAX_FIREWALL_HA_OVERRIDES {
        return Err(ConfigError::new(format!(
            "scenario firewall HA overrides exceed the {MAX_FIREWALL_HA_OVERRIDES}-entry limit"
        )));
    }
    let mut appliances = BTreeSet::new();
    for state in overrides {
        ComponentId::new(&state.appliance).map_err(|error| ConfigError::new(error.to_string()))?;
        if !appliances.insert(&state.appliance) {
            return Err(ConfigError::new(format!(
                "scenario repeats firewall HA override {}",
                state.appliance
            )));
        }
    }
    Ok(())
}

pub(crate) fn scenario_firewall_ha_states(
    scenario: &ScenarioConfig,
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    connection_states: &[ScenarioConnectionState],
    request_overrides: Option<&[ScenarioFirewallHaOverride]>,
) -> Result<Vec<ScenarioFirewallHaState>, ConfigError> {
    validate_firewall_ha_override_syntax(&scenario.firewall_ha_overrides)?;
    if let Some(overrides) = request_overrides {
        validate_firewall_ha_override_syntax(overrides)?;
    }
    let selected = scenario.participants.iter().collect::<BTreeSet<_>>();
    let connection_operational = connection_states
        .iter()
        .map(|state| (state.id.as_str(), state.operational))
        .collect::<BTreeMap<_, _>>();
    let mut states = Vec::new();
    for appliance_id in &scenario.participants {
        let appliance = appliances
            .get(appliance_id)
            .expect("scenario participant validation guarantees appliance existence");
        let Some(ha) = &appliance.config.firewall_ha else {
            continue;
        };
        let peer = appliances
            .get(&ha.peer)
            .expect("firewall HA repository validation guarantees peer existence");
        let peer_ha = peer
            .config
            .firewall_ha
            .as_ref()
            .expect("firewall HA repository validation guarantees reciprocal peer");
        let sync_connection = connections
            .connections()
            .find(|loaded| {
                let endpoints = &loaded.config.endpoints;
                (endpoints.a.appliance == appliance.config.id
                    && endpoints.a.interface == ha.sync_interface
                    && endpoints.b.appliance == peer.config.id
                    && endpoints.b.interface == peer_ha.sync_interface)
                    || (endpoints.b.appliance == appliance.config.id
                        && endpoints.b.interface == ha.sync_interface
                        && endpoints.a.appliance == peer.config.id
                        && endpoints.a.interface == peer_ha.sync_interface)
            })
            .expect("firewall HA connection validation guarantees direct sync link");
        states.push(ScenarioFirewallHaState {
            appliance: appliance_id.clone(),
            peer: ha.peer.clone(),
            domain: ha.domain.clone(),
            configured_role: ha.role,
            role: ha.role,
            sync_interface: ha.sync_interface.clone(),
            sync_connection: sync_connection.config.id.clone(),
            sync_operational: selected.contains(&ha.peer)
                && connection_operational
                    .get(sync_connection.config.id.as_str())
                    .copied()
                    .unwrap_or(false),
            session_sync: ha.session_sync,
            heartbeat_interval_ms: ha.heartbeat_interval_ms,
            failure_hold_ms: ha.failure_hold_ms,
            monitored_interfaces: ha.monitored_interfaces.clone(),
        });
    }
    let indexes = states
        .iter()
        .enumerate()
        .map(|(index, state)| (state.appliance.clone(), index))
        .collect::<BTreeMap<_, _>>();
    apply_overrides(
        &mut states,
        &indexes,
        &scenario.firewall_ha_overrides,
        &scenario.id,
    )?;
    if let Some(overrides) = request_overrides {
        apply_overrides(&mut states, &indexes, overrides, &scenario.id)?;
    }
    validate_domains(&states, &scenario.id)?;
    Ok(states)
}

fn apply_overrides(
    states: &mut [ScenarioFirewallHaState],
    indexes: &BTreeMap<String, usize>,
    overrides: &[ScenarioFirewallHaOverride],
    scenario_id: &str,
) -> Result<(), ConfigError> {
    for state in overrides {
        let index = indexes.get(&state.appliance).ok_or_else(|| {
            ConfigError::new(format!(
                "scenario {scenario_id} firewall HA override {} is not in its selected topology",
                state.appliance
            ))
        })?;
        states[*index].role = state.role;
    }
    Ok(())
}

fn validate_domains(
    states: &[ScenarioFirewallHaState],
    scenario_id: &str,
) -> Result<(), ConfigError> {
    let mut domains: BTreeMap<&str, Vec<&ScenarioFirewallHaState>> = BTreeMap::new();
    for state in states {
        domains.entry(&state.domain).or_default().push(state);
    }
    for (domain, members) in domains {
        if members.len() == 1 {
            if members[0].role != FirewallHaRole::Active {
                return Err(ConfigError::new(format!(
                    "scenario {scenario_id} selects only standby firewall {} from HA domain {domain}",
                    members[0].appliance
                )));
            }
            continue;
        }
        if members.len() != 2
            || members
                .iter()
                .filter(|state| state.role == FirewallHaRole::Active)
                .count()
                != 1
        {
            return Err(ConfigError::new(format!(
                "scenario {scenario_id} firewall HA domain {domain} requires exactly one active member"
            )));
        }
    }
    Ok(())
}

pub(crate) fn validate_ha_isolation_state(
    scenario: &ScenarioConfig,
    connections: &ConnectionRepository,
    baseline_connections: &[ScenarioConnectionState],
    baseline_states: &[ScenarioFirewallHaState],
) -> Result<(), ConfigError> {
    let Some(isolation) = &scenario.ha_isolation else {
        return Ok(());
    };
    if !scenario.first_hop_overrides.is_empty() || !scenario.firewall_ha_overrides.is_empty() {
        return Err(ConfigError::new(format!(
            "scenario {} HA isolation controls roles internally",
            scenario.id
        )));
    }
    let standby = baseline_states
        .iter()
        .find(|state| state.appliance == isolation.standby_appliance)
        .ok_or_else(|| {
            ConfigError::new(format!(
                "scenario {} isolated standby {} is not a selected HA firewall",
                scenario.id, isolation.standby_appliance
            ))
        })?;
    let active = baseline_states
        .iter()
        .find(|state| state.appliance == standby.peer)
        .ok_or_else(|| {
            ConfigError::new(format!(
                "scenario {} does not select HA peer {}",
                scenario.id, standby.peer
            ))
        })?;
    if standby.role.is_active()
        || !active.role.is_active()
        || !standby.sync_operational
        || !active.sync_operational
        || !standby.session_sync
        || !active.session_sync
    {
        return Err(ConfigError::new(format!(
            "scenario {} HA isolation requires one active member, one synchronized standby, and an operational baseline sync path",
            scenario.id
        )));
    }
    let minimum_isolation_at = standby.heartbeat_interval_ms.saturating_mul(1_000);
    if isolation.isolation_at_us < minimum_isolation_at {
        return Err(ConfigError::new(format!(
            "scenario {} HA isolation must allow one heartbeat interval before link loss",
            scenario.id
        )));
    }
    let earliest_continuation = isolation.isolation_at_us.saturating_add(
        standby
            .failure_hold_ms
            .saturating_add(standby.heartbeat_interval_ms)
            .saturating_mul(1_000),
    );
    if isolation.continuation_at_us < earliest_continuation {
        return Err(ConfigError::new(format!(
            "scenario {} HA isolation continuation must follow the hold and evaluation interval",
            scenario.id
        )));
    }
    let isolated_connections = super::connection::scenario_connection_states(
        scenario,
        connections,
        Some(&isolation.connection_overrides),
    )?;
    let changed = baseline_connections
        .iter()
        .zip(&isolated_connections)
        .filter(|(before, after)| before.operational != after.operational)
        .collect::<Vec<_>>();
    if changed.len() != 1 || changed[0].1.id != standby.sync_connection || changed[0].1.operational
    {
        return Err(ConfigError::new(format!(
            "scenario {} HA isolation must withdraw only sync connection {}",
            scenario.id, standby.sync_connection
        )));
    }
    Ok(())
}
