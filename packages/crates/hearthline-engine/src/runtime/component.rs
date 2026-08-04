use core::fmt::{self, Display, Formatter};
use core::net::Ipv4Addr;
use heapless::Vec as FixedList;

use hearthline_model::{
    ComponentId, ComponentKind, EthernetFrame, Ipv4Packet, PortId, ProcessCommand, ProcessEvent,
    ProcessSignal, ServiceKind, SignalValue, Text,
};

use crate::MediaDropReason;

pub const EFFECT_CAPACITY: usize = 32;
pub type EffectList = FixedList<Effect, EFFECT_CAPACITY>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkIngress {
    pub port: PortId,
    pub frame: EthernetFrame,
    pub received_at_us: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ipv4Egress {
    pub packet: Ipv4Packet,
    pub wire_len_bytes: u16,
    pub sent_at_us: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FirewallHaControl {
    HeartbeatTick {
        at_us: u64,
    },
    EvaluatePeer {
        at_us: u64,
        peer_failure_confirmed: bool,
    },
    ClearReplicatedSessions {
        at_us: u64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum SimulationEvent {
    Network(NetworkIngress),
    Ipv4Egress(Ipv4Egress),
    FirewallHa(FirewallHaControl),
    Process(ProcessEvent),
    SetOperational(bool),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProcessEffect {
    Signal(ProcessSignal),
    Command(ProcessCommand),
    Output {
        tag: Text<64>,
        value: SignalValue,
    },
    Alarm {
        code: Text<64>,
        active: bool,
        message: Text<192>,
    },
    State {
        name: Text<64>,
        value: Text<128>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DropReason {
    ComponentDown,
    PortDown(PortId),
    SpanningTreeDiscarding {
        port: PortId,
        vlan: u16,
    },
    LinkAggregationDiscarding(PortId),
    FirewallStandby,
    FirewallHaDomainMismatch,
    InvalidIngress(PortId),
    InvalidEthernetFrame,
    InterfaceMtuExceeded {
        port: PortId,
        wire_bytes: u16,
        maximum: u32,
    },
    InvalidSourceMac(hearthline_model::MacAddress),
    L2DestinationMismatch {
        expected: hearthline_model::MacAddress,
        actual: hearthline_model::MacAddress,
    },
    InvalidArp,
    InvalidTcpState,
    InvalidSourceIp(Ipv4Addr),
    VlanNotAllowed(u16),
    NoRoute(Ipv4Addr),
    TtlExpired,
    PolicyDenied {
        rule: Option<Text<64>>,
    },
    NoTranslation,
    NatTableFull,
    NeighborQueueFull,
    NextHopOffLink {
        next_hop: Ipv4Addr,
        egress: PortId,
    },
    NoInterfaceAddress(PortId),
    UnsupportedProtocol,
    ServiceUnavailable(ServiceKind),
    NotAddressedToComponent,
    ApplicationRejected(Text<96>),
    SafetyTrip(Text<96>),
    LinkLoss,
    Media(MediaDropReason),
    QueueLimit,
}

impl Display for DropReason {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ComponentDown => formatter.write_str("component is not operational"),
            Self::PortDown(port) => write!(formatter, "port {port} is down"),
            Self::SpanningTreeDiscarding { port, vlan } => {
                write!(
                    formatter,
                    "spanning tree discards VLAN {vlan} on port {port}"
                )
            }
            Self::LinkAggregationDiscarding(port) => {
                write!(formatter, "link aggregation discards port {port}")
            }
            Self::FirewallStandby => formatter.write_str("firewall HA member is in standby"),
            Self::FirewallHaDomainMismatch => {
                formatter.write_str("firewall HA control message belongs to another domain")
            }
            Self::InvalidIngress(port) => write!(formatter, "unknown ingress port {port}"),
            Self::InvalidEthernetFrame => formatter.write_str("invalid Ethernet frame"),
            Self::InterfaceMtuExceeded {
                port,
                wire_bytes,
                maximum,
            } => write!(
                formatter,
                "frame length {wire_bytes} exceeds interface {port} limit of {maximum} bytes"
            ),
            Self::InvalidSourceMac(mac) => write!(formatter, "invalid source MAC {mac}"),
            Self::L2DestinationMismatch { expected, actual } => {
                write!(
                    formatter,
                    "frame destination {actual} does not match interface MAC {expected}"
                )
            }
            Self::InvalidArp => formatter.write_str("invalid ARP message"),
            Self::InvalidTcpState => {
                formatter.write_str("TCP packet does not belong to a valid session")
            }
            Self::InvalidSourceIp(address) => {
                write!(
                    formatter,
                    "IPv4 source {address} is not assigned to the egress interface"
                )
            }
            Self::VlanNotAllowed(vlan) => write!(formatter, "VLAN {vlan} is not allowed"),
            Self::NoRoute(destination) => write!(formatter, "no route to {destination}"),
            Self::TtlExpired => formatter.write_str("IPv4 TTL expired"),
            Self::PolicyDenied { rule: Some(rule) } => {
                write!(formatter, "denied by policy rule {rule}")
            }
            Self::PolicyDenied { rule: None } => formatter.write_str("denied by default policy"),
            Self::NoTranslation => formatter.write_str("no matching NAT state or static mapping"),
            Self::NatTableFull => formatter.write_str("NAT translation table is full"),
            Self::NeighborQueueFull => {
                formatter.write_str("pending neighbor-resolution queue is full")
            }
            Self::NextHopOffLink { next_hop, egress } => {
                write!(formatter, "next hop {next_hop} is not on-link for {egress}")
            }
            Self::NoInterfaceAddress(port) => {
                write!(formatter, "interface {port} has no IPv4 address")
            }
            Self::UnsupportedProtocol => formatter.write_str("protocol is not modeled"),
            Self::ServiceUnavailable(service) => {
                write!(formatter, "service {service:?} is unavailable")
            }
            Self::NotAddressedToComponent => {
                formatter.write_str("packet is not addressed to this component")
            }
            Self::ApplicationRejected(reason) => {
                write!(formatter, "application rejected: {reason}")
            }
            Self::SafetyTrip(cause) => write!(formatter, "safety trip: {cause}"),
            Self::LinkLoss => formatter.write_str("frame lost by modeled link impairment"),
            Self::Media(reason) => write!(formatter, "media transit failed: {reason}"),
            Self::QueueLimit => formatter.write_str("simulation event limit reached"),
        }
    }
}

// Network effects remain inline because the deterministic runtime cannot allocate.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Transmit {
        egress: PortId,
        next_hop: Option<Ipv4Addr>,
        frame: EthernetFrame,
        delay_ms: u64,
    },
    Deliver {
        service: ServiceKind,
        detail: Text<192>,
    },
    ApplicationForward {
        service: ServiceKind,
        target: ComponentId,
        detail: Text<192>,
    },
    MediaTransit {
        connection: ComponentId,
        destination_component: ComponentId,
        destination_port: PortId,
        wire_bytes: u16,
        queue_delay_us: u64,
        serialization_us: u64,
        propagation_us: u64,
        arrival_us: u64,
    },
    Drop(DropReason),
    Observe {
        detail: Text<192>,
    },
    Process(ProcessEffect),
}

pub trait SimulatedComponent {
    fn id(&self) -> &ComponentId;
    fn kind(&self) -> ComponentKind;
    fn has_port(&self, port: &PortId) -> bool;
    fn handle(&mut self, event: SimulationEvent) -> EffectList;
}

pub fn single_effect(effect: Effect) -> EffectList {
    let mut effects = EffectList::new();
    effects
        .push(effect)
        .expect("single effect must fit runtime capacity");
    effects
}
