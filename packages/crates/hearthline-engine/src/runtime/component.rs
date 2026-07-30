use core::fmt::{self, Display, Formatter};
use core::net::Ipv4Addr;
use heapless::Vec as FixedList;

use hearthline_model::{
    ComponentId, ComponentKind, EthernetFrame, PortId, ProcessCommand, ProcessEvent, ProcessSignal,
    ServiceKind, SignalValue, Text,
};

pub const EFFECT_CAPACITY: usize = 32;
pub type EffectList = FixedList<Effect, EFFECT_CAPACITY>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkIngress {
    pub port: PortId,
    pub frame: EthernetFrame,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SimulationEvent {
    Network(NetworkIngress),
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
    InvalidIngress(PortId),
    VlanNotAllowed(u16),
    NoRoute(Ipv4Addr),
    TtlExpired,
    PolicyDenied { rule: Option<Text<64>> },
    NoTranslation,
    UnsupportedProtocol,
    ServiceUnavailable(ServiceKind),
    NotAddressedToComponent,
    ApplicationRejected(Text<96>),
    SafetyTrip(Text<96>),
    LinkLoss,
    QueueLimit,
}

impl Display for DropReason {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ComponentDown => formatter.write_str("component is not operational"),
            Self::PortDown(port) => write!(formatter, "port {port} is down"),
            Self::InvalidIngress(port) => write!(formatter, "unknown ingress port {port}"),
            Self::VlanNotAllowed(vlan) => write!(formatter, "VLAN {vlan} is not allowed"),
            Self::NoRoute(destination) => write!(formatter, "no route to {destination}"),
            Self::TtlExpired => formatter.write_str("IPv4 TTL expired"),
            Self::PolicyDenied { rule: Some(rule) } => {
                write!(formatter, "denied by policy rule {rule}")
            }
            Self::PolicyDenied { rule: None } => formatter.write_str("denied by default policy"),
            Self::NoTranslation => formatter.write_str("no matching NAT state or static mapping"),
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
            Self::QueueLimit => formatter.write_str("simulation event limit reached"),
        }
    }
}

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
