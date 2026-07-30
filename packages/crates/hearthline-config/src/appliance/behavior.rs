use std::fmt::{self, Display, Formatter};

use hearthline_model::BehaviorFamily;
use serde::Deserialize;

use super::{ConfigError, default_true, join_numbers};

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

    pub(super) fn validate(&self, appliance_id: &str) -> Result<(), ConfigError> {
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

    pub(super) fn services(&self) -> Vec<String> {
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

    pub(super) fn facts(&self) -> Vec<String> {
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
