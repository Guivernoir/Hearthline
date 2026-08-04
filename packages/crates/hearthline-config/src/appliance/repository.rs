use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::connection::ConnectionRepository;

use super::{
    APPLIANCE_SCHEMA_VERSION, ApplianceConfig, ConfigError, FRONTEND_CATALOG_SCHEMA_VERSION,
    FrontendAppliance, FrontendApplianceCatalog, collect_yaml_paths, source_revision,
};

#[derive(Clone, Debug)]
pub struct LoadedAppliance {
    pub config: ApplianceConfig,
    pub source_path: String,
    pub source_yaml: String,
    pub source_file: PathBuf,
}

impl LoadedAppliance {
    pub fn revision(&self) -> String {
        source_revision(&self.source_yaml)
    }
}

#[derive(Clone, Debug)]
pub struct ConfigRepository {
    appliances: BTreeMap<String, LoadedAppliance>,
}

impl ConfigRepository {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, ConfigError> {
        Self::load_with_override(root, None)
    }

    pub fn load_with_override(
        root: impl AsRef<Path>,
        source_override: Option<(&Path, &str)>,
    ) -> Result<Self, ConfigError> {
        let root = root.as_ref();
        let mut paths = Vec::new();
        collect_yaml_paths(root, &mut paths)?;
        paths.sort();
        if paths.is_empty() {
            return Err(ConfigError::new(format!(
                "{} contains no appliance YAML files",
                root.display()
            )));
        }

        let source_base = root
            .parent()
            .and_then(Path::parent)
            .or_else(|| root.parent())
            .unwrap_or(root);
        let mut appliances = BTreeMap::new();
        for path in paths {
            let source_yaml = if source_override
                .as_ref()
                .is_some_and(|(override_path, _)| *override_path == path)
            {
                source_override
                    .as_ref()
                    .map(|(_, source)| (*source).to_owned())
                    .unwrap_or_default()
            } else {
                fs::read_to_string(&path).map_err(|error| {
                    ConfigError::new(format!("cannot read {}: {error}", path.display()))
                })?
            };
            let config =
                ApplianceConfig::from_yaml(&source_yaml).map_err(|error| error.with_path(&path))?;
            let expected_file = format!("{}.yaml", config.id);
            let actual_file = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if actual_file != expected_file {
                return Err(ConfigError::new(format!(
                    "{} must be named {} to preserve one-file-per-appliance identity",
                    path.display(),
                    expected_file
                )));
            }
            let source_path = path
                .strip_prefix(source_base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let id = config.id.clone();
            if appliances
                .insert(
                    id.clone(),
                    LoadedAppliance {
                        config,
                        source_path,
                        source_yaml,
                        source_file: path,
                    },
                )
                .is_some()
            {
                return Err(ConfigError::new(format!("duplicate appliance id {id}")));
            }
        }

        validate_first_hop_groups(&appliances)?;
        validate_spanning_tree_bridges(&appliances)?;
        validate_redundancy_appliances(&appliances)?;
        let repository = Self { appliances };
        crate::hmi::validate_repository(&repository)?;
        Ok(repository)
    }

    pub fn appliances(&self) -> impl Iterator<Item = &LoadedAppliance> {
        self.appliances.values()
    }

    pub fn get(&self, id: &str) -> Option<&LoadedAppliance> {
        self.appliances.get(id)
    }

    pub fn len(&self) -> usize {
        self.appliances.len()
    }

    pub fn is_empty(&self) -> bool {
        self.appliances.is_empty()
    }

    pub fn frontend_catalog(&self, connections: &ConnectionRepository) -> FrontendApplianceCatalog {
        let mut node_index: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let appliances = self
            .appliances
            .values()
            .map(|loaded| {
                for binding in &loaded.config.render {
                    let key = format!("{}:{}:{}", binding.view, binding.node, binding.mode);
                    node_index
                        .entry(key)
                        .or_default()
                        .push(loaded.config.id.clone());
                }
                FrontendAppliance::from(loaded)
            })
            .collect();

        FrontendApplianceCatalog {
            schema_version: FRONTEND_CATALOG_SCHEMA_VERSION,
            appliance_schema_version: APPLIANCE_SCHEMA_VERSION,
            generation_status: "generated",
            generated_by: "hearthline-engine configuration pipeline",
            appliance_source_root: "config/appliances",
            connection_source_root: "config/connections",
            appliances,
            node_index,
            connections: connections.frontend_connections(self),
            appliance_connection_index: connections.appliance_index(),
        }
    }
}

fn validate_redundancy_appliances(
    appliances: &BTreeMap<String, LoadedAppliance>,
) -> Result<(), ConfigError> {
    validate_firewall_ha_appliances(appliances)?;
    let mut system_macs: BTreeMap<&str, Vec<&LoadedAppliance>> = BTreeMap::new();
    for appliance in appliances.values() {
        let Some(link_aggregation) = &appliance.config.link_aggregation else {
            continue;
        };
        system_macs
            .entry(link_aggregation.system_mac.as_str())
            .or_default()
            .push(appliance);
    }

    for (system_mac, members) in system_macs {
        if members.len() == 1 {
            continue;
        }
        if members.len() != 2 {
            return Err(ConfigError::new(format!(
                "LACP system MAC {system_mac} is shared by more than one multi-chassis pair"
            )));
        }
        let [left, right] = [members[0], members[1]];
        let left_domain = left.config.multi_chassis.as_ref().ok_or_else(|| {
            ConfigError::new(format!(
                "LACP system MAC {system_mac} is shared by standalone appliance {}",
                left.config.id
            ))
        })?;
        let right_domain = right.config.multi_chassis.as_ref().ok_or_else(|| {
            ConfigError::new(format!(
                "LACP system MAC {system_mac} is shared by standalone appliance {}",
                right.config.id
            ))
        })?;
        if left_domain.domain != right_domain.domain
            || left_domain.peer != right.config.id
            || right_domain.peer != left.config.id
        {
            return Err(ConfigError::new(format!(
                "LACP system MAC {system_mac} must belong to one reciprocal multi-chassis pair"
            )));
        }
    }

    for appliance in appliances.values() {
        let Some(domain) = &appliance.config.multi_chassis else {
            continue;
        };
        let peer = appliances.get(&domain.peer).ok_or_else(|| {
            ConfigError::new(format!(
                "appliance {} references unknown multi-chassis peer {}",
                appliance.config.id, domain.peer
            ))
        })?;
        let peer_domain = peer.config.multi_chassis.as_ref().ok_or_else(|| {
            ConfigError::new(format!(
                "appliance {} multi-chassis peer {} does not define a domain",
                appliance.config.id, peer.config.id
            ))
        })?;
        if peer_domain.peer != appliance.config.id
            || peer_domain.domain != domain.domain
            || peer_domain.role == domain.role
        {
            return Err(ConfigError::new(format!(
                "appliances {} and {} require reciprocal multi-chassis configuration with opposite roles",
                appliance.config.id, peer.config.id
            )));
        }
        let system_mac = &appliance
            .config
            .link_aggregation
            .as_ref()
            .expect("local multi-chassis validation requires link aggregation")
            .system_mac;
        let peer_system_mac = &peer
            .config
            .link_aggregation
            .as_ref()
            .ok_or_else(|| {
                ConfigError::new(format!(
                    "multi-chassis peer {} does not define link aggregation",
                    peer.config.id
                ))
            })?
            .system_mac;
        if system_mac != peer_system_mac {
            return Err(ConfigError::new(format!(
                "multi-chassis domain {} requires one shared LACP system MAC",
                domain.domain
            )));
        }
    }
    Ok(())
}

fn validate_firewall_ha_appliances(
    appliances: &BTreeMap<String, LoadedAppliance>,
) -> Result<(), ConfigError> {
    for appliance in appliances.values() {
        let Some(ha) = &appliance.config.firewall_ha else {
            continue;
        };
        let peer = appliances.get(&ha.peer).ok_or_else(|| {
            ConfigError::new(format!(
                "firewall {} references unknown HA peer {}",
                appliance.config.id, ha.peer
            ))
        })?;
        let peer_ha = peer.config.firewall_ha.as_ref().ok_or_else(|| {
            ConfigError::new(format!(
                "firewall {} HA peer {} does not declare firewall HA",
                appliance.config.id, peer.config.id
            ))
        })?;
        if peer_ha.peer != appliance.config.id
            || peer_ha.domain != ha.domain
            || peer_ha.role == ha.role
            || peer_ha.session_sync != ha.session_sync
            || peer_ha.heartbeat_interval_ms != ha.heartbeat_interval_ms
            || peer_ha.failure_hold_ms != ha.failure_hold_ms
        {
            return Err(ConfigError::new(format!(
                "firewalls {} and {} require reciprocal HA configuration, opposite roles, and matching timers",
                appliance.config.id, peer.config.id
            )));
        }
        if ha.monitored_interfaces != peer_ha.monitored_interfaces {
            return Err(ConfigError::new(format!(
                "firewall HA domain {} requires matching monitored interface order",
                ha.domain
            )));
        }
        validate_firewall_virtual_identities(appliance, peer)?;
        validate_firewall_policy_equivalence(appliance, peer)?;
    }
    Ok(())
}

fn validate_firewall_virtual_identities(
    appliance: &LoadedAppliance,
    peer: &LoadedAppliance,
) -> Result<(), ConfigError> {
    let ha = appliance
        .config
        .firewall_ha
        .as_ref()
        .expect("caller selected an HA firewall");
    for interface_id in &ha.monitored_interfaces {
        let interface = appliance
            .config
            .interfaces
            .iter()
            .find(|interface| interface.id == *interface_id)
            .expect("local firewall HA validation checked interface existence");
        let peer_interface = peer
            .config
            .interfaces
            .iter()
            .find(|candidate| candidate.id == *interface_id)
            .ok_or_else(|| {
                ConfigError::new(format!(
                    "firewall {} HA peer {} is missing monitored interface {interface_id}",
                    appliance.config.id, peer.config.id
                ))
            })?;
        let identity = interface
            .first_hop
            .as_ref()
            .expect("local firewall HA validation requires virtual identity");
        let peer_identity = peer_interface.first_hop.as_ref().ok_or_else(|| {
            ConfigError::new(format!(
                "firewall {} interface {interface_id} is missing the HA virtual identity",
                peer.config.id
            ))
        })?;
        if identity.protocol != peer_identity.protocol
            || identity.group != peer_identity.group
            || identity.virtual_ip != peer_identity.virtual_ip
            || identity.virtual_mac != peer_identity.virtual_mac
        {
            return Err(ConfigError::new(format!(
                "firewall HA domain {} interface {interface_id} requires one shared virtual identity",
                ha.domain
            )));
        }
    }
    Ok(())
}

fn validate_firewall_policy_equivalence(
    appliance: &LoadedAppliance,
    peer: &LoadedAppliance,
) -> Result<(), ConfigError> {
    let (
        super::BehaviorConfig::StatefulFirewall {
            stateful,
            default_action,
            zones,
            routes,
            rules,
        },
        super::BehaviorConfig::StatefulFirewall {
            stateful: peer_stateful,
            default_action: peer_default_action,
            zones: peer_zones,
            routes: peer_routes,
            rules: peer_rules,
        },
    ) = (&appliance.config.behavior, &peer.config.behavior)
    else {
        return Err(ConfigError::new(format!(
            "firewall HA domain members {} and {} require stateful-firewall behavior",
            appliance.config.id, peer.config.id
        )));
    };
    let route_shapes = routes
        .iter()
        .map(|route| (&route.destination, &route.interface))
        .collect::<Vec<_>>();
    let peer_route_shapes = peer_routes
        .iter()
        .map(|route| (&route.destination, &route.interface))
        .collect::<Vec<_>>();
    if stateful != peer_stateful
        || default_action != peer_default_action
        || zones != peer_zones
        || rules != peer_rules
        || route_shapes != peer_route_shapes
    {
        return Err(ConfigError::new(format!(
            "firewall HA domain {} requires synchronized stateful policy, zones, and route destinations",
            appliance
                .config
                .firewall_ha
                .as_ref()
                .expect("caller selected an HA firewall")
                .domain
        )));
    }
    Ok(())
}

fn validate_spanning_tree_bridges(
    appliances: &BTreeMap<String, LoadedAppliance>,
) -> Result<(), ConfigError> {
    let mut bridge_macs = BTreeMap::new();
    for appliance in appliances.values() {
        let Some(spanning_tree) = &appliance.config.spanning_tree else {
            continue;
        };
        if let Some(existing) = bridge_macs.insert(
            spanning_tree.bridge_mac.as_str(),
            appliance.config.id.as_str(),
        ) {
            return Err(ConfigError::new(format!(
                "spanning-tree bridge MAC {} is shared by {existing} and {}",
                spanning_tree.bridge_mac, appliance.config.id
            )));
        }
    }
    Ok(())
}

fn validate_first_hop_groups(
    appliances: &BTreeMap<String, LoadedAppliance>,
) -> Result<(), ConfigError> {
    let mut groups = BTreeMap::new();
    for appliance in appliances.values() {
        for interface in &appliance.config.interfaces {
            let Some(first_hop) = &interface.first_hop else {
                continue;
            };
            groups
                .entry((
                    appliance.config.site.as_str(),
                    appliance.config.environment.as_str(),
                    first_hop.protocol,
                    first_hop.group,
                ))
                .or_insert_with(Vec::new)
                .push((
                    appliance.config.id.as_str(),
                    interface.id.as_str(),
                    first_hop,
                ));
        }
    }

    for ((site, environment, protocol, group), members) in groups {
        if members.len() < 2 {
            return Err(ConfigError::new(format!(
                "{site} {environment} {protocol} group {group} requires at least two members"
            )));
        }
        let (_, _, reference) = members[0];
        let mut appliances = BTreeSet::new();
        let mut priorities = BTreeSet::new();
        let mut active = 0;
        for (appliance, interface, member) in members {
            if !appliances.insert(appliance) {
                return Err(ConfigError::new(format!(
                    "appliance {appliance} repeats {protocol} group {group}"
                )));
            }
            if member.virtual_ip != reference.virtual_ip
                || member.virtual_mac != reference.virtual_mac
            {
                return Err(ConfigError::new(format!(
                    "appliance {appliance} interface {interface} does not match the virtual identity for {protocol} group {group}"
                )));
            }
            if !priorities.insert(member.priority) {
                return Err(ConfigError::new(format!(
                    "{site} {environment} {protocol} group {group} repeats priority {}",
                    member.priority
                )));
            }
            active += usize::from(member.initial_role.is_active());
        }
        if active != 1 {
            return Err(ConfigError::new(format!(
                "{site} {environment} {protocol} group {group} requires exactly one initial active member"
            )));
        }
    }
    Ok(())
}
