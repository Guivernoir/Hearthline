use super::BehaviorConfig;
use crate::appliance::{application_gateway_facts, join_numbers};

impl BehaviorConfig {
    pub(in crate::appliance) fn facts(&self) -> Vec<String> {
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
                zones,
                routes,
                rules,
            } => vec![
                format!("Stateful inspection: {stateful}"),
                format!("Default action: {default_action}"),
                format!("Security zones: {}", zones.len()),
                format!("Routes: {}", routes.len()),
                format!("Policy rules: {}", rules.len()),
            ],
            Self::ApplicationGateway {
                listeners,
                allowed_hosts,
                allowed_methods,
                inspection_rules,
                upstreams,
                routes,
                max_request_bytes,
            } => application_gateway_facts(
                listeners,
                allowed_hosts,
                allowed_methods,
                inspection_rules,
                upstreams,
                routes,
                *max_request_bytes,
            ),
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
                signal_tags,
                command_tags,
                control_station,
                supervisory_profile,
                ..
            } => {
                let mut facts = vec![
                    format!("Controller: {controller}"),
                    format!(
                        "Station: {}",
                        control_station.as_ref().map_or_else(
                            || "generic operator interface".into(),
                            |station| format!("{} / {}", station.station_type, station.target),
                        )
                    ),
                    format!("Permissions: {}", permissions.join(", ")),
                    format!(
                        "Signal scope: {}",
                        if signal_tags.is_empty() {
                            "all area signals".into()
                        } else {
                            signal_tags.join(", ")
                        }
                    ),
                    format!("Command tags: {}", command_tags.join(", ")),
                ];
                if let Some(profile) = supervisory_profile {
                    facts.push(format!("Supervisory model: {}", profile.model_id));
                    facts.push(format!("Asset instances: {}", profile.assets.len()));
                    facts.push(format!(
                        "Deployment nodes: {}",
                        profile.deployment_nodes.len()
                    ));
                }
                facts
            }
            Self::RemoteIo {
                controller,
                channels,
                control_cabinet,
            } => {
                let mut facts = vec![
                    format!("Controller: {controller}"),
                    format!("Channels: {}", channels.join(", ")),
                ];
                if let Some(cabinet) = control_cabinet {
                    facts.push(format!("Cabinet target: {}", cabinet.target));
                    facts.push(format!("Cabinet modules: {}", cabinet.modules.join(", ")));
                }
                facts
            }
            Self::FieldSensor {
                signal_tag,
                unit,
                minimum,
                maximum,
                initial_value,
            } => vec![
                format!("Signal: {signal_tag}"),
                format!("Range: {minimum} to {maximum} {unit}"),
                format!(
                    "Initial value: {}",
                    initial_value.map_or_else(|| "not set".into(), |value| value.to_string())
                ),
            ],
            Self::FieldActuator {
                command_tag,
                safe_state,
                feedback_tag,
                states,
                motion_profile,
                utility_cabinet,
            } => {
                let mut facts = vec![
                    format!("Command: {command_tag}"),
                    format!("Safe state: {safe_state}"),
                    format!("States: {}", states.join(", ")),
                    format!(
                        "Feedback: {}",
                        feedback_tag.clone().unwrap_or_else(|| "none".into())
                    ),
                ];
                if let Some(profile) = motion_profile {
                    facts.push(format!("Motion program: {}", profile.program_ref));
                    facts.push(format!(
                        "Motion limits: {} mm/s linear, {} deg/s joint",
                        profile.max_linear_speed_mm_s, profile.max_joint_speed_deg_s
                    ));
                    facts.push(format!(
                        "Motion group: {}",
                        profile.architecture.motion_group
                    ));
                    facts.push(format!("User frames: {}", profile.frames.len()));
                    facts.push(format!("Mould handoffs: {}", profile.handoffs.len()));
                }
                if let Some(cabinet) = utility_cabinet {
                    facts.push(format!("Embedded utility target: {}", cabinet.target));
                    facts.push(format!("Utility circuits: {}", cabinet.circuits.len()));
                }
                facts
            }
            Self::Safety {
                permissives,
                latched_trip,
                initially_permissive,
            } => vec![
                format!("Permissives: {}", permissives.join(", ")),
                format!("Initially permissive: {}", initially_permissive.join(", ")),
                format!("Trip latched: {latched_trip}"),
            ],
        }
    }
}
