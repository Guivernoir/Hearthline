use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    ConfigError, ConfigRepository, ConnectionRepository, LinkAggregationMode,
    LinkAggregationProtocol,
};

use super::ScenarioConnectionState;
use crate::scenario::ScenarioConfig;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScenarioLinkAggregationState {
    pub appliance: String,
    pub interface: String,
    pub connection: String,
    pub group: String,
    pub logical_id: String,
    pub protocol: String,
    pub mode: String,
    pub system_id: String,
    pub partner_system_id: String,
    pub multi_chassis_domain: Option<String>,
    pub selected: bool,
    pub collecting: bool,
    pub distributing: bool,
    pub bundle_operational: bool,
    pub active_members: usize,
    pub minimum_active_members: u8,
    pub peer_forwarding: bool,
}

#[derive(Clone, Debug)]
struct Member {
    appliance: String,
    interface: String,
    group: String,
    logical_id: String,
    logical_system: String,
    system_id: String,
    protocol: LinkAggregationProtocol,
    mode: LinkAggregationMode,
    minimum_active_members: u8,
    multi_chassis_domain: Option<String>,
    initially_usable: bool,
}

#[derive(Clone, Debug)]
struct PendingState {
    local: Member,
    partner: Member,
    connection: String,
    selected: bool,
}

pub(crate) fn scenario_link_aggregation_states(
    scenario: &ScenarioConfig,
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    connection_states: &[ScenarioConnectionState],
) -> Result<Vec<ScenarioLinkAggregationState>, ConfigError> {
    let participants = scenario.participants.iter().collect::<BTreeSet<_>>();
    let operational = connection_states
        .iter()
        .map(|state| (state.id.as_str(), state.operational))
        .collect::<BTreeMap<_, _>>();
    let members = configured_members(appliances, &participants);
    let mut pending = Vec::new();

    for loaded in connections.connections().filter(|loaded| {
        operational.contains_key(loaded.config.id.as_str())
            && participants.contains(&loaded.config.endpoints.a.appliance)
            && participants.contains(&loaded.config.endpoints.b.appliance)
    }) {
        let config = &loaded.config;
        let a_key = (
            config.endpoints.a.appliance.as_str(),
            config.endpoints.a.interface.as_str(),
        );
        let b_key = (
            config.endpoints.b.appliance.as_str(),
            config.endpoints.b.interface.as_str(),
        );
        let (Some(a), Some(b)) = (members.get(&a_key), members.get(&b_key)) else {
            continue;
        };
        if a.logical_id != b.logical_id || a.logical_system == b.logical_system {
            return Err(ConfigError::new(format!(
                "scenario {} has invalid aggregate member connection {}",
                scenario.id, config.id
            )));
        }
        let selected = operational[config.id.as_str()]
            && a.initially_usable
            && b.initially_usable
            && (a.mode == LinkAggregationMode::Active || b.mode == LinkAggregationMode::Active);
        pending.push(PendingState {
            local: a.clone(),
            partner: b.clone(),
            connection: config.id.clone(),
            selected,
        });
        pending.push(PendingState {
            local: b.clone(),
            partner: a.clone(),
            connection: config.id.clone(),
            selected,
        });
    }

    let active_counts =
        pending
            .iter()
            .filter(|state| state.selected)
            .fold(BTreeMap::new(), |mut counts, state| {
                *counts
                    .entry((
                        state.local.logical_id.clone(),
                        state.local.logical_system.clone(),
                    ))
                    .or_insert(0_usize) += 1;
                counts
            });
    let mut states = pending
        .into_iter()
        .map(|pending| {
            let active_members = active_counts
                .get(&(
                    pending.local.logical_id.clone(),
                    pending.local.logical_system.clone(),
                ))
                .copied()
                .unwrap_or(0);
            let partner_active = active_counts
                .get(&(
                    pending.partner.logical_id.clone(),
                    pending.partner.logical_system.clone(),
                ))
                .copied()
                .unwrap_or(0);
            let bundle_operational = active_members
                >= usize::from(pending.local.minimum_active_members)
                && partner_active >= usize::from(pending.partner.minimum_active_members);
            ScenarioLinkAggregationState {
                appliance: pending.local.appliance,
                interface: pending.local.interface,
                connection: pending.connection,
                group: pending.local.group,
                logical_id: pending.local.logical_id,
                protocol: pending.local.protocol.to_string(),
                mode: pending.local.mode.to_string(),
                system_id: pending.local.system_id,
                partner_system_id: pending.partner.system_id,
                multi_chassis_domain: pending.local.multi_chassis_domain,
                selected: pending.selected,
                collecting: pending.selected && bundle_operational,
                distributing: pending.selected && bundle_operational,
                bundle_operational,
                active_members,
                minimum_active_members: pending.local.minimum_active_members,
                peer_forwarding: false,
            }
        })
        .collect::<Vec<_>>();
    assign_peer_forwarding(&mut states, appliances);
    states.sort_by(|left, right| {
        (&left.logical_id, &left.appliance, &left.interface).cmp(&(
            &right.logical_id,
            &right.appliance,
            &right.interface,
        ))
    });
    Ok(states)
}

fn configured_members<'a>(
    appliances: &'a ConfigRepository,
    participants: &BTreeSet<&String>,
) -> BTreeMap<(&'a str, &'a str), Member> {
    let mut members = BTreeMap::new();
    for appliance in appliances
        .appliances()
        .filter(|appliance| participants.contains(&appliance.config.id))
    {
        let Some(aggregation) = &appliance.config.link_aggregation else {
            continue;
        };
        let domain = appliance.config.multi_chassis.as_ref();
        let logical_system = domain.map_or(appliance.config.id.as_str(), |domain| {
            domain.domain.as_str()
        });
        for group in &aggregation.groups {
            for member in &group.members {
                let interface = appliance
                    .config
                    .interfaces
                    .iter()
                    .find(|interface| interface.id == *member)
                    .expect("appliance validation guarantees aggregate member interface");
                members.insert(
                    (appliance.config.id.as_str(), member.as_str()),
                    Member {
                        appliance: appliance.config.id.clone(),
                        interface: member.clone(),
                        group: group.id.clone(),
                        logical_id: group.logical_id.clone(),
                        logical_system: logical_system.to_owned(),
                        system_id: aggregation.system_mac.clone(),
                        protocol: group.protocol,
                        mode: group.mode,
                        minimum_active_members: group.minimum_active_members,
                        multi_chassis_domain: domain.map(|domain| domain.domain.clone()),
                        initially_usable: interface.state.initially_usable(),
                    },
                );
            }
        }
    }
    members
}

fn assign_peer_forwarding(
    states: &mut [ScenarioLinkAggregationState],
    appliances: &ConfigRepository,
) {
    let distributing = states
        .iter()
        .filter(|state| state.distributing)
        .map(|state| {
            (
                state.logical_id.clone(),
                state.appliance.clone(),
                state.multi_chassis_domain.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    for state in states {
        let Some(domain) = &state.multi_chassis_domain else {
            continue;
        };
        let peer = appliances
            .get(&state.appliance)
            .and_then(|appliance| appliance.config.multi_chassis.as_ref())
            .map(|config| config.peer.as_str())
            .expect("multi-chassis state has a configured peer");
        state.peer_forwarding = distributing.contains(&(
            state.logical_id.clone(),
            peer.to_owned(),
            Some(domain.clone()),
        ));
    }
}
