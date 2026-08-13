use hearthline_model::BehaviorFamily;
use serde::Deserialize;

mod cabinet;
mod facts;
mod operator;
mod robot;
mod services;
mod supervisory;

pub use cabinet::{
    MouldControlCabinetConfig, MouldUtilityCabinetConfig, MouldUtilityCircuitConfig,
    UtilityMediumConfig,
};

pub use operator::{
    OperatorControlMode, OperatorModeSelectorConfig, OperatorParameterConfig, OperatorRecipeConfig,
    OperatorStationConfig, OperatorStationType,
};
pub use robot::{
    RobotArchitectureConfig, RobotFrameConfig, RobotHandoffConfig, RobotMotionProfileConfig,
    RobotPayloadConfig, RobotPoseConfig, RobotTaughtPositionConfig, RobotToolConfig,
    RobotWorkspaceConfig,
};
pub use supervisory::{
    SupervisoryAssetConfig, SupervisoryDeploymentNodeConfig, SupervisoryHistoryConfig,
    SupervisoryIdentityConfig, SupervisoryNodeRoleConfig, SupervisoryNodeStateConfig,
    SupervisoryProfileConfig, SupervisoryRepositoryConfig, SupervisoryRoleConfig,
    SupervisoryTemplateConfig,
};

use super::{
    ApplicationUpstreamConfig, ConfigError, DnsRecordConfig, FirewallZoneConfig,
    HttpInspectionRuleConfig, HttpMethodConfig, HttpSiteConfig, ListenerConfig,
    NatTranslationConfig, PolicyAction, PolicyRuleConfig, RouteConfig, default_true,
    validate_application_gateway, validate_dns_records,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "family", rename_all = "kebab-case", deny_unknown_fields)]
pub enum BehaviorConfig {
    Endpoint {
        #[serde(default)]
        accepted_services: Vec<String>,
        #[serde(default = "default_true")]
        respond_to_icmp: bool,
        #[serde(default)]
        hostname: Option<String>,
        #[serde(default)]
        dns_servers: Vec<String>,
    },
    ServiceHost {
        accepted_services: Vec<String>,
        #[serde(default = "default_true")]
        respond_to_icmp: bool,
        #[serde(default)]
        dns_records: Vec<DnsRecordConfig>,
        #[serde(default)]
        http_site: Option<HttpSiteConfig>,
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
        zones: Vec<FirewallZoneConfig>,
        #[serde(default)]
        routes: Vec<RouteConfig>,
        #[serde(default)]
        rules: Vec<PolicyRuleConfig>,
    },
    ApplicationGateway {
        listeners: Vec<ListenerConfig>,
        allowed_hosts: Vec<String>,
        allowed_methods: Vec<HttpMethodConfig>,
        inspection_rules: Vec<HttpInspectionRuleConfig>,
        upstreams: Vec<ApplicationUpstreamConfig>,
        #[serde(default)]
        routes: Vec<RouteConfig>,
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
        #[serde(default)]
        signal_tags: Vec<String>,
        #[serde(default)]
        command_tags: Vec<String>,
        #[serde(default)]
        safety_components: Vec<String>,
        #[serde(default)]
        control_station: Option<OperatorStationConfig>,
        #[serde(default)]
        parameters: Vec<OperatorParameterConfig>,
        #[serde(default)]
        recipes: Vec<OperatorRecipeConfig>,
        #[serde(default)]
        active_recipe: Option<String>,
        #[serde(default)]
        supervisory_profile: Option<SupervisoryProfileConfig>,
    },
    RemoteIo {
        controller: String,
        channels: Vec<String>,
        #[serde(default)]
        control_cabinet: Option<MouldControlCabinetConfig>,
    },
    FieldSensor {
        signal_tag: String,
        unit: String,
        minimum: f64,
        maximum: f64,
        #[serde(default)]
        initial_value: Option<f64>,
    },
    FieldActuator {
        command_tag: String,
        safe_state: String,
        #[serde(default)]
        feedback_tag: Option<String>,
        #[serde(default)]
        states: Vec<String>,
        #[serde(default)]
        motion_profile: Option<RobotMotionProfileConfig>,
        #[serde(default)]
        utility_cabinet: Option<MouldUtilityCabinetConfig>,
    },
    Safety {
        permissives: Vec<String>,
        #[serde(default = "default_true")]
        latched_trip: bool,
        #[serde(default)]
        initially_permissive: Vec<String>,
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

    pub(crate) const fn responds_to_icmp(&self) -> bool {
        match self {
            Self::Endpoint {
                respond_to_icmp, ..
            }
            | Self::ServiceHost {
                respond_to_icmp, ..
            } => *respond_to_icmp,
            _ => false,
        }
    }

    pub(super) fn validate(&self, appliance_id: &str) -> Result<(), ConfigError> {
        crate::hmi::validate_behavior(self, appliance_id)?;
        if let Self::ServiceHost { dns_records, .. } = self {
            validate_dns_records(appliance_id, dns_records)?;
        }
        if let Self::ServiceHost {
            accepted_services,
            http_site: Some(site),
            ..
        } = self
        {
            if !accepted_services.iter().any(|service| service == "https") {
                return Err(ConfigError::new(format!(
                    "HTTP site on {appliance_id} requires the https service"
                )));
            }
            site.validate(appliance_id)?;
        }
        if let Self::ApplicationGateway {
            listeners,
            allowed_hosts,
            allowed_methods,
            inspection_rules,
            upstreams,
            ..
        } = self
        {
            validate_application_gateway(
                appliance_id,
                listeners,
                allowed_hosts,
                allowed_methods,
                upstreams,
                inspection_rules,
            )?;
            for upstream in upstreams {
                upstream.validate(appliance_id)?;
            }
        }
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
            Self::ImpairedLink {
                loss_every: Some(0),
                ..
            } => Err(ConfigError::new(format!(
                "impaired link {appliance_id} loss interval must be non-zero"
            ))),
            Self::StatefulFirewall { default_action, .. }
                if *default_action != PolicyAction::Deny =>
            {
                Err(ConfigError::new(format!(
                    "firewall {appliance_id} must explicitly default deny"
                )))
            }
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

    pub(super) fn dns_records(&self) -> &[DnsRecordConfig] {
        match self {
            Self::ServiceHost { dns_records, .. } => dns_records,
            _ => &[],
        }
    }
}
