use hearthline_model::{BehaviorFamily, ComponentKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApplianceContract {
    pub kind: ComponentKind,
    pub family: BehaviorFamily,
    pub baseline: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderedRoleContract {
    pub rendered_role: &'static str,
    pub kind: ComponentKind,
}

pub fn appliance_contracts() -> impl Iterator<Item = ApplianceContract> {
    ComponentKind::ALL
        .into_iter()
        .map(|kind| ApplianceContract {
            kind,
            family: kind.behavior_family(),
            baseline: baseline_for(kind.behavior_family()),
        })
}

/// Maps every appliance role currently rendered by Svelte to one Rust kind.
///
/// Entries ending in `*` cover repeated area-prefixed assets or grouped
/// instances. Sites, zones, environment handoffs, and process-area boundaries
/// are intentionally excluded because they are topology concepts, not
/// appliances.
pub const RENDERED_ROLE_CONTRACTS: [RenderedRoleContract; 45] = [
    RenderedRoleContract {
        rendered_role: "Customer PC-*",
        kind: ComponentKind::Workstation,
    },
    RenderedRoleContract {
        rendered_role: "Customer SW-01",
        kind: ComponentKind::Layer2Switch,
    },
    RenderedRoleContract {
        rendered_role: "Customer RTR-01",
        kind: ComponentKind::NatRouter,
    },
    RenderedRoleContract {
        rendered_role: "* INET-CPE-*",
        kind: ComponentKind::TransparentCpe,
    },
    RenderedRoleContract {
        rendered_role: "WAN-*",
        kind: ComponentKind::WanCircuit,
    },
    RenderedRoleContract {
        rendered_role: "ISP EDGE-RTR-*",
        kind: ComponentKind::Router,
    },
    RenderedRoleContract {
        rendered_role: "ISP-DNS-*",
        kind: ComponentKind::DnsServer,
    },
    RenderedRoleContract {
        rendered_role: "Business EDGE-RTR-*",
        kind: ComponentKind::NatRouter,
    },
    RenderedRoleContract {
        rendered_role: "Business FRW-*",
        kind: ComponentKind::Firewall,
    },
    RenderedRoleContract {
        rendered_role: "Business IT-DMZ-SW-*",
        kind: ComponentKind::Layer2Switch,
    },
    RenderedRoleContract {
        rendered_role: "Business WEB-GW-*",
        kind: ComponentKind::ReverseProxyWaf,
    },
    RenderedRoleContract {
        rendered_role: "Business IT-CORE-SW-*",
        kind: ComponentKind::Layer3Switch,
    },
    RenderedRoleContract {
        rendered_role: "Business IT-*-SW-*",
        kind: ComponentKind::Layer2Switch,
    },
    RenderedRoleContract {
        rendered_role: "Internal Service Clusters",
        kind: ComponentKind::ServiceCluster,
    },
    RenderedRoleContract {
        rendered_role: "Business IT-VOICE-GW-*",
        kind: ComponentKind::VoiceGateway,
    },
    RenderedRoleContract {
        rendered_role: "Business IT-USR-PC-*",
        kind: ComponentKind::Workstation,
    },
    RenderedRoleContract {
        rendered_role: "Business IT-PHONE-*",
        kind: ComponentKind::IpPhone,
    },
    RenderedRoleContract {
        rendered_role: "Business IT-PRN-*",
        kind: ComponentKind::Printer,
    },
    RenderedRoleContract {
        rendered_role: "Guest Wireless",
        kind: ComponentKind::WirelessAccessPoint,
    },
    RenderedRoleContract {
        rendered_role: "Guest unmanaged client",
        kind: ComponentKind::Workstation,
    },
    RenderedRoleContract {
        rendered_role: "Business IT-PAW-*",
        kind: ComponentKind::PrivilegedWorkstation,
    },
    RenderedRoleContract {
        rendered_role: "Business IT-NET-CTRL-*",
        kind: ComponentKind::NetworkController,
    },
    RenderedRoleContract {
        rendered_role: "Identity & Policy Services",
        kind: ComponentKind::IdentityPolicyService,
    },
    RenderedRoleContract {
        rendered_role: "Central NOC / Central SOC",
        kind: ComponentKind::OperationsConsole,
    },
    RenderedRoleContract {
        rendered_role: "Process Analytics Platform",
        kind: ComponentKind::AnalyticsPlatform,
    },
    RenderedRoleContract {
        rendered_role: "Process Analysis Workstations",
        kind: ComponentKind::Workstation,
    },
    RenderedRoleContract {
        rendered_role: "Change Approval & Staging",
        kind: ComponentKind::ChangeStagingService,
    },
    RenderedRoleContract {
        rendered_role: "* Conduit",
        kind: ComponentKind::EncryptedConduit,
    },
    RenderedRoleContract {
        rendered_role: "OT-DMZ-SW-*",
        kind: ComponentKind::Layer2Switch,
    },
    RenderedRoleContract {
        rendered_role: "OT-DMZ-JUMP-SRV-*",
        kind: ComponentKind::JumpHost,
    },
    RenderedRoleContract {
        rendered_role: "OT-DMZ-HIST-REPLICA-*",
        kind: ComponentKind::HistorianReplica,
    },
    RenderedRoleContract {
        rendered_role: "OT-DMZ-XFER-SRV-*",
        kind: ComponentKind::FileTransferGateway,
    },
    RenderedRoleContract {
        rendered_role: "OT-DMZ-MON-*",
        kind: ComponentKind::MonitoringCollector,
    },
    RenderedRoleContract {
        rendered_role: "OT-SENSOR-*",
        kind: ComponentKind::PassiveNetworkSensor,
    },
    RenderedRoleContract {
        rendered_role: "OT Operations",
        kind: ComponentKind::ServiceCluster,
    },
    RenderedRoleContract {
        rendered_role: "OT-vPLC-HOST-*",
        kind: ComponentKind::VirtualizationHost,
    },
    RenderedRoleContract {
        rendered_role: "AREA-*-SW-*",
        kind: ComponentKind::Layer2Switch,
    },
    RenderedRoleContract {
        rendered_role: "AREA-*-vPLC-*",
        kind: ComponentKind::VirtualPlc,
    },
    RenderedRoleContract {
        rendered_role: "AREA-*-SCADA-*",
        kind: ComponentKind::ScadaWorkstation,
    },
    RenderedRoleContract {
        rendered_role: "AREA-*-HMI-*",
        kind: ComponentKind::Hmi,
    },
    RenderedRoleContract {
        rendered_role: "AREA-*-RIO-*",
        kind: ComponentKind::RemoteIo,
    },
    RenderedRoleContract {
        rendered_role: "AREA-*-sensor",
        kind: ComponentKind::FieldSensor,
    },
    RenderedRoleContract {
        rendered_role: "AREA-*-actuator",
        kind: ComponentKind::FieldActuator,
    },
    RenderedRoleContract {
        rendered_role: "AREA-*-ROBOT-CTRL-*",
        kind: ComponentKind::RobotController,
    },
    RenderedRoleContract {
        rendered_role: "AREA-*-safety/permissive",
        kind: ComponentKind::SafetyInterface,
    },
];

const fn baseline_for(family: BehaviorFamily) -> &'static str {
    match family {
        BehaviorFamily::Endpoint => "local delivery, service acceptance, and ICMP response",
        BehaviorFamily::ServiceHost => "explicit service acceptance and deterministic response",
        BehaviorFamily::PolicyService => "explicit identity and policy service acceptance",
        BehaviorFamily::TransparentLink => "bidirectional forwarding and operational failure",
        BehaviorFamily::ImpairedLink => "forwarding, delay, deterministic loss, and failure",
        BehaviorFamily::EthernetSwitch => "VLAN admission, MAC learning, unicast, and flooding",
        BehaviorFamily::Router => "longest-prefix forwarding, TTL, and no-route diagnostics",
        BehaviorFamily::NatRouter => "routing, PAT state, reverse translation, and static NAT",
        BehaviorFamily::StatefulFirewall => {
            "ordered policy, connection state, routing, and default deny"
        }
        BehaviorFamily::ApplicationGateway => "host, method, size, TLS, and upstream policy",
        BehaviorFamily::WirelessBridge => "association policy and bridged client forwarding",
        BehaviorFamily::PassiveMonitor => "out-of-band observation without forwarding dependency",
        BehaviorFamily::Voice => "voice-signaling service acceptance and availability",
        BehaviorFamily::ComputeHost => "management service acceptance and host availability",
        BehaviorFamily::VirtualController => "deterministic scan and explicit rule evaluation",
        BehaviorFamily::OperatorInterface => "authorized command submission and state observation",
        BehaviorFamily::RemoteIo => "input sampling, output application, and channel validation",
        BehaviorFamily::FieldSensor => "scaled process measurement and quality state",
        BehaviorFamily::FieldActuator => "commanded state, failure, and safe-state handling",
        BehaviorFamily::Safety => "permissive evaluation, latched trip, and authorized reset",
    }
}
