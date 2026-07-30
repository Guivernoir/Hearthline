use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

/// Error returned when a stable model identifier is empty or malformed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentifierError {
    value: String,
}

impl IdentifierError {
    fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl Display for IdentifierError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "identifier '{}' must contain only lowercase ASCII letters, digits, and hyphens",
            self.value
        )
    }
}

impl Error for IdentifierError {}

fn validate_identifier(value: &str) -> Result<(), IdentifierError> {
    let valid = !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');

    if valid {
        Ok(())
    } else {
        Err(IdentifierError::new(value))
    }
}

/// Repository-wide stable component identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ComponentId(String);

impl ComponentId {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        let value = value.into();
        validate_identifier(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ComponentId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Stable port or interface identifier scoped to one component.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PortId(String);

impl PortId {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        let value = value.into();
        validate_identifier(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for PortId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Reusable behavior implementation assigned to a rendered appliance kind.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BehaviorFamily {
    Endpoint,
    ServiceHost,
    PolicyService,
    TransparentLink,
    ImpairedLink,
    EthernetSwitch,
    Router,
    NatRouter,
    StatefulFirewall,
    ApplicationGateway,
    WirelessBridge,
    PassiveMonitor,
    Voice,
    ComputeHost,
    VirtualController,
    OperatorInterface,
    RemoteIo,
    FieldSensor,
    FieldActuator,
    Safety,
}

impl Display for BehaviorFamily {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Endpoint => "endpoint",
            Self::ServiceHost => "service-host",
            Self::PolicyService => "policy-service",
            Self::TransparentLink => "transparent-link",
            Self::ImpairedLink => "impaired-link",
            Self::EthernetSwitch => "ethernet-switch",
            Self::Router => "router",
            Self::NatRouter => "nat-router",
            Self::StatefulFirewall => "stateful-firewall",
            Self::ApplicationGateway => "application-gateway",
            Self::WirelessBridge => "wireless-bridge",
            Self::PassiveMonitor => "passive-monitor",
            Self::Voice => "voice",
            Self::ComputeHost => "compute-host",
            Self::VirtualController => "virtual-controller",
            Self::OperatorInterface => "operator-interface",
            Self::RemoteIo => "remote-io",
            Self::FieldSensor => "field-sensor",
            Self::FieldActuator => "field-actuator",
            Self::Safety => "safety",
        };
        formatter.write_str(name)
    }
}

/// All appliance categories currently required by the rendered architecture.
///
/// Diagram-only concepts such as sites, zones, handoffs, and trust boundaries
/// are not appliances. WAN and encrypted conduits are included because their
/// availability, delay, and loss behavior affects scenarios.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ComponentKind {
    Workstation,
    PrivilegedWorkstation,
    EngineeringWorkstation,
    Printer,
    IpPhone,
    WirelessAccessPoint,
    Layer2Switch,
    Layer3Switch,
    Router,
    NatRouter,
    TransparentCpe,
    WanCircuit,
    EncryptedConduit,
    Firewall,
    DnsServer,
    ReverseProxyWaf,
    ServiceCluster,
    IdentityPolicyService,
    NetworkController,
    MonitoringCollector,
    PassiveNetworkSensor,
    JumpHost,
    HistorianReplica,
    FileTransferGateway,
    VoiceGateway,
    AnalyticsPlatform,
    OperationsConsole,
    ChangeStagingService,
    VirtualizationHost,
    VirtualPlc,
    Hmi,
    RemoteIo,
    FieldSensor,
    FieldActuator,
    SafetyInterface,
}

impl ComponentKind {
    pub const ALL: [Self; 35] = [
        Self::Workstation,
        Self::PrivilegedWorkstation,
        Self::EngineeringWorkstation,
        Self::Printer,
        Self::IpPhone,
        Self::WirelessAccessPoint,
        Self::Layer2Switch,
        Self::Layer3Switch,
        Self::Router,
        Self::NatRouter,
        Self::TransparentCpe,
        Self::WanCircuit,
        Self::EncryptedConduit,
        Self::Firewall,
        Self::DnsServer,
        Self::ReverseProxyWaf,
        Self::ServiceCluster,
        Self::IdentityPolicyService,
        Self::NetworkController,
        Self::MonitoringCollector,
        Self::PassiveNetworkSensor,
        Self::JumpHost,
        Self::HistorianReplica,
        Self::FileTransferGateway,
        Self::VoiceGateway,
        Self::AnalyticsPlatform,
        Self::OperationsConsole,
        Self::ChangeStagingService,
        Self::VirtualizationHost,
        Self::VirtualPlc,
        Self::Hmi,
        Self::RemoteIo,
        Self::FieldSensor,
        Self::FieldActuator,
        Self::SafetyInterface,
    ];

    pub const fn behavior_family(self) -> BehaviorFamily {
        match self {
            Self::Workstation
            | Self::PrivilegedWorkstation
            | Self::EngineeringWorkstation
            | Self::OperationsConsole => BehaviorFamily::Endpoint,
            Self::Printer
            | Self::DnsServer
            | Self::ServiceCluster
            | Self::NetworkController
            | Self::MonitoringCollector
            | Self::JumpHost
            | Self::HistorianReplica
            | Self::FileTransferGateway
            | Self::AnalyticsPlatform
            | Self::ChangeStagingService => BehaviorFamily::ServiceHost,
            Self::IdentityPolicyService => BehaviorFamily::PolicyService,
            Self::IpPhone | Self::VoiceGateway => BehaviorFamily::Voice,
            Self::WirelessAccessPoint => BehaviorFamily::WirelessBridge,
            Self::Layer2Switch => BehaviorFamily::EthernetSwitch,
            Self::Layer3Switch | Self::Router => BehaviorFamily::Router,
            Self::NatRouter => BehaviorFamily::NatRouter,
            Self::TransparentCpe => BehaviorFamily::TransparentLink,
            Self::WanCircuit => BehaviorFamily::ImpairedLink,
            Self::EncryptedConduit => BehaviorFamily::TransparentLink,
            Self::Firewall => BehaviorFamily::StatefulFirewall,
            Self::ReverseProxyWaf => BehaviorFamily::ApplicationGateway,
            Self::PassiveNetworkSensor => BehaviorFamily::PassiveMonitor,
            Self::VirtualizationHost => BehaviorFamily::ComputeHost,
            Self::VirtualPlc => BehaviorFamily::VirtualController,
            Self::Hmi => BehaviorFamily::OperatorInterface,
            Self::RemoteIo => BehaviorFamily::RemoteIo,
            Self::FieldSensor => BehaviorFamily::FieldSensor,
            Self::FieldActuator => BehaviorFamily::FieldActuator,
            Self::SafetyInterface => BehaviorFamily::Safety,
        }
    }
}

impl Display for ComponentKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Workstation => "workstation",
            Self::PrivilegedWorkstation => "privileged-workstation",
            Self::EngineeringWorkstation => "engineering-workstation",
            Self::Printer => "printer",
            Self::IpPhone => "ip-phone",
            Self::WirelessAccessPoint => "wireless-access-point",
            Self::Layer2Switch => "layer-2-switch",
            Self::Layer3Switch => "layer-3-switch",
            Self::Router => "router",
            Self::NatRouter => "nat-router",
            Self::TransparentCpe => "transparent-cpe",
            Self::WanCircuit => "wan-circuit",
            Self::EncryptedConduit => "encrypted-conduit",
            Self::Firewall => "firewall",
            Self::DnsServer => "dns-server",
            Self::ReverseProxyWaf => "reverse-proxy-waf",
            Self::ServiceCluster => "service-cluster",
            Self::IdentityPolicyService => "identity-policy-service",
            Self::NetworkController => "network-controller",
            Self::MonitoringCollector => "monitoring-collector",
            Self::PassiveNetworkSensor => "passive-network-sensor",
            Self::JumpHost => "jump-host",
            Self::HistorianReplica => "historian-replica",
            Self::FileTransferGateway => "file-transfer-gateway",
            Self::VoiceGateway => "voice-gateway",
            Self::AnalyticsPlatform => "analytics-platform",
            Self::OperationsConsole => "operations-console",
            Self::ChangeStagingService => "change-staging-service",
            Self::VirtualizationHost => "virtualization-host",
            Self::VirtualPlc => "virtual-plc",
            Self::Hmi => "hmi",
            Self::RemoteIo => "remote-io",
            Self::FieldSensor => "field-sensor",
            Self::FieldActuator => "field-actuator",
            Self::SafetyInterface => "safety-interface",
        };
        formatter.write_str(name)
    }
}

impl FromStr for ComponentKind {
    type Err = ComponentKindParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let kind = match value {
            "workstation" => Self::Workstation,
            "privileged-workstation" => Self::PrivilegedWorkstation,
            "engineering-workstation" => Self::EngineeringWorkstation,
            "printer" => Self::Printer,
            "ip-phone" => Self::IpPhone,
            "wireless-access-point" => Self::WirelessAccessPoint,
            "layer-2-switch" => Self::Layer2Switch,
            "layer-3-switch" => Self::Layer3Switch,
            "router" => Self::Router,
            "nat-router" => Self::NatRouter,
            "transparent-cpe" => Self::TransparentCpe,
            "wan-circuit" => Self::WanCircuit,
            "encrypted-conduit" => Self::EncryptedConduit,
            "firewall" => Self::Firewall,
            "dns-server" => Self::DnsServer,
            "reverse-proxy-waf" => Self::ReverseProxyWaf,
            "service-cluster" => Self::ServiceCluster,
            "identity-policy-service" => Self::IdentityPolicyService,
            "network-controller" => Self::NetworkController,
            "monitoring-collector" => Self::MonitoringCollector,
            "passive-network-sensor" => Self::PassiveNetworkSensor,
            "jump-host" => Self::JumpHost,
            "historian-replica" => Self::HistorianReplica,
            "file-transfer-gateway" => Self::FileTransferGateway,
            "voice-gateway" => Self::VoiceGateway,
            "analytics-platform" => Self::AnalyticsPlatform,
            "operations-console" => Self::OperationsConsole,
            "change-staging-service" => Self::ChangeStagingService,
            "virtualization-host" => Self::VirtualizationHost,
            "virtual-plc" => Self::VirtualPlc,
            "hmi" => Self::Hmi,
            "remote-io" => Self::RemoteIo,
            "field-sensor" => Self::FieldSensor,
            "field-actuator" => Self::FieldActuator,
            "safety-interface" => Self::SafetyInterface,
            _ => return Err(ComponentKindParseError(value.to_owned())),
        };
        Ok(kind)
    }
}

/// Error returned when a configuration names an unsupported appliance kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentKindParseError(String);

impl Display for ComponentKindParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "unsupported component kind '{}'", self.0)
    }
}

impl Error for ComponentKindParseError {}

/// Application or infrastructure service exposed by an endpoint.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ServiceKind {
    Dns,
    Dhcp,
    Http,
    Https,
    Ssh,
    Rdp,
    Snmp,
    Syslog,
    Ntp,
    Pki,
    Identity,
    PolicyDecision,
    FileTransfer,
    HistorianReplication,
    Monitoring,
    Analytics,
    VoiceSignaling,
    Printing,
    PlcEngineering,
    IndustrialIo,
    Management,
    Generic,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_stable_and_restricted() {
        assert!(ComponentId::new("business-frw-01a").is_ok());
        assert!(ComponentId::new("Business FRW-01A").is_err());
        assert!(PortId::new("gigabit-ethernet-0-1").is_ok());
        assert!(PortId::new("").is_err());
    }

    #[test]
    fn every_component_kind_has_a_behavior_family() {
        for kind in ComponentKind::ALL {
            assert!(!kind.behavior_family().to_string().is_empty());
            assert_eq!(kind.to_string().parse::<ComponentKind>(), Ok(kind));
        }
    }
}
