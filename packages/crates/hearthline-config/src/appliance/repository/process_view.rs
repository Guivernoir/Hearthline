use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use hearthline_model::ComponentKind;
use serde::{Deserialize, Serialize};

use super::{ConfigRepository, LoadedAppliance};
use crate::ConfigError;

pub const PROCESS_VIEW_SCHEMA_VERSION: &str = "0.3.0";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessViewConfig {
    schema_version: String,
    support_nodes: Vec<ProcessSupportNode>,
    areas: Vec<ProcessAreaConfig>,
    network_edges: Vec<ProcessEdge>,
    material_flow: Vec<ProcessEdge>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProcessPosition {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProcessSupportNode {
    id: String,
    label: String,
    subtitle: String,
    zone: String,
    accent: String,
    icon: String,
    tags: Vec<String>,
    detail: String,
    position: ProcessPosition,
    kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProcessAreaConfig {
    id: String,
    route_key: String,
    environment: String,
    label: String,
    subtitle: String,
    zone: String,
    accent: String,
    icon: String,
    tags: Vec<String>,
    detail: String,
    position: ProcessPosition,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessEdge {
    source: String,
    destination: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendProcessView {
    schema_version: &'static str,
    generation_status: &'static str,
    generated_by: &'static str,
    source_root: &'static str,
    support_nodes: Vec<ProcessSupportNode>,
    areas: Vec<FrontendProcessArea>,
    network_edges: Vec<ProcessEdge>,
    material_flow: Vec<ProcessEdge>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendProcessArea {
    id: String,
    route_key: String,
    label: String,
    subtitle: String,
    zone: String,
    accent: String,
    icon: String,
    tags: Vec<String>,
    detail: String,
    position: ProcessPosition,
    equipment: Vec<FrontendProcessEquipment>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendProcessEquipment {
    id: String,
    label: String,
    kind: &'static str,
    role: String,
    icon: &'static str,
    accent: &'static str,
    slot: &'static str,
    link_kind: &'static str,
    upstream: Option<String>,
    physical_upstream: Option<String>,
    config_ref: String,
    facts: Vec<String>,
}

impl ProcessViewConfig {
    pub fn load(
        path: impl AsRef<Path>,
        appliances: &ConfigRepository,
    ) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let source = fs::read_to_string(path).map_err(|error| {
            ConfigError::new(format!("cannot read {}: {error}", path.display()))
        })?;
        let config: Self = serde_yaml_ng::from_str(&source).map_err(|error| {
            ConfigError::new(format!("cannot parse {}: {error}", path.display()))
        })?;
        config.validate(appliances)?;
        Ok(config)
    }

    pub fn into_frontend(
        self,
        appliances: &ConfigRepository,
    ) -> Result<FrontendProcessView, ConfigError> {
        let mut areas = Vec::with_capacity(self.areas.len());
        for area in self.areas {
            let equipment = if matches!(area.route_key.as_str(), "body-preparation" | "forming") {
                Vec::new()
            } else {
                equipment_for_area(appliances, &area.environment)?
            };
            areas.push(FrontendProcessArea {
                id: area.id,
                route_key: area.route_key,
                label: area.label,
                subtitle: area.subtitle,
                zone: area.zone,
                accent: area.accent,
                icon: area.icon,
                tags: area.tags,
                detail: area.detail,
                position: area.position,
                equipment,
            });
        }
        Ok(FrontendProcessView {
            schema_version: PROCESS_VIEW_SCHEMA_VERSION,
            generation_status: "generated",
            generated_by: "hearthline-config process topology pipeline",
            source_root: "config/ot/process",
            support_nodes: self.support_nodes,
            areas,
            network_edges: self.network_edges,
            material_flow: self.material_flow,
        })
    }

    fn validate(&self, appliances: &ConfigRepository) -> Result<(), ConfigError> {
        if self.schema_version != PROCESS_VIEW_SCHEMA_VERSION {
            return Err(ConfigError::new(format!(
                "unsupported process-view schema {}; expected {PROCESS_VIEW_SCHEMA_VERSION}",
                self.schema_version
            )));
        }
        if self.support_nodes.is_empty() || self.areas.is_empty() {
            return Err(ConfigError::new(
                "process topology requires support nodes and process areas",
            ));
        }
        let mut nodes = BTreeSet::new();
        let mut routes = BTreeSet::new();
        for node in &self.support_nodes {
            validate_node(&node.id, &node.label, &node.position)?;
            if !matches!(node.kind.as_str(), "boundary" | "platform") {
                return Err(ConfigError::new(format!(
                    "process support node {} has invalid kind {}",
                    node.id, node.kind
                )));
            }
            if !nodes.insert(node.id.as_str()) {
                return Err(ConfigError::new(format!(
                    "duplicate process node {}",
                    node.id
                )));
            }
        }
        for area in &self.areas {
            validate_node(&area.id, &area.label, &area.position)?;
            if !nodes.insert(area.id.as_str()) {
                return Err(ConfigError::new(format!(
                    "duplicate process node {}",
                    area.id
                )));
            }
            if !routes.insert(area.route_key.as_str()) {
                return Err(ConfigError::new(format!(
                    "duplicate process route {}",
                    area.route_key
                )));
            }
            if !appliances
                .appliances()
                .any(|loaded| loaded.config.environment == area.environment)
            {
                return Err(ConfigError::new(format!(
                    "process area {} has no appliances in environment {}",
                    area.id, area.environment
                )));
            }
        }
        validate_edges("network", &self.network_edges, &nodes)?;
        validate_edges("material", &self.material_flow, &nodes)?;
        Ok(())
    }
}

fn validate_node(id: &str, label: &str, position: &ProcessPosition) -> Result<(), ConfigError> {
    if id.is_empty() || label.trim().is_empty() {
        return Err(ConfigError::new("process nodes require an id and label"));
    }
    if position.width == 0 || position.height == 0 {
        return Err(ConfigError::new(format!(
            "process node {id} requires positive dimensions"
        )));
    }
    Ok(())
}

fn validate_edges(
    kind: &str,
    edges: &[ProcessEdge],
    nodes: &BTreeSet<&str>,
) -> Result<(), ConfigError> {
    let mut identities = BTreeSet::new();
    for edge in edges {
        if edge.source == edge.destination
            || !nodes.contains(edge.source.as_str())
            || !nodes.contains(edge.destination.as_str())
        {
            return Err(ConfigError::new(format!(
                "invalid {kind} edge {} -> {}",
                edge.source, edge.destination
            )));
        }
        if !identities.insert((edge.source.as_str(), edge.destination.as_str())) {
            return Err(ConfigError::new(format!(
                "duplicate {kind} edge {} -> {}",
                edge.source, edge.destination
            )));
        }
    }
    Ok(())
}

fn equipment_for_area(
    appliances: &ConfigRepository,
    environment: &str,
) -> Result<Vec<FrontendProcessEquipment>, ConfigError> {
    let mut area: Vec<_> = appliances
        .appliances()
        .filter(|loaded| loaded.config.environment == environment)
        .collect();
    area.sort_by(|left, right| left.config.id.cmp(&right.config.id));
    let switch = unique_kind(&area, ComponentKind::Layer2Switch, environment)?;
    let controller = unique_kind(&area, ComponentKind::VirtualPlc, environment)?;
    let hmi = unique_kind(&area, ComponentKind::Hmi, environment)?;
    let remote_io = unique_kind(&area, ComponentKind::RemoteIo, environment)?;
    let safety = unique_kind(&area, ComponentKind::SafetyInterface, environment)?;
    let sensors: Vec<_> = area
        .iter()
        .copied()
        .filter(|loaded| loaded.config.kind == ComponentKind::FieldSensor)
        .collect();
    let actuators: Vec<_> = area
        .iter()
        .copied()
        .filter(|loaded| loaded.config.kind == ComponentKind::FieldActuator)
        .collect();
    if sensors.len() != 2 || actuators.len() != 2 || area.len() != 9 {
        return Err(ConfigError::new(format!(
            "process environment {environment} requires the nine-appliance area contract"
        )));
    }

    Ok(vec![
        equipment(switch, "switch", None, None),
        equipment(controller, "controller", Some(switch), None),
        equipment(hmi, "hmi", Some(controller), Some(switch)),
        equipment(remote_io, "remote-io", Some(controller), Some(switch)),
        equipment(sensors[0], "sensor-a", Some(remote_io), None),
        equipment(sensors[1], "sensor-b", Some(remote_io), None),
        equipment(actuators[0], "actuator-a", Some(remote_io), None),
        equipment(actuators[1], "actuator-b", Some(remote_io), None),
        equipment(safety, "safety", Some(controller), Some(switch)),
    ])
}

fn unique_kind<'a>(
    appliances: &[&'a LoadedAppliance],
    kind: ComponentKind,
    environment: &str,
) -> Result<&'a LoadedAppliance, ConfigError> {
    let mut matching = appliances
        .iter()
        .copied()
        .filter(|loaded| loaded.config.kind == kind);
    let appliance = matching.next().ok_or_else(|| {
        ConfigError::new(format!("process environment {environment} has no {kind}"))
    })?;
    if matching.next().is_some() {
        return Err(ConfigError::new(format!(
            "process environment {environment} has multiple {kind} appliances"
        )));
    }
    Ok(appliance)
}

fn equipment(
    loaded: &LoadedAppliance,
    slot: &'static str,
    upstream: Option<&LoadedAppliance>,
    physical_upstream: Option<&LoadedAppliance>,
) -> FrontendProcessEquipment {
    let (kind, icon, accent, link_kind) = equipment_presentation(loaded.config.kind);
    let mut facts = loaded.config.behavior.facts();
    facts.insert(0, loaded.config.summary.clone());
    FrontendProcessEquipment {
        id: loaded.config.id.clone(),
        label: loaded.config.label.clone(),
        kind,
        role: loaded.config.role.clone(),
        icon,
        accent,
        slot,
        link_kind,
        upstream: upstream.map(|value| value.config.id.clone()),
        physical_upstream: physical_upstream.map(|value| value.config.id.clone()),
        config_ref: loaded.source_path.clone(),
        facts,
    }
}

fn equipment_presentation(
    kind: ComponentKind,
) -> (&'static str, &'static str, &'static str, &'static str) {
    match kind {
        ComponentKind::Layer2Switch => ("network", "network", "#3567a6", "ethernet"),
        ComponentKind::VirtualPlc => ("controller", "cpu", "#267168", "ethernet"),
        ComponentKind::Hmi => ("operator interface", "monitor", "#426d9d", "ethernet"),
        ComponentKind::RemoteIo => ("distributed I/O", "remote-io", "#7a6546", "ethernet"),
        ComponentKind::FieldSensor => ("sensor", "gauge", "#51704c", "io"),
        ComponentKind::FieldActuator => ("actuator", "valve", "#b65034", "io"),
        ComponentKind::SafetyInterface => {
            ("safety interface", "shield", "#9e3f2f", "safety-status")
        }
        _ => unreachable!("validated process equipment kind"),
    }
}
