use std::collections::{BTreeMap, BTreeSet};

use crate::appliance::{
    ConfigError, ConfigRepository, LinkAggregationMode, LinkAggregationProtocol,
};

use super::{ConnectionDirection, LoadedConnection, TransportKind, endpoint_port};

#[derive(Clone, Debug)]
struct AggregateMember {
    logical_id: String,
    logical_system: String,
    protocol: LinkAggregationProtocol,
    mode: LinkAggregationMode,
}

#[derive(Default)]
struct AggregateSide {
    members: BTreeSet<(String, String)>,
    protocols: BTreeSet<LinkAggregationProtocol>,
    minimums: BTreeSet<u8>,
}

pub(super) fn validate_redundancy_connections(
    appliances: &ConfigRepository,
    connections: &BTreeMap<String, LoadedConnection>,
) -> Result<(), ConfigError> {
    validate_firewall_ha_links(appliances, connections)?;
    validate_peer_links(appliances, connections)?;

    let mut members = BTreeMap::new();
    let mut aggregates: BTreeMap<String, BTreeMap<String, AggregateSide>> = BTreeMap::new();
    for appliance in appliances.appliances() {
        let Some(link_aggregation) = &appliance.config.link_aggregation else {
            continue;
        };
        let logical_system = appliance
            .config
            .multi_chassis
            .as_ref()
            .map_or(appliance.config.id.as_str(), |domain| {
                domain.domain.as_str()
            });
        for group in &link_aggregation.groups {
            let side = aggregates
                .entry(group.logical_id.clone())
                .or_default()
                .entry(logical_system.to_owned())
                .or_default();
            side.protocols.insert(group.protocol);
            side.minimums.insert(group.minimum_active_members);
            for interface in &group.members {
                let key = (appliance.config.id.clone(), interface.clone());
                side.members.insert(key.clone());
                members.insert(
                    key,
                    AggregateMember {
                        logical_id: group.logical_id.clone(),
                        logical_system: logical_system.to_owned(),
                        protocol: group.protocol,
                        mode: group.mode,
                    },
                );
            }
        }
    }

    let mut connected_members = BTreeSet::new();
    for loaded in connections.values() {
        let config = &loaded.config;
        let a_key = (
            config.endpoints.a.appliance.clone(),
            config.endpoints.a.interface.clone(),
        );
        let b_key = (
            config.endpoints.b.appliance.clone(),
            config.endpoints.b.interface.clone(),
        );
        let a = members.get(&a_key);
        let b = members.get(&b_key);
        if a.is_none() && b.is_none() {
            continue;
        }
        let (Some(a), Some(b)) = (a, b) else {
            return Err(ConfigError::new(format!(
                "connection {} joins an aggregate member to a non-member interface",
                config.id
            )));
        };
        if a.logical_id != b.logical_id || a.logical_system == b.logical_system {
            return Err(ConfigError::new(format!(
                "connection {} does not join opposite systems of one logical aggregate",
                config.id
            )));
        }
        if a.protocol != b.protocol {
            return Err(ConfigError::new(format!(
                "connection {} joins different aggregation protocols",
                config.id
            )));
        }
        if a.mode == LinkAggregationMode::Passive && b.mode == LinkAggregationMode::Passive {
            return Err(ConfigError::new(format!(
                "connection {} cannot negotiate LACP with both endpoints passive",
                config.id
            )));
        }
        if config.transport != TransportKind::Ethernet
            || config.properties.direction != ConnectionDirection::Bidirectional
        {
            return Err(ConfigError::new(format!(
                "aggregate member connection {} must be bidirectional Ethernet",
                config.id
            )));
        }
        validate_member_compatibility(appliances, loaded)?;
        connected_members.insert(a_key);
        connected_members.insert(b_key);
    }

    for (key, member) in &members {
        if !connected_members.contains(key) {
            return Err(ConfigError::new(format!(
                "aggregate {} member {}:{} has no compatible physical connection",
                member.logical_id, key.0, key.1
            )));
        }
    }
    for (logical_id, sides) in aggregates {
        if sides.len() != 2 {
            return Err(ConfigError::new(format!(
                "logical aggregate {logical_id} requires exactly two logical systems"
            )));
        }
        for (system, side) in sides {
            if side.protocols.len() != 1 || side.minimums.len() != 1 {
                return Err(ConfigError::new(format!(
                    "logical aggregate {logical_id} system {system} has inconsistent member configuration"
                )));
            }
            let minimum = usize::from(*side.minimums.first().expect("validated minimum"));
            if side.members.len() < minimum {
                return Err(ConfigError::new(format!(
                    "logical aggregate {logical_id} system {system} has fewer members than its minimum"
                )));
            }
        }
    }
    Ok(())
}

fn validate_firewall_ha_links(
    appliances: &ConfigRepository,
    connections: &BTreeMap<String, LoadedConnection>,
) -> Result<(), ConfigError> {
    for appliance in appliances.appliances() {
        let Some(ha) = &appliance.config.firewall_ha else {
            continue;
        };
        let peer = appliances
            .get(&ha.peer)
            .expect("appliance HA validation guarantees peer existence");
        let peer_ha = peer
            .config
            .firewall_ha
            .as_ref()
            .expect("appliance HA validation guarantees reciprocal peer");
        let matches = connections
            .values()
            .filter(|loaded| {
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
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(ConfigError::new(format!(
                "firewall HA domain {} requires one direct sync connection between {} and {}",
                ha.domain, appliance.config.id, peer.config.id
            )));
        }
        let connection = matches[0];
        if connection.config.transport != TransportKind::Ethernet
            || connection.config.properties.direction != ConnectionDirection::Bidirectional
            || !connection.config.properties.operational
        {
            return Err(ConfigError::new(format!(
                "firewall HA sync connection {} must be operational bidirectional Ethernet",
                connection.config.id
            )));
        }
        let a = endpoint_port(appliances, &connection.config.endpoints.a)?;
        let b = endpoint_port(appliances, &connection.config.endpoints.b)?;
        if a.settings != b.settings {
            return Err(ConfigError::new(format!(
                "firewall HA sync connection {} requires matching port settings",
                connection.config.id
            )));
        }
    }
    Ok(())
}

fn validate_member_compatibility(
    appliances: &ConfigRepository,
    connection: &LoadedConnection,
) -> Result<(), ConfigError> {
    let a = endpoint_port(appliances, &connection.config.endpoints.a)?;
    let b = endpoint_port(appliances, &connection.config.endpoints.b)?;
    if a.mode != b.mode
        || a.settings.speed_mbps != b.settings.speed_mbps
        || a.settings.duplex != b.settings.duplex
        || a.settings.mtu != b.settings.mtu
        || a.vlans != b.vlans
    {
        return Err(ConfigError::new(format!(
            "aggregate member connection {} requires matching mode, speed, duplex, MTU, and VLANs",
            connection.config.id
        )));
    }
    Ok(())
}

fn validate_peer_links(
    appliances: &ConfigRepository,
    connections: &BTreeMap<String, LoadedConnection>,
) -> Result<(), ConfigError> {
    for appliance in appliances.appliances() {
        let Some(domain) = &appliance.config.multi_chassis else {
            continue;
        };
        let peer = appliances
            .get(&domain.peer)
            .expect("appliance redundancy validation guarantees peer existence");
        let peer_domain = peer
            .config
            .multi_chassis
            .as_ref()
            .expect("appliance redundancy validation guarantees reciprocal peer");
        let matches = connections
            .values()
            .filter(|loaded| {
                let config = &loaded.config;
                (config.endpoints.a.appliance == appliance.config.id
                    && config.endpoints.a.interface == domain.peer_link
                    && config.endpoints.b.appliance == peer.config.id
                    && config.endpoints.b.interface == peer_domain.peer_link)
                    || (config.endpoints.b.appliance == appliance.config.id
                        && config.endpoints.b.interface == domain.peer_link
                        && config.endpoints.a.appliance == peer.config.id
                        && config.endpoints.a.interface == peer_domain.peer_link)
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(ConfigError::new(format!(
                "multi-chassis domain {} requires one direct peer-link connection between {} and {}",
                domain.domain, appliance.config.id, peer.config.id
            )));
        }
        if matches[0].config.properties.direction != ConnectionDirection::Bidirectional {
            return Err(ConfigError::new(format!(
                "multi-chassis peer link {} must be bidirectional",
                matches[0].config.id
            )));
        }
    }
    Ok(())
}
