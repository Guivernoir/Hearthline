use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{
    ConfigError, ConfigRepository, ConnectionDirection, ConnectionRepository, InterfaceMode,
    SpanningTreeProtocol,
};
use hearthline_model::{ComponentKind, MacAddress};

use super::{ScenarioConnectionState, ScenarioLinkAggregationState};
use crate::scenario::ScenarioConfig;

mod model;

pub use model::{ScenarioSpanningTreeState, SpanningTreePortRole, SpanningTreePortState};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct BridgeKey {
    priority: u16,
    mac: MacAddress,
}

#[derive(Clone, Debug)]
struct Bridge {
    key: BridgeKey,
    protocol: SpanningTreeProtocol,
}

#[derive(Clone, Debug)]
struct EdgeEndpoint {
    appliance: String,
    interface: String,
    speed_mbps: u64,
}

#[derive(Clone, Debug)]
struct Edge {
    connection: String,
    a: EdgeEndpoint,
    b: EdgeEndpoint,
    vlans: Vec<u16>,
    operational: bool,
}

pub(crate) fn scenario_spanning_tree_states(
    scenario: &ScenarioConfig,
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    connection_states: &[ScenarioConnectionState],
    link_aggregation_states: &[ScenarioLinkAggregationState],
) -> Result<Vec<ScenarioSpanningTreeState>, ConfigError> {
    let participants = scenario.participants.iter().collect::<BTreeSet<_>>();
    let operational = connection_states
        .iter()
        .map(|state| (state.id.as_str(), state.operational))
        .collect::<BTreeMap<_, _>>();
    let bridges = configured_bridges(scenario, appliances)?;
    let mut edges = Vec::new();

    for loaded in connections.connections().filter(|loaded| {
        participants.contains(&loaded.config.endpoints.a.appliance)
            && participants.contains(&loaded.config.endpoints.b.appliance)
    }) {
        let config = &loaded.config;
        let a_appliance = appliances
            .get(&config.endpoints.a.appliance)
            .expect("connection endpoint validation guarantees appliance existence");
        let b_appliance = appliances
            .get(&config.endpoints.b.appliance)
            .expect("connection endpoint validation guarantees appliance existence");
        if !is_switch(a_appliance.config.kind) || !is_switch(b_appliance.config.kind) {
            continue;
        }
        let a_interface = a_appliance
            .config
            .interfaces
            .iter()
            .find(|interface| interface.id == config.endpoints.a.interface)
            .expect("connection endpoint validation guarantees interface existence");
        let b_interface = b_appliance
            .config
            .interfaces
            .iter()
            .find(|interface| interface.id == config.endpoints.b.interface)
            .expect("connection endpoint validation guarantees interface existence");
        if !is_bridge_port(a_interface.mode) || !is_bridge_port(b_interface.mode) {
            continue;
        }
        let vlans = a_interface
            .vlans
            .iter()
            .copied()
            .filter(|vlan| b_interface.vlans.contains(vlan))
            .collect::<Vec<_>>();
        if vlans.is_empty() {
            continue;
        }
        let a_bridge = bridges.get(&config.endpoints.a.appliance);
        let b_bridge = bridges.get(&config.endpoints.b.appliance);
        match (a_bridge, b_bridge) {
            (None, None) => continue,
            (Some(_), None) | (None, Some(_)) => {
                return Err(ConfigError::new(format!(
                    "scenario {} connection {} mixes configured and unconfigured spanning-tree bridges",
                    scenario.id, config.id
                )));
            }
            (Some(a_bridge), Some(b_bridge)) if a_bridge.protocol != b_bridge.protocol => {
                return Err(ConfigError::new(format!(
                    "scenario {} connection {} joins different spanning-tree protocols",
                    scenario.id, config.id
                )));
            }
            (Some(_), Some(_)) => {}
        }
        if config.properties.direction != ConnectionDirection::Bidirectional {
            return Err(ConfigError::new(format!(
                "scenario {} spanning-tree connection {} must be bidirectional",
                scenario.id, config.id
            )));
        }
        edges.push(Edge {
            connection: config.id.clone(),
            a: EdgeEndpoint {
                appliance: config.endpoints.a.appliance.clone(),
                interface: config.endpoints.a.interface.clone(),
                speed_mbps: a_interface.settings.speed_mbps,
            },
            b: EdgeEndpoint {
                appliance: config.endpoints.b.appliance.clone(),
                interface: config.endpoints.b.interface.clone(),
                speed_mbps: b_interface.settings.speed_mbps,
            },
            vlans,
            operational: operational
                .get(config.id.as_str())
                .copied()
                .unwrap_or(config.properties.operational)
                && a_interface.state.initially_usable()
                && b_interface.state.initially_usable(),
        });
    }
    edges.sort_by(|left, right| left.connection.cmp(&right.connection));

    let vlans = edges
        .iter()
        .flat_map(|edge| edge.vlans.iter().copied())
        .collect::<BTreeSet<_>>();
    let mut states = Vec::new();
    for vlan in vlans {
        states.extend(converged_vlan_states(vlan, &edges, &bridges));
    }
    states.sort_by(|left, right| {
        (&left.appliance, &left.interface, left.vlan).cmp(&(
            &right.appliance,
            &right.interface,
            right.vlan,
        ))
    });
    apply_aggregate_port_roles(&mut states, link_aggregation_states);
    Ok(states)
}

fn apply_aggregate_port_roles(
    states: &mut [ScenarioSpanningTreeState],
    link_aggregation_states: &[ScenarioLinkAggregationState],
) {
    let aggregate_members = link_aggregation_states
        .iter()
        .map(|state| {
            (
                (state.appliance.as_str(), state.interface.as_str()),
                (state.logical_id.as_str(), state.distributing),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut aggregate_roles = BTreeMap::new();
    for state in states.iter() {
        let Some((logical_id, true)) = aggregate_members
            .get(&(state.appliance.as_str(), state.interface.as_str()))
            .copied()
        else {
            continue;
        };
        aggregate_roles
            .entry((state.appliance.clone(), logical_id.to_owned(), state.vlan))
            .and_modify(|role| {
                if port_role_rank(state.role) < port_role_rank(*role) {
                    *role = state.role;
                }
            })
            .or_insert(state.role);
    }
    for state in states {
        let Some((logical_id, true)) = aggregate_members
            .get(&(state.appliance.as_str(), state.interface.as_str()))
            .copied()
        else {
            continue;
        };
        state.role = aggregate_roles[&(state.appliance.clone(), logical_id.to_owned(), state.vlan)];
        state.state = SpanningTreePortState::Forwarding;
    }
}

const fn port_role_rank(role: SpanningTreePortRole) -> u8 {
    match role {
        SpanningTreePortRole::Root => 0,
        SpanningTreePortRole::Designated => 1,
        SpanningTreePortRole::Alternate => 2,
        SpanningTreePortRole::Disabled => 3,
    }
}

fn configured_bridges(
    scenario: &ScenarioConfig,
    appliances: &ConfigRepository,
) -> Result<BTreeMap<String, Bridge>, ConfigError> {
    scenario
        .participants
        .iter()
        .filter_map(|id| {
            let config = &appliances
                .get(id)
                .expect("scenario participant validation guarantees appliance existence")
                .config;
            config.spanning_tree.as_ref().map(|spanning_tree| {
                let mac = spanning_tree
                    .bridge_mac
                    .parse::<MacAddress>()
                    .map_err(|error| ConfigError::new(error.to_string()))?;
                Ok((
                    id.clone(),
                    Bridge {
                        key: BridgeKey {
                            priority: spanning_tree.bridge_priority,
                            mac,
                        },
                        protocol: spanning_tree.protocol,
                    },
                ))
            })
        })
        .collect()
}

fn converged_vlan_states(
    vlan: u16,
    edges: &[Edge],
    bridges: &BTreeMap<String, Bridge>,
) -> Vec<ScenarioSpanningTreeState> {
    let vlan_edges = edges
        .iter()
        .filter(|edge| edge.vlans.contains(&vlan))
        .collect::<Vec<_>>();
    let nodes = vlan_edges
        .iter()
        .flat_map(|edge| [&edge.a.appliance, &edge.b.appliance])
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut roots = BTreeMap::new();
    let mut distances = BTreeMap::new();
    let mut root_ports = BTreeMap::new();
    let mut visited = BTreeSet::new();

    for start in &nodes {
        if visited.contains(start) {
            continue;
        }
        let component = connected_component(start, &vlan_edges);
        visited.extend(component.iter().cloned());
        let root = component
            .iter()
            .min_by_key(|id| bridges.get(*id).expect("edge bridge").key)
            .expect("component contains its start")
            .clone();
        let component_distances = root_distances(&root, &component, &vlan_edges);
        for bridge in &component {
            roots.insert(bridge.clone(), root.clone());
            distances.insert(
                bridge.clone(),
                *component_distances
                    .get(bridge)
                    .expect("connected bridge has root distance"),
            );
        }
        root_ports.extend(select_root_ports(
            &root,
            &component,
            &component_distances,
            &vlan_edges,
            bridges,
        ));
    }

    let mut states = Vec::with_capacity(vlan_edges.len().saturating_mul(2));
    for edge in vlan_edges {
        for (local, remote) in [(&edge.a, &edge.b), (&edge.b, &edge.a)] {
            let role = port_role(edge, local, remote, &root_ports, &distances, bridges);
            states.push(ScenarioSpanningTreeState {
                appliance: local.appliance.clone(),
                interface: local.interface.clone(),
                connection: edge.connection.clone(),
                protocol: bridges
                    .get(&local.appliance)
                    .expect("edge bridge")
                    .protocol
                    .to_string(),
                vlan,
                root_bridge: roots
                    .get(&local.appliance)
                    .expect("edge bridge has component root")
                    .clone(),
                root_path_cost: u32::try_from(
                    *distances
                        .get(&local.appliance)
                        .expect("edge bridge has root distance"),
                )
                .unwrap_or(u32::MAX),
                port_path_cost: long_path_cost(local.speed_mbps),
                state: if matches!(
                    role,
                    SpanningTreePortRole::Root | SpanningTreePortRole::Designated
                ) {
                    SpanningTreePortState::Forwarding
                } else {
                    SpanningTreePortState::Discarding
                },
                role,
            });
        }
    }
    states
}

fn connected_component(start: &str, edges: &[&Edge]) -> BTreeSet<String> {
    let mut component = BTreeSet::new();
    let mut pending = VecDeque::from([start.to_owned()]);
    while let Some(current) = pending.pop_front() {
        if !component.insert(current.clone()) {
            continue;
        }
        for edge in edges.iter().filter(|edge| edge.operational) {
            if edge.a.appliance == current {
                pending.push_back(edge.b.appliance.clone());
            } else if edge.b.appliance == current {
                pending.push_back(edge.a.appliance.clone());
            }
        }
    }
    component
}

fn root_distances(
    root: &str,
    component: &BTreeSet<String>,
    edges: &[&Edge],
) -> BTreeMap<String, u64> {
    let mut distances = component
        .iter()
        .map(|bridge| (bridge.clone(), u64::MAX))
        .collect::<BTreeMap<_, _>>();
    distances.insert(root.to_owned(), 0);
    for _ in 0..component.len() {
        let mut changed = false;
        for edge in edges.iter().filter(|edge| edge.operational) {
            let a_distance = distances[&edge.a.appliance];
            let b_distance = distances[&edge.b.appliance];
            if a_distance != u64::MAX {
                let candidate =
                    a_distance.saturating_add(u64::from(long_path_cost(edge.b.speed_mbps)));
                if candidate < b_distance {
                    distances.insert(edge.b.appliance.clone(), candidate);
                    changed = true;
                }
            }
            if b_distance != u64::MAX {
                let candidate =
                    b_distance.saturating_add(u64::from(long_path_cost(edge.a.speed_mbps)));
                if candidate < a_distance {
                    distances.insert(edge.a.appliance.clone(), candidate);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    distances
}

fn select_root_ports(
    root: &str,
    component: &BTreeSet<String>,
    distances: &BTreeMap<String, u64>,
    edges: &[&Edge],
    bridges: &BTreeMap<String, Bridge>,
) -> BTreeMap<String, (String, String)> {
    let mut root_ports = BTreeMap::new();
    for bridge in component.iter().filter(|bridge| bridge.as_str() != root) {
        let best = edges
            .iter()
            .filter(|edge| edge.operational)
            .filter_map(|edge| {
                let (local, neighbor) = if &edge.a.appliance == bridge {
                    (&edge.a, &edge.b)
                } else if &edge.b.appliance == bridge {
                    (&edge.b, &edge.a)
                } else {
                    return None;
                };
                let cost = distances[&neighbor.appliance]
                    .saturating_add(u64::from(long_path_cost(local.speed_mbps)));
                Some((
                    (
                        cost,
                        bridges[&neighbor.appliance].key,
                        neighbor.interface.as_str(),
                        local.interface.as_str(),
                        edge.connection.as_str(),
                    ),
                    (edge.connection.clone(), local.interface.clone()),
                ))
            })
            .min_by_key(|(rank, _)| *rank)
            .expect("non-root connected bridge has a root-port candidate")
            .1;
        root_ports.insert(bridge.clone(), best);
    }
    root_ports
}

fn port_role(
    edge: &Edge,
    local: &EdgeEndpoint,
    remote: &EdgeEndpoint,
    root_ports: &BTreeMap<String, (String, String)>,
    distances: &BTreeMap<String, u64>,
    bridges: &BTreeMap<String, Bridge>,
) -> SpanningTreePortRole {
    if !edge.operational {
        return SpanningTreePortRole::Disabled;
    }
    if root_ports.get(&local.appliance) == Some(&(edge.connection.clone(), local.interface.clone()))
    {
        return SpanningTreePortRole::Root;
    }
    let local_rank = (
        distances[&local.appliance],
        bridges[&local.appliance].key,
        local.interface.as_str(),
    );
    let remote_rank = (
        distances[&remote.appliance],
        bridges[&remote.appliance].key,
        remote.interface.as_str(),
    );
    if local_rank < remote_rank {
        SpanningTreePortRole::Designated
    } else {
        SpanningTreePortRole::Alternate
    }
}

fn long_path_cost(speed_mbps: u64) -> u32 {
    match speed_mbps {
        100_000.. => 200,
        40_000.. => 500,
        10_000.. => 2_000,
        1_000.. => 20_000,
        100.. => 200_000,
        10.. => 2_000_000,
        _ => 20_000_000,
    }
}

fn is_switch(kind: ComponentKind) -> bool {
    matches!(
        kind,
        ComponentKind::Layer2Switch | ComponentKind::Layer3Switch
    )
}

fn is_bridge_port(mode: InterfaceMode) -> bool {
    matches!(mode, InterfaceMode::Access | InterfaceMode::Trunk)
}
