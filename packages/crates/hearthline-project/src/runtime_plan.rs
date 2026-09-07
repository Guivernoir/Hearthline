use std::collections::{BTreeMap, BTreeSet};

use hearthline_config::{
    BehaviorConfig, ConfigError, ConfigRepository, ConnectionRepository, LoadedAppliance,
    RuntimeCapacityManifest, RuntimePartitionRule,
};
use hearthline_model::{BehaviorFamily, ComponentId, ComponentKind};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ObservedCapacityResource {
    ProcessPorts,
    ProcessTags,
    HmiCommandTags,
    AppliancePorts,
    SwitchPorts,
    Layer3SwitchPorts,
    UnaddressedPorts,
    CellComponents,
    CellLinks,
}

impl ObservedCapacityResource {
    pub(crate) const fn engine_key(self) -> &'static str {
        match self {
            Self::ProcessPorts => "industrial.process-ports",
            Self::ProcessTags => "industrial.process-tags",
            Self::HmiCommandTags => "industrial.hmi-command-tags",
            Self::AppliancePorts => "network.link-appliance-ports",
            Self::SwitchPorts => "network.switch-ports",
            Self::Layer3SwitchPorts => "network.layer3-switch-ports",
            Self::UnaddressedPorts => "network.unaddressed-ports",
            Self::CellComponents => "runtime.partition-components",
            Self::CellLinks => "runtime.partition-links",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapacityObservation {
    pub resource: ObservedCapacityResource,
    pub demand: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePartitionPlan {
    pub id: String,
    pub component_demand: usize,
    pub internal_link_demand: usize,
    pub boundary_link_demand: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeBoundaryPlan {
    pub connection_id: String,
    pub partition_a: String,
    pub partition_b: String,
}

#[derive(Clone, Debug)]
pub struct ProjectRuntimePlan {
    pub partitions: Vec<RuntimePartitionPlan>,
    pub boundaries: Vec<RuntimeBoundaryPlan>,
    pub observations: Vec<CapacityObservation>,
    appliance_partitions: BTreeMap<String, String>,
}

impl ProjectRuntimePlan {
    pub fn partition_for(&self, appliance_id: &str) -> Option<&str> {
        self.appliance_partitions
            .get(appliance_id)
            .map(String::as_str)
    }
}

pub fn compile_runtime_plan(
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    manifest: &RuntimeCapacityManifest,
) -> Result<ProjectRuntimePlan, ConfigError> {
    let mut appliance_partitions = BTreeMap::new();
    let mut matched_rules = BTreeSet::new();
    for loaded in appliances.appliances() {
        let matching_rules = manifest
            .partitioning
            .rules
            .iter()
            .filter(|rule| rule_matches(rule, loaded))
            .collect::<Vec<_>>();
        if matching_rules.len() > 1 {
            return Err(ConfigError::new(format!(
                "appliance {} matches multiple runtime partition rules",
                loaded.config.id
            )));
        }
        let partition = if let Some(rule) = matching_rules.first() {
            matched_rules.insert(rule.id.as_str());
            rule.id.clone()
        } else {
            format!(
                "{}-{}",
                slug(&loaded.config.site),
                slug(&loaded.config.environment)
            )
        };
        if ComponentId::new(&partition).is_err() {
            return Err(ConfigError::new(format!(
                "runtime cell id {partition} is not a valid model identifier"
            )));
        }
        appliance_partitions.insert(loaded.config.id.clone(), partition);
    }
    for rule in &manifest.partitioning.rules {
        if !matched_rules.contains(rule.id.as_str()) {
            return Err(ConfigError::new(format!(
                "runtime partition rule {} matches no canonical appliance",
                rule.id
            )));
        }
    }

    let mut demands: BTreeMap<String, RuntimePartitionPlan> = BTreeMap::new();
    for partition in appliance_partitions.values() {
        demands
            .entry(partition.clone())
            .or_insert_with(|| RuntimePartitionPlan {
                id: partition.clone(),
                component_demand: 0,
                internal_link_demand: 0,
                boundary_link_demand: 0,
            })
            .component_demand += 1;
    }

    let mut boundaries = Vec::new();
    for loaded in connections.connections() {
        let partition_a = appliance_partitions
            .get(&loaded.config.endpoints.a.appliance)
            .expect("connection repository validates appliance identities");
        let partition_b = appliance_partitions
            .get(&loaded.config.endpoints.b.appliance)
            .expect("connection repository validates appliance identities");
        if partition_a == partition_b {
            demands
                .get_mut(partition_a)
                .expect("partition demand exists")
                .internal_link_demand += 1;
        } else {
            demands
                .get_mut(partition_a)
                .expect("partition demand exists")
                .boundary_link_demand += 1;
            demands
                .get_mut(partition_b)
                .expect("partition demand exists")
                .boundary_link_demand += 1;
            boundaries.push(RuntimeBoundaryPlan {
                connection_id: loaded.config.id.clone(),
                partition_a: partition_a.clone(),
                partition_b: partition_b.clone(),
            });
        }
    }

    let partitions = demands.into_values().collect::<Vec<_>>();
    let observations = observe_capacity(appliances, &partitions);
    Ok(ProjectRuntimePlan {
        partitions,
        boundaries,
        observations,
        appliance_partitions,
    })
}

fn rule_matches(rule: &RuntimePartitionRule, loaded: &LoadedAppliance) -> bool {
    loaded.config.site == rule.site
        && loaded.config.environment == rule.environment
        && (rule.appliance_ids.iter().any(|id| id == &loaded.config.id)
            || rule
                .appliance_prefixes
                .iter()
                .any(|prefix| loaded.config.id.starts_with(prefix)))
}

fn slug(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !result.is_empty() {
                result.push('-');
            }
            result.push(character.to_ascii_lowercase());
            separator = false;
        } else {
            separator = true;
        }
    }
    result
}

fn observe_capacity(
    appliances: &ConfigRepository,
    partitions: &[RuntimePartitionPlan],
) -> Vec<CapacityObservation> {
    let mut process_ports = 0;
    let mut process_tags = 0;
    let mut hmi_commands = 0;
    let mut appliance_ports = 0;
    let mut switch_ports = 0;
    let mut layer3_switch_ports = 0;
    let mut unaddressed_ports = 0;
    for loaded in appliances.appliances() {
        let interface_count = loaded.config.interfaces.len();
        appliance_ports = appliance_ports.max(interface_count);
        if loaded.config.kind == ComponentKind::Layer2Switch {
            switch_ports = switch_ports.max(interface_count);
        }
        if loaded.config.kind == ComponentKind::Layer3Switch {
            layer3_switch_ports = layer3_switch_ports.max(interface_count);
        }
        if matches!(
            loaded.config.kind.behavior_family(),
            BehaviorFamily::VirtualController
                | BehaviorFamily::OperatorInterface
                | BehaviorFamily::RemoteIo
                | BehaviorFamily::FieldSensor
                | BehaviorFamily::FieldActuator
                | BehaviorFamily::Safety
        ) {
            process_ports = process_ports.max(interface_count);
        }
        let requires_addressing = matches!(
            loaded.config.behavior,
            BehaviorConfig::Endpoint { .. }
                | BehaviorConfig::ServiceHost { .. }
                | BehaviorConfig::PolicyService { .. }
                | BehaviorConfig::Voice { .. }
                | BehaviorConfig::ComputeHost { .. }
                | BehaviorConfig::Router { .. }
                | BehaviorConfig::NatRouter { .. }
                | BehaviorConfig::StatefulFirewall { .. }
                | BehaviorConfig::ApplicationGateway { .. }
        );
        let addressed = loaded
            .config
            .interfaces
            .iter()
            .filter(|interface| !interface.addresses.is_empty())
            .collect::<Vec<_>>();
        let virtual_switch_host =
            matches!(loaded.config.behavior, BehaviorConfig::ComputeHost { .. })
                && addressed.is_empty();
        if requires_addressing
            && !virtual_switch_host
            && (addressed.is_empty()
                || addressed
                    .iter()
                    .any(|interface| interface.mac_address.is_none()))
        {
            unaddressed_ports = unaddressed_ports.max(interface_count);
        }
        match &loaded.config.behavior {
            BehaviorConfig::RemoteIo { channels, .. } => {
                process_tags = process_tags.max(channels.len());
            }
            BehaviorConfig::OperatorInterface { command_tags, .. } => {
                hmi_commands = hmi_commands.max(command_tags.len());
            }
            _ => {}
        }
    }
    let mut observations = vec![
        observation(ObservedCapacityResource::ProcessPorts, process_ports),
        observation(ObservedCapacityResource::ProcessTags, process_tags),
        observation(ObservedCapacityResource::HmiCommandTags, hmi_commands),
        observation(ObservedCapacityResource::AppliancePorts, appliance_ports),
        observation(ObservedCapacityResource::SwitchPorts, switch_ports),
        observation(
            ObservedCapacityResource::Layer3SwitchPorts,
            layer3_switch_ports,
        ),
        observation(
            ObservedCapacityResource::UnaddressedPorts,
            unaddressed_ports,
        ),
        observation(
            ObservedCapacityResource::CellComponents,
            partitions
                .iter()
                .map(|item| item.component_demand)
                .max()
                .unwrap_or(0),
        ),
        observation(
            ObservedCapacityResource::CellLinks,
            partitions
                .iter()
                .map(|item| item.internal_link_demand)
                .max()
                .unwrap_or(0),
        ),
    ];
    observations.sort_by_key(|observation| observation.resource);
    observations
}

const fn observation(resource: ObservedCapacityResource, demand: usize) -> CapacityObservation {
    CapacityObservation { resource, demand }
}

#[cfg(test)]
mod tests {
    use super::ObservedCapacityResource;
    use std::collections::BTreeSet;

    #[test]
    fn every_observed_capacity_resource_maps_to_a_unique_engine_key() {
        let resources = [
            ObservedCapacityResource::ProcessPorts,
            ObservedCapacityResource::ProcessTags,
            ObservedCapacityResource::HmiCommandTags,
            ObservedCapacityResource::AppliancePorts,
            ObservedCapacityResource::SwitchPorts,
            ObservedCapacityResource::Layer3SwitchPorts,
            ObservedCapacityResource::UnaddressedPorts,
            ObservedCapacityResource::CellComponents,
            ObservedCapacityResource::CellLinks,
        ];
        let keys = resources.map(ObservedCapacityResource::engine_key);
        assert_eq!(
            keys.into_iter().collect::<BTreeSet<_>>().len(),
            resources.len()
        );
    }
}
