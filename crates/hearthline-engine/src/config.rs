use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter, Write as _};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use hearthline_model::{BehaviorFamily, ComponentId, ComponentKind};
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

use crate::connection::{ConnectionRepository, FrontendConnection};
use crate::port::{
    PortHardwareKind, PortSettings, PortState, PortStateConfig, appliance_supports_port,
};

pub const APPLIANCE_SCHEMA_VERSION: &str = "0.3.0";
pub const FRONTEND_CATALOG_SCHEMA_VERSION: &str = "0.3.0";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplianceConfig {
    pub schema_version: String,
    pub id: String,
    pub label: String,
    #[serde(deserialize_with = "deserialize_component_kind")]
    pub kind: ComponentKind,
    pub site: String,
    pub environment: String,
    pub zone: String,
    pub role: String,
    pub summary: String,
    #[serde(default)]
    pub lifecycle: Lifecycle,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub render: Vec<RenderBinding>,
    #[serde(default)]
    pub interfaces: Vec<InterfaceConfig>,
    pub behavior: BehaviorConfig,
}

impl ApplianceConfig {
    pub fn from_yaml(source: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_yaml_ng::from_str(source)
            .map_err(|error| ConfigError::new(format!("invalid YAML: {error}")))?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != APPLIANCE_SCHEMA_VERSION {
            return Err(ConfigError::new(format!(
                "appliance {} uses schema {}, expected {}",
                self.id, self.schema_version, APPLIANCE_SCHEMA_VERSION
            )));
        }
        ComponentId::new(self.id.clone()).map_err(|error| ConfigError::new(error.to_string()))?;
        require_text("label", &self.label)?;
        require_text("site", &self.site)?;
        require_text("environment", &self.environment)?;
        require_text("zone", &self.zone)?;
        require_text("role", &self.role)?;
        require_text("summary", &self.summary)?;

        let expected_family = self.kind.behavior_family();
        let configured_family = self.behavior.family();
        if expected_family != configured_family {
            return Err(ConfigError::new(format!(
                "appliance {} kind {} requires behavior family {}, not {}",
                self.id, self.kind, expected_family, configured_family
            )));
        }

        let mut interface_ids = BTreeSet::new();
        for interface in &self.interfaces {
            ComponentId::new(interface.id.clone()).map_err(|error| {
                ConfigError::new(format!(
                    "appliance {} has invalid interface id: {error}",
                    self.id
                ))
            })?;
            if !interface_ids.insert(&interface.id) {
                return Err(ConfigError::new(format!(
                    "appliance {} repeats interface {}",
                    self.id, interface.id
                )));
            }
            if !appliance_supports_port(self.kind, interface.hardware) {
                return Err(ConfigError::new(format!(
                    "appliance {} kind {} does not support {} port {}",
                    self.id, self.kind, interface.hardware, interface.id
                )));
            }
            if interface.state.administrative == PortState::Down
                && interface.state.initial_operational == PortState::Up
            {
                return Err(ConfigError::new(format!(
                    "appliance {} port {} cannot be operationally up while administratively down",
                    self.id, interface.id
                )));
            }
            interface.settings.validate().map_err(|error| {
                ConfigError::new(format!(
                    "appliance {} port {}: {error}",
                    self.id, interface.id
                ))
            })?;
        }

        let mut bindings = BTreeSet::new();
        for binding in &self.render {
            require_text("render.view", &binding.view)?;
            require_text("render.node", &binding.node)?;
            if !bindings.insert((&binding.view, &binding.node, binding.mode)) {
                return Err(ConfigError::new(format!(
                    "appliance {} repeats render binding {}:{}:{:?}",
                    self.id, binding.view, binding.node, binding.mode
                )));
            }
        }

        self.behavior.validate(&self.id)
    }

    pub fn behavior_family(&self) -> BehaviorFamily {
        self.behavior.family()
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum Lifecycle {
    #[default]
    Design,
    Configured,
    Simulated,
}

impl Display for Lifecycle {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Design => "design",
            Self::Configured => "configured",
            Self::Simulated => "simulated",
        };
        formatter.write_str(value)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum RenderMode {
    #[default]
    Any,
    Physical,
    Logical,
}

impl Display for RenderMode {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Any => "any",
            Self::Physical => "physical",
            Self::Logical => "logical",
        };
        formatter.write_str(value)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderBinding {
    pub view: String,
    pub node: String,
    #[serde(default)]
    pub mode: RenderMode,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterfaceConfig {
    pub id: String,
    pub hardware: PortHardwareKind,
    pub state: PortStateConfig,
    pub settings: PortSettings,
    pub mode: InterfaceMode,
    #[serde(default)]
    pub addresses: Vec<String>,
    #[serde(default)]
    pub vlans: Vec<u16>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InterfaceMode {
    Access,
    Trunk,
    Routed,
    Transparent,
    Management,
    Monitor,
    FieldIo,
}

impl Display for InterfaceMode {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Access => "access",
            Self::Trunk => "trunk",
            Self::Routed => "routed",
            Self::Transparent => "transparent",
            Self::Management => "management",
            Self::Monitor => "monitor",
            Self::FieldIo => "field-io",
        };
        formatter.write_str(value)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "family", rename_all = "kebab-case", deny_unknown_fields)]
pub enum BehaviorConfig {
    Endpoint {
        #[serde(default)]
        accepted_services: Vec<String>,
        #[serde(default = "default_true")]
        respond_to_icmp: bool,
    },
    ServiceHost {
        accepted_services: Vec<String>,
        #[serde(default = "default_true")]
        respond_to_icmp: bool,
    },
    PolicyService {
        accepted_services: Vec<String>,
        decision_inputs: Vec<String>,
    },
    TransparentLink {
        #[serde(default = "default_true")]
        operational: bool,
    },
    ImpairedLink {
        #[serde(default = "default_true")]
        operational: bool,
        delay_ms: u64,
        #[serde(default)]
        loss_every: Option<u64>,
    },
    EthernetSwitch {
        vlans: Vec<u16>,
        #[serde(default)]
        management_vlan: Option<u16>,
        #[serde(default = "default_true")]
        spanning_tree: bool,
    },
    Router {
        routes: Vec<RouteConfig>,
        #[serde(default = "default_true")]
        forwarding: bool,
    },
    NatRouter {
        routes: Vec<RouteConfig>,
        inside_interfaces: Vec<String>,
        outside_interfaces: Vec<String>,
        #[serde(default)]
        translations: Vec<NatTranslationConfig>,
    },
    StatefulFirewall {
        #[serde(default = "default_true")]
        stateful: bool,
        default_action: PolicyAction,
        #[serde(default)]
        rules: Vec<PolicyRuleConfig>,
    },
    ApplicationGateway {
        listeners: Vec<ListenerConfig>,
        upstreams: Vec<String>,
        #[serde(default)]
        max_request_bytes: Option<u64>,
    },
    WirelessBridge {
        ssid: String,
        client_vlan: u16,
        #[serde(default = "default_true")]
        client_isolation: bool,
    },
    PassiveMonitor {
        capture_sources: Vec<String>,
        inline: bool,
    },
    Voice {
        accepted_services: Vec<String>,
        #[serde(default)]
        extension: Option<String>,
        #[serde(default)]
        call_controller: Option<String>,
    },
    ComputeHost {
        accepted_services: Vec<String>,
        workloads: Vec<String>,
    },
    VirtualController {
        scan_interval_ms: u64,
        program_ref: String,
        io_binding: String,
    },
    OperatorInterface {
        controller: String,
        permissions: Vec<String>,
    },
    RemoteIo {
        controller: String,
        channels: Vec<String>,
    },
    FieldSensor {
        signal_tag: String,
        unit: String,
        minimum: f64,
        maximum: f64,
    },
    FieldActuator {
        command_tag: String,
        safe_state: String,
        #[serde(default)]
        feedback_tag: Option<String>,
    },
    Safety {
        permissives: Vec<String>,
        #[serde(default = "default_true")]
        latched_trip: bool,
    },
}

impl BehaviorConfig {
    pub const fn family(&self) -> BehaviorFamily {
        match self {
            Self::Endpoint { .. } => BehaviorFamily::Endpoint,
            Self::ServiceHost { .. } => BehaviorFamily::ServiceHost,
            Self::PolicyService { .. } => BehaviorFamily::PolicyService,
            Self::TransparentLink { .. } => BehaviorFamily::TransparentLink,
            Self::ImpairedLink { .. } => BehaviorFamily::ImpairedLink,
            Self::EthernetSwitch { .. } => BehaviorFamily::EthernetSwitch,
            Self::Router { .. } => BehaviorFamily::Router,
            Self::NatRouter { .. } => BehaviorFamily::NatRouter,
            Self::StatefulFirewall { .. } => BehaviorFamily::StatefulFirewall,
            Self::ApplicationGateway { .. } => BehaviorFamily::ApplicationGateway,
            Self::WirelessBridge { .. } => BehaviorFamily::WirelessBridge,
            Self::PassiveMonitor { .. } => BehaviorFamily::PassiveMonitor,
            Self::Voice { .. } => BehaviorFamily::Voice,
            Self::ComputeHost { .. } => BehaviorFamily::ComputeHost,
            Self::VirtualController { .. } => BehaviorFamily::VirtualController,
            Self::OperatorInterface { .. } => BehaviorFamily::OperatorInterface,
            Self::RemoteIo { .. } => BehaviorFamily::RemoteIo,
            Self::FieldSensor { .. } => BehaviorFamily::FieldSensor,
            Self::FieldActuator { .. } => BehaviorFamily::FieldActuator,
            Self::Safety { .. } => BehaviorFamily::Safety,
        }
    }

    fn validate(&self, appliance_id: &str) -> Result<(), ConfigError> {
        match self {
            Self::ServiceHost {
                accepted_services, ..
            }
            | Self::Voice {
                accepted_services, ..
            } if accepted_services.is_empty() => Err(ConfigError::new(format!(
                "appliance {appliance_id} must accept at least one service"
            ))),
            Self::PolicyService {
                accepted_services,
                decision_inputs,
            } if accepted_services.is_empty() || decision_inputs.is_empty() => {
                Err(ConfigError::new(format!(
                    "policy appliance {appliance_id} requires services and decision inputs"
                )))
            }
            Self::EthernetSwitch { vlans, .. } if vlans.is_empty() => Err(ConfigError::new(
                format!("switch {appliance_id} must define at least one VLAN"),
            )),
            Self::NatRouter {
                inside_interfaces,
                outside_interfaces,
                ..
            } if inside_interfaces.is_empty() || outside_interfaces.is_empty() => {
                Err(ConfigError::new(format!(
                    "NAT router {appliance_id} requires inside and outside interfaces"
                )))
            }
            Self::StatefulFirewall { default_action, .. }
                if *default_action != PolicyAction::Deny =>
            {
                Err(ConfigError::new(format!(
                    "firewall {appliance_id} must explicitly default deny"
                )))
            }
            Self::ApplicationGateway {
                listeners,
                upstreams,
                ..
            } if listeners.is_empty() || upstreams.is_empty() => Err(ConfigError::new(format!(
                "application gateway {appliance_id} requires listeners and upstreams"
            ))),
            Self::PassiveMonitor { inline: true, .. } => Err(ConfigError::new(format!(
                "passive sensor {appliance_id} cannot be configured inline"
            ))),
            Self::ComputeHost { workloads, .. } if workloads.is_empty() => Err(ConfigError::new(
                format!("compute host {appliance_id} requires at least one workload"),
            )),
            Self::VirtualController {
                scan_interval_ms, ..
            } if *scan_interval_ms == 0 => Err(ConfigError::new(format!(
                "virtual controller {appliance_id} requires a non-zero scan interval"
            ))),
            Self::FieldSensor {
                minimum, maximum, ..
            } if minimum >= maximum => Err(ConfigError::new(format!(
                "sensor {appliance_id} requires minimum below maximum"
            ))),
            Self::Safety { permissives, .. } if permissives.is_empty() => Err(ConfigError::new(
                format!("safety interface {appliance_id} requires at least one permissive"),
            )),
            _ => Ok(()),
        }
    }

    fn services(&self) -> Vec<String> {
        match self {
            Self::Endpoint {
                accepted_services, ..
            }
            | Self::ServiceHost {
                accepted_services, ..
            }
            | Self::PolicyService {
                accepted_services, ..
            }
            | Self::Voice {
                accepted_services, ..
            }
            | Self::ComputeHost {
                accepted_services, ..
            } => accepted_services.clone(),
            Self::ApplicationGateway { listeners, .. } => listeners
                .iter()
                .map(|listener| format!("{}:{}", listener.protocol, listener.port))
                .collect(),
            _ => Vec::new(),
        }
    }

    fn facts(&self) -> Vec<String> {
        match self {
            Self::Endpoint {
                respond_to_icmp, ..
            }
            | Self::ServiceHost {
                respond_to_icmp, ..
            } => vec![format!("ICMP response: {respond_to_icmp}")],
            Self::PolicyService {
                decision_inputs, ..
            } => vec![format!("Decision inputs: {}", decision_inputs.join(", "))],
            Self::TransparentLink { operational } => {
                vec![format!("Operational: {operational}")]
            }
            Self::ImpairedLink {
                operational,
                delay_ms,
                loss_every,
            } => vec![
                format!("Operational: {operational}"),
                format!("Modeled delay: {delay_ms} ms"),
                format!(
                    "Deterministic loss: {}",
                    loss_every.map_or_else(|| "disabled".into(), |value| format!("1/{value}"))
                ),
            ],
            Self::EthernetSwitch {
                vlans,
                management_vlan,
                spanning_tree,
            } => vec![
                format!("VLANs: {}", join_numbers(vlans)),
                format!(
                    "Management VLAN: {}",
                    management_vlan.map_or_else(|| "none".into(), |value| value.to_string())
                ),
                format!("Spanning tree: {spanning_tree}"),
            ],
            Self::Router { routes, forwarding } => vec![
                format!("Forwarding: {forwarding}"),
                format!("Routes: {}", routes.len()),
            ],
            Self::NatRouter {
                routes,
                inside_interfaces,
                outside_interfaces,
                translations,
            } => vec![
                format!("Routes: {}", routes.len()),
                format!("Inside: {}", inside_interfaces.join(", ")),
                format!("Outside: {}", outside_interfaces.join(", ")),
                format!("Static translations: {}", translations.len()),
            ],
            Self::StatefulFirewall {
                stateful,
                default_action,
                rules,
            } => vec![
                format!("Stateful inspection: {stateful}"),
                format!("Default action: {default_action}"),
                format!("Policy rules: {}", rules.len()),
            ],
            Self::ApplicationGateway {
                listeners,
                upstreams,
                max_request_bytes,
            } => vec![
                format!("Listeners: {}", listeners.len()),
                format!("Upstreams: {}", upstreams.join(", ")),
                format!(
                    "Request limit: {}",
                    max_request_bytes.map_or_else(|| "not set".into(), |value| value.to_string())
                ),
            ],
            Self::WirelessBridge {
                ssid,
                client_vlan,
                client_isolation,
            } => vec![
                format!("SSID: {ssid}"),
                format!("Client VLAN: {client_vlan}"),
                format!("Client isolation: {client_isolation}"),
            ],
            Self::PassiveMonitor {
                capture_sources,
                inline,
            } => vec![
                format!("Capture sources: {}", capture_sources.join(", ")),
                format!("Inline: {inline}"),
            ],
            Self::Voice {
                extension,
                call_controller,
                ..
            } => vec![
                format!(
                    "Extension: {}",
                    extension.clone().unwrap_or_else(|| "not assigned".into())
                ),
                format!(
                    "Call controller: {}",
                    call_controller
                        .clone()
                        .unwrap_or_else(|| "not assigned".into())
                ),
            ],
            Self::ComputeHost { workloads, .. } => {
                vec![format!("Workloads: {}", workloads.join(", "))]
            }
            Self::VirtualController {
                scan_interval_ms,
                program_ref,
                io_binding,
            } => vec![
                format!("Scan interval: {scan_interval_ms} ms"),
                format!("Program: {program_ref}"),
                format!("I/O binding: {io_binding}"),
            ],
            Self::OperatorInterface {
                controller,
                permissions,
            } => vec![
                format!("Controller: {controller}"),
                format!("Permissions: {}", permissions.join(", ")),
            ],
            Self::RemoteIo {
                controller,
                channels,
            } => vec![
                format!("Controller: {controller}"),
                format!("Channels: {}", channels.join(", ")),
            ],
            Self::FieldSensor {
                signal_tag,
                unit,
                minimum,
                maximum,
            } => vec![
                format!("Signal: {signal_tag}"),
                format!("Range: {minimum} to {maximum} {unit}"),
            ],
            Self::FieldActuator {
                command_tag,
                safe_state,
                feedback_tag,
            } => vec![
                format!("Command: {command_tag}"),
                format!("Safe state: {safe_state}"),
                format!(
                    "Feedback: {}",
                    feedback_tag.clone().unwrap_or_else(|| "none".into())
                ),
            ],
            Self::Safety {
                permissives,
                latched_trip,
            } => vec![
                format!("Permissives: {}", permissives.join(", ")),
                format!("Trip latched: {latched_trip}"),
            ],
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteConfig {
    pub destination: String,
    pub next_hop: Option<String>,
    pub interface: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NatTranslationConfig {
    pub public_address: String,
    pub private_address: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyAction {
    Permit,
    Deny,
}

impl Display for PolicyAction {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Permit => formatter.write_str("permit"),
            Self::Deny => formatter.write_str("deny"),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyRuleConfig {
    pub name: String,
    pub action: PolicyAction,
    pub source: String,
    pub destination: String,
    pub service: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListenerConfig {
    pub protocol: String,
    pub port: u16,
}

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

        Ok(Self { appliances })
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendApplianceCatalog {
    pub schema_version: &'static str,
    pub generation_status: &'static str,
    pub generated_by: &'static str,
    pub appliance_source_root: &'static str,
    pub connection_source_root: &'static str,
    pub appliances: Vec<FrontendAppliance>,
    pub node_index: BTreeMap<String, Vec<String>>,
    pub connections: Vec<FrontendConnection>,
    pub appliance_connection_index: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendAppliance {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub behavior_family: String,
    pub site: String,
    pub environment: String,
    pub zone: String,
    pub role: String,
    pub summary: String,
    pub lifecycle: String,
    pub tags: Vec<String>,
    pub source_path: String,
    pub source_yaml: String,
    pub revision: String,
    pub addresses: Vec<String>,
    pub interface_count: usize,
    pub interfaces: Vec<FrontendInterface>,
    pub services: Vec<String>,
    pub behavior_facts: Vec<String>,
}

impl From<&LoadedAppliance> for FrontendAppliance {
    fn from(loaded: &LoadedAppliance) -> Self {
        let config = &loaded.config;
        Self {
            id: config.id.clone(),
            label: config.label.clone(),
            kind: config.kind.to_string(),
            behavior_family: config.behavior_family().to_string(),
            site: config.site.clone(),
            environment: config.environment.clone(),
            zone: config.zone.clone(),
            role: config.role.clone(),
            summary: config.summary.clone(),
            lifecycle: config.lifecycle.to_string(),
            tags: config.tags.clone(),
            source_path: loaded.source_path.clone(),
            source_yaml: loaded.source_yaml.clone(),
            revision: loaded.revision(),
            addresses: config
                .interfaces
                .iter()
                .flat_map(|interface| interface.addresses.iter().cloned())
                .collect(),
            interface_count: config.interfaces.len(),
            interfaces: config
                .interfaces
                .iter()
                .map(FrontendInterface::from)
                .collect(),
            services: config.behavior.services(),
            behavior_facts: config.behavior.facts(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendInterface {
    pub id: String,
    pub hardware: String,
    pub mode: String,
    pub administrative_state: String,
    pub initial_operational_state: String,
    pub speed_mbps: u64,
    pub duplex: String,
    pub mtu: u32,
    pub addresses: Vec<String>,
    pub vlans: Vec<u16>,
    pub supported_media: Vec<String>,
}

impl From<&InterfaceConfig> for FrontendInterface {
    fn from(interface: &InterfaceConfig) -> Self {
        Self {
            id: interface.id.clone(),
            hardware: interface.hardware.to_string(),
            mode: interface.mode.to_string(),
            administrative_state: interface.state.administrative.to_string(),
            initial_operational_state: interface.state.initial_operational.to_string(),
            speed_mbps: interface.settings.speed_mbps,
            duplex: interface.settings.duplex.to_string(),
            mtu: interface.settings.mtu,
            addresses: interface.addresses.clone(),
            vlans: interface.vlans.clone(),
            supported_media: interface
                .hardware
                .supported_media()
                .iter()
                .map(ToString::to_string)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigError {
    message: String,
}

impl ConfigError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn with_path(self, path: &Path) -> Self {
        Self::new(format!("{}: {}", path.display(), self.message))
    }
}

impl Display for ConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ConfigError {}

fn collect_yaml_paths(root: &Path, paths: &mut Vec<PathBuf>) -> Result<(), ConfigError> {
    let entries = fs::read_dir(root)
        .map_err(|error| ConfigError::new(format!("cannot read {}: {error}", root.display())))?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            ConfigError::new(format!(
                "cannot read entry under {}: {error}",
                root.display()
            ))
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_yaml_paths(&path, paths)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("yaml") {
            paths.push(path);
        }
    }
    Ok(())
}

fn deserialize_component_kind<'de, D>(deserializer: D) -> Result<ComponentKind, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    ComponentKind::from_str(&value).map_err(serde::de::Error::custom)
}

fn require_text(field: &str, value: &str) -> Result<(), ConfigError> {
    if value.trim().is_empty() {
        Err(ConfigError::new(format!("{field} cannot be empty")))
    } else {
        Ok(())
    }
}

const fn default_true() -> bool {
    true
}

fn join_numbers(values: &[u16]) -> String {
    values
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn source_revision(source: &str) -> String {
    let digest = Sha256::digest(source.as_bytes());
    let mut revision = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut revision, "{byte:02x}").expect("writing to a String cannot fail");
    }
    revision
}

#[cfg(test)]
mod tests {
    use super::*;

    const SWITCH: &str = r#"
schema_version: 0.3.0
id: test-switch-01
label: Test SW-01
kind: layer-2-switch
site: test
environment: test-lan
zone: access
role: Test access switch
summary: Valid parser fixture
render:
  - view: test/test-lan
    node: switch
interfaces:
  - id: ethernet-1
    hardware: ethernet-rj45
    state:
      administrative: up
      initial_operational: up
    settings:
      speed_mbps: 1000
      duplex: full
      mtu: 1500
    mode: access
    addresses: []
    vlans: [10]
behavior:
  family: ethernet-switch
  vlans: [10]
  management_vlan: 10
  spanning_tree: true
"#;

    #[test]
    fn appliance_dispatches_to_its_typed_behavior() {
        let config = ApplianceConfig::from_yaml(SWITCH).expect("valid switch");
        assert_eq!(config.kind, ComponentKind::Layer2Switch);
        assert_eq!(config.behavior_family(), BehaviorFamily::EthernetSwitch);
    }

    #[test]
    fn kind_and_behavior_must_match() {
        let invalid = SWITCH.replace(
            "family: ethernet-switch\n  vlans: [10]\n  management_vlan: 10\n  spanning_tree: true",
            "family: endpoint\n  accepted_services: []\n  respond_to_icmp: true",
        );
        let error = ApplianceConfig::from_yaml(&invalid).expect_err("must reject mismatch");
        assert!(error.to_string().contains("requires behavior family"));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let invalid = SWITCH.replace(
            "summary: Valid parser fixture",
            "summary: Valid parser fixture\nmystery: true",
        );
        assert!(ApplianceConfig::from_yaml(&invalid).is_err());
    }

    #[test]
    fn firewall_must_default_deny() {
        let firewall = SWITCH
            .replace("kind: layer-2-switch", "kind: firewall")
            .replace(
                "family: ethernet-switch\n  vlans: [10]\n  management_vlan: 10\n  spanning_tree: true",
                "family: stateful-firewall\n  stateful: true\n  default_action: permit\n  rules: []",
            );
        let error = ApplianceConfig::from_yaml(&firewall).expect_err("must reject permit default");
        assert!(error.to_string().contains("default deny"));
    }
}
