use hearthline_model::{BehaviorFamily, ComponentKind};

use crate::SaturationBehavior;

/// Conservative preallocation contract for one host runtime component handle.
///
/// The simulation adapter statically verifies its concrete tagged handle
/// against this value, avoiding an upward project-to-sim dependency.
pub const RUNTIME_COMPONENT_HANDLE_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityProfile {
    AddressedHost,
    ServiceHost,
    TransparentMedia,
    EthernetBridge,
    RoutedNetwork,
    PolicyGateway,
    WirelessBridge,
    PassiveObservation,
    Compute,
    ProcessControl,
    OperatorControl,
    FieldIo,
    Safety,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BehaviorImplementation {
    Endpoint,
    ServiceNode,
    PolicyService,
    Link,
    ImpairedLink,
    LearningSwitch,
    Router,
    NatRouter,
    StatefulFirewall,
    ReverseProxyWaf,
    WirelessAccessPoint,
    PassiveSensor,
    VoiceEndpoint,
    ComputeHost,
    VirtualPlc,
    OperatorInterface,
    RemoteIo,
    FieldSensor,
    Actuator,
    SafetyInterface,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotContract {
    pub schema: &'static str,
    pub version: u16,
    pub includes_authoritative_state: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApplianceFamilyContract {
    pub family: BehaviorFamily,
    pub implementation: BehaviorImplementation,
    pub capabilities: CapabilityProfile,
    pub schema_mapping: &'static str,
    pub saturation: SaturationBehavior,
    pub snapshot: SnapshotContract,
    pub runtime_object_bytes: usize,
    pub baseline: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApplianceContract {
    pub kind: ComponentKind,
    pub family: BehaviorFamily,
    pub implementation: BehaviorImplementation,
    pub capabilities: CapabilityProfile,
    pub schema_mapping: &'static str,
    pub saturation: SaturationBehavior,
    pub snapshot: SnapshotContract,
    pub runtime_object_bytes: usize,
    pub baseline: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderedRoleContract {
    pub rendered_role: &'static str,
    pub kind: ComponentKind,
}

pub fn appliance_contracts() -> impl Iterator<Item = ApplianceContract> {
    ComponentKind::ALL.into_iter().map(|kind| {
        let contract = appliance_family_contract(kind.behavior_family());
        ApplianceContract {
            kind,
            family: contract.family,
            implementation: contract.implementation,
            capabilities: contract.capabilities,
            schema_mapping: contract.schema_mapping,
            saturation: contract.saturation,
            snapshot: contract.snapshot,
            runtime_object_bytes: contract.runtime_object_bytes,
            baseline: contract.baseline,
        }
    })
}

pub fn appliance_family_contract(family: BehaviorFamily) -> &'static ApplianceFamilyContract {
    FAMILY_CONTRACTS
        .iter()
        .find(|contract| contract.family == family)
        .expect("every behavior family has a compile-time contract")
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

const SNAPSHOT: SnapshotContract = SnapshotContract {
    schema: "component-state",
    version: 1,
    includes_authoritative_state: true,
};

const fn contract(
    family: BehaviorFamily,
    implementation: BehaviorImplementation,
    capabilities: CapabilityProfile,
    schema_mapping: &'static str,
    saturation: SaturationBehavior,
    baseline: &'static str,
) -> ApplianceFamilyContract {
    ApplianceFamilyContract {
        family,
        implementation,
        capabilities,
        schema_mapping,
        saturation,
        snapshot: SNAPSHOT,
        runtime_object_bytes: implementation_size(implementation),
        baseline,
    }
}

const fn maximum(left: usize, right: usize) -> usize {
    if left > right { left } else { right }
}

const fn implementation_size(implementation: BehaviorImplementation) -> usize {
    match implementation {
        BehaviorImplementation::Endpoint => core::mem::size_of::<crate::ServiceNode>(),
        BehaviorImplementation::ServiceNode => maximum(
            core::mem::size_of::<crate::ServiceNode>(),
            core::mem::size_of::<crate::DnsServer>(),
        ),
        BehaviorImplementation::PolicyService
        | BehaviorImplementation::VoiceEndpoint
        | BehaviorImplementation::ComputeHost => maximum(
            core::mem::size_of::<crate::ServiceNode>(),
            core::mem::size_of::<crate::LinkAppliance>(),
        ),
        BehaviorImplementation::Link | BehaviorImplementation::ImpairedLink => {
            core::mem::size_of::<crate::LinkAppliance>()
        }
        BehaviorImplementation::LearningSwitch => core::mem::size_of::<crate::LearningSwitch>(),
        BehaviorImplementation::Router => maximum(
            core::mem::size_of::<crate::Router>(),
            core::mem::size_of::<crate::Layer3Switch>(),
        ),
        BehaviorImplementation::NatRouter => core::mem::size_of::<crate::NatRouter>(),
        BehaviorImplementation::StatefulFirewall => core::mem::size_of::<crate::StatefulFirewall>(),
        BehaviorImplementation::ReverseProxyWaf => core::mem::size_of::<crate::ReverseProxyWaf>(),
        BehaviorImplementation::WirelessAccessPoint => {
            core::mem::size_of::<crate::WirelessAccessPoint>()
        }
        BehaviorImplementation::PassiveSensor => core::mem::size_of::<crate::PassiveSensor>(),
        BehaviorImplementation::VirtualPlc => core::mem::size_of::<crate::VirtualPlc>(),
        BehaviorImplementation::OperatorInterface => {
            core::mem::size_of::<crate::OperatorInterface>()
        }
        BehaviorImplementation::RemoteIo => core::mem::size_of::<crate::RemoteIo>(),
        BehaviorImplementation::FieldSensor => core::mem::size_of::<crate::FieldSensor>(),
        BehaviorImplementation::Actuator => core::mem::size_of::<crate::Actuator>(),
        BehaviorImplementation::SafetyInterface => core::mem::size_of::<crate::SafetyInterface>(),
    }
}

pub const FAMILY_CONTRACTS: [ApplianceFamilyContract; 20] = [
    contract(
        BehaviorFamily::Endpoint,
        BehaviorImplementation::Endpoint,
        CapabilityProfile::AddressedHost,
        "behavior.endpoint",
        SaturationBehavior::DropWithDiagnostic,
        "local delivery, service acceptance, and ICMP response",
    ),
    contract(
        BehaviorFamily::ServiceHost,
        BehaviorImplementation::ServiceNode,
        CapabilityProfile::ServiceHost,
        "behavior.service-host",
        SaturationBehavior::DropWithDiagnostic,
        "explicit service acceptance and deterministic response",
    ),
    contract(
        BehaviorFamily::PolicyService,
        BehaviorImplementation::PolicyService,
        CapabilityProfile::ServiceHost,
        "behavior.policy-service",
        SaturationBehavior::RejectOperation,
        "explicit identity and policy service acceptance",
    ),
    contract(
        BehaviorFamily::TransparentLink,
        BehaviorImplementation::Link,
        CapabilityProfile::TransparentMedia,
        "behavior.transparent-link",
        SaturationBehavior::DropWithDiagnostic,
        "bidirectional forwarding and operational failure",
    ),
    contract(
        BehaviorFamily::ImpairedLink,
        BehaviorImplementation::ImpairedLink,
        CapabilityProfile::TransparentMedia,
        "behavior.impaired-link",
        SaturationBehavior::DropWithDiagnostic,
        "forwarding, delay, deterministic loss, and failure",
    ),
    contract(
        BehaviorFamily::EthernetSwitch,
        BehaviorImplementation::LearningSwitch,
        CapabilityProfile::EthernetBridge,
        "behavior.ethernet-switch",
        SaturationBehavior::DropWithDiagnostic,
        "VLAN admission, MAC learning, unicast, and flooding",
    ),
    contract(
        BehaviorFamily::Router,
        BehaviorImplementation::Router,
        CapabilityProfile::RoutedNetwork,
        "behavior.router",
        SaturationBehavior::DropWithDiagnostic,
        "longest-prefix forwarding, TTL, and no-route diagnostics",
    ),
    contract(
        BehaviorFamily::NatRouter,
        BehaviorImplementation::NatRouter,
        CapabilityProfile::RoutedNetwork,
        "behavior.nat-router",
        SaturationBehavior::RejectOperation,
        "routing, PAT state, reverse translation, and static NAT",
    ),
    contract(
        BehaviorFamily::StatefulFirewall,
        BehaviorImplementation::StatefulFirewall,
        CapabilityProfile::PolicyGateway,
        "behavior.stateful-firewall",
        SaturationBehavior::RejectOperation,
        "ordered policy, connection state, routing, and default deny",
    ),
    contract(
        BehaviorFamily::ApplicationGateway,
        BehaviorImplementation::ReverseProxyWaf,
        CapabilityProfile::PolicyGateway,
        "behavior.application-gateway",
        SaturationBehavior::RejectOperation,
        "host, method, size, TLS, and upstream policy",
    ),
    contract(
        BehaviorFamily::WirelessBridge,
        BehaviorImplementation::WirelessAccessPoint,
        CapabilityProfile::WirelessBridge,
        "behavior.wireless-bridge",
        SaturationBehavior::DropWithDiagnostic,
        "association policy and bridged client forwarding",
    ),
    contract(
        BehaviorFamily::PassiveMonitor,
        BehaviorImplementation::PassiveSensor,
        CapabilityProfile::PassiveObservation,
        "behavior.passive-monitor",
        SaturationBehavior::DropWithDiagnostic,
        "out-of-band observation without forwarding dependency",
    ),
    contract(
        BehaviorFamily::Voice,
        BehaviorImplementation::VoiceEndpoint,
        CapabilityProfile::AddressedHost,
        "behavior.voice",
        SaturationBehavior::DropWithDiagnostic,
        "voice-signaling service acceptance and availability",
    ),
    contract(
        BehaviorFamily::ComputeHost,
        BehaviorImplementation::ComputeHost,
        CapabilityProfile::Compute,
        "behavior.compute-host",
        SaturationBehavior::RejectOperation,
        "management service acceptance and host availability",
    ),
    contract(
        BehaviorFamily::VirtualController,
        BehaviorImplementation::VirtualPlc,
        CapabilityProfile::ProcessControl,
        "behavior.virtual-controller",
        SaturationBehavior::StopSimulation,
        "deterministic scan and explicit rule evaluation",
    ),
    contract(
        BehaviorFamily::OperatorInterface,
        BehaviorImplementation::OperatorInterface,
        CapabilityProfile::OperatorControl,
        "behavior.operator-interface",
        SaturationBehavior::RejectOperation,
        "authorized command submission and state observation",
    ),
    contract(
        BehaviorFamily::RemoteIo,
        BehaviorImplementation::RemoteIo,
        CapabilityProfile::FieldIo,
        "behavior.remote-io",
        SaturationBehavior::StopSimulation,
        "input sampling, output application, and channel validation",
    ),
    contract(
        BehaviorFamily::FieldSensor,
        BehaviorImplementation::FieldSensor,
        CapabilityProfile::FieldIo,
        "behavior.field-sensor",
        SaturationBehavior::DropWithDiagnostic,
        "scaled process measurement and quality state",
    ),
    contract(
        BehaviorFamily::FieldActuator,
        BehaviorImplementation::Actuator,
        CapabilityProfile::FieldIo,
        "behavior.field-actuator",
        SaturationBehavior::StopSimulation,
        "commanded state, failure, and safe-state handling",
    ),
    contract(
        BehaviorFamily::Safety,
        BehaviorImplementation::SafetyInterface,
        CapabilityProfile::Safety,
        "behavior.safety",
        SaturationBehavior::StopSimulation,
        "permissive evaluation, latched trip, and authorized reset",
    ),
];
