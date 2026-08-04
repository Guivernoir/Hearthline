use core::net::Ipv4Addr;
use core::{fmt, result};
use heapless::Vec as FixedList;

use hearthline_model::{
    ComponentId, ComponentKind, FlowKey, IcmpMessage, NetworkPayload, PortId, Text, Transport,
    TransportProtocol,
};

use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{
    DropReason, Effect, EffectList, RoutedInterface, RoutingTable, SimulatedComponent,
    SimulationEvent,
};

use super::forwarding::{ForwardingPlane, ReceiveOutcome, local_response};

const PAT_CAPACITY: usize = 64;
const TCP_PAT_TIMEOUT_US: u64 = 300_000_000;
const UDP_PAT_TIMEOUT_US: u64 = 60_000_000;
const ICMP_PAT_TIMEOUT_US: u64 = 30_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticNat {
    pub public_address: Ipv4Addr,
    pub private_address: Ipv4Addr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticNatError {
    DuplicatePublicAddress(Ipv4Addr),
    DuplicatePrivateAddress(Ipv4Addr),
    PublicAddressOffLink(Ipv4Addr),
    PrivateAddressUnreachable(Ipv4Addr),
    TableFull,
}

impl fmt::Display for StaticNatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicatePublicAddress(address) => {
                write!(formatter, "duplicate static NAT public address {address}")
            }
            Self::DuplicatePrivateAddress(address) => {
                write!(formatter, "duplicate static NAT private address {address}")
            }
            Self::PublicAddressOffLink(address) => {
                write!(formatter, "static NAT public address {address} is off-link")
            }
            Self::PrivateAddressUnreachable(address) => {
                write!(
                    formatter,
                    "static NAT private address {address} has no inside route"
                )
            }
            Self::TableFull => formatter.write_str("static NAT table exceeds capacity"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PatFlow {
    flow: FlowKey,
    source_token: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PatSession {
    outbound: PatFlow,
    external_token: u16,
    expires_at_us: u64,
}

#[derive(Clone, Debug)]
pub struct NatRouter {
    id: ComponentId,
    plane: ForwardingPlane,
    inside_ports: FixedList<PortId, 16>,
    outside_port: PortId,
    outside_address: Ipv4Addr,
    static_nat: FixedList<StaticNat, 16>,
    pat: FixedList<PatSession, PAT_CAPACITY>,
    next_pat_token: u16,
    operational: bool,
}

impl NatRouter {
    pub fn new(
        id: ComponentId,
        interfaces: impl IntoIterator<Item = RoutedInterface>,
        inside_ports: impl IntoIterator<Item = PortId>,
        outside_address: Ipv4Addr,
        routes: RoutingTable,
    ) -> Self {
        let interfaces: FixedList<RoutedInterface, 16> = collect_fixed(interfaces);
        let inside_ports = collect_fixed(inside_ports);
        assert!(
            inside_ports
                .iter()
                .all(|port| interfaces.iter().any(|interface| interface.id == *port)),
            "every NAT inside port requires a routed interface"
        );
        let outside_port = interfaces
            .iter()
            .find(|interface| {
                !inside_ports.contains(&interface.id) && interface.has_address(outside_address)
            })
            .map(|interface| interface.id.clone())
            .expect("NAT outside address must belong to an outside interface");
        Self {
            id,
            plane: ForwardingPlane::new(interfaces, routes),
            inside_ports,
            outside_port,
            outside_address,
            static_nat: FixedList::new(),
            pat: FixedList::new(),
            next_pat_token: 49_152,
            operational: true,
        }
    }

    pub fn add_static_nat(&mut self, mapping: StaticNat) -> result::Result<(), StaticNatError> {
        if self
            .static_nat
            .iter()
            .any(|candidate| candidate.public_address == mapping.public_address)
        {
            return Err(StaticNatError::DuplicatePublicAddress(
                mapping.public_address,
            ));
        }
        if self
            .static_nat
            .iter()
            .any(|candidate| candidate.private_address == mapping.private_address)
        {
            return Err(StaticNatError::DuplicatePrivateAddress(
                mapping.private_address,
            ));
        }
        if !self
            .plane
            .interface_is_on_link(&self.outside_port, mapping.public_address)
        {
            return Err(StaticNatError::PublicAddressOffLink(mapping.public_address));
        }
        let private_route_is_inside = self
            .plane
            .route(mapping.private_address)
            .is_some_and(|route| self.inside_ports.contains(&route.egress));
        if !private_route_is_inside {
            return Err(StaticNatError::PrivateAddressUnreachable(
                mapping.private_address,
            ));
        }
        if self.static_nat.is_full() {
            return Err(StaticNatError::TableFull);
        }
        self.plane
            .add_proxy_address(self.outside_port.clone(), mapping.public_address)
            .map_err(|_| StaticNatError::TableFull)?;
        self.static_nat
            .push(mapping)
            .map_err(|_| StaticNatError::TableFull)
    }

    pub fn translation_count(&self) -> usize {
        self.pat.len()
    }

    fn expire_pat(&mut self, now_us: u64) {
        let mut index = 0;
        while index < self.pat.len() {
            if now_us >= self.pat[index].expires_at_us {
                self.pat.swap_remove(index);
            } else {
                index += 1;
            }
        }
    }

    fn allocate_token(&mut self) -> Option<u16> {
        let first = self.next_pat_token;
        loop {
            let candidate = self.next_pat_token;
            self.next_pat_token = if candidate == u16::MAX {
                49_152
            } else {
                candidate + 1
            };
            if !self
                .pat
                .iter()
                .any(|session| session.external_token == candidate)
            {
                return Some(candidate);
            }
            if self.next_pat_token == first {
                return None;
            }
        }
    }

    fn translate_outbound(
        &mut self,
        frame: &mut hearthline_model::EthernetFrame,
        now_us: u64,
    ) -> Result<Text<192>, DropReason> {
        let NetworkPayload::Ipv4(packet) = &mut frame.payload else {
            return Err(DropReason::UnsupportedProtocol);
        };

        if let Some(mapping) = self
            .static_nat
            .iter()
            .find(|mapping| mapping.private_address == packet.source)
        {
            let original = packet.source;
            packet.source = mapping.public_address;
            return Ok(runtime_text(format_args!(
                "static source NAT {original} -> {}",
                mapping.public_address
            )));
        }

        let Some(internal_token) = packet.transport.source_token() else {
            return Err(DropReason::UnsupportedProtocol);
        };
        let flow = PatFlow {
            flow: packet.flow_key(),
            source_token: internal_token,
        };
        let external_token =
            if let Some(session) = self.pat.iter_mut().find(|session| session.outbound == flow) {
                session.expires_at_us =
                    now_us.saturating_add(pat_timeout_us(packet.transport.protocol()));
                session.external_token
            } else {
                if self.pat.is_full() {
                    return Err(DropReason::NatTableFull);
                }
                let token = self.allocate_token().ok_or(DropReason::QueueLimit)?;
                self.pat
                    .push(PatSession {
                        outbound: flow,
                        external_token: token,
                        expires_at_us: now_us
                            .saturating_add(pat_timeout_us(packet.transport.protocol())),
                    })
                    .map_err(|_| DropReason::NatTableFull)?;
                token
            };

        let internal_address = packet.source;
        packet.source = self.outside_address;
        packet.transport.rewrite_source_token(external_token);
        Ok(runtime_text(format_args!(
            "PAT {internal_address}:{internal_token} -> {}:{external_token}",
            self.outside_address
        )))
    }

    fn translate_inbound(
        &mut self,
        frame: &mut hearthline_model::EthernetFrame,
        now_us: u64,
    ) -> Result<Text<192>, DropReason> {
        let NetworkPayload::Ipv4(packet) = &mut frame.payload else {
            return Err(DropReason::UnsupportedProtocol);
        };

        if let Some(mapping) = self
            .static_nat
            .iter()
            .find(|mapping| mapping.public_address == packet.destination)
        {
            let original = packet.destination;
            packet.destination = mapping.private_address;
            return Ok(runtime_text(format_args!(
                "static destination NAT {original} -> {}",
                mapping.private_address
            )));
        }

        if packet.destination != self.outside_address {
            return Ok("routed without destination translation".into());
        }
        let Some(external_token) = packet.transport.destination_token() else {
            return Err(DropReason::NoTranslation);
        };
        let Some(mapping) = self.pat.iter_mut().find(|session| {
            session.outbound.flow.protocol == packet.transport.protocol()
                && session.external_token == external_token
                && session.outbound.flow.destination == packet.source
                && session.outbound.flow.destination_port == packet.transport.source_port()
        }) else {
            return Err(DropReason::NoTranslation);
        };

        mapping.expires_at_us = now_us.saturating_add(pat_timeout_us(packet.transport.protocol()));
        packet.destination = mapping.outbound.flow.source;
        packet
            .transport
            .rewrite_destination_token(mapping.outbound.source_token);
        Ok(runtime_text(format_args!(
            "reverse PAT {}:{external_token} -> {}:{}",
            self.outside_address, mapping.outbound.flow.source, mapping.outbound.source_token
        )))
    }
}

impl SimulatedComponent for NatRouter {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::NatRouter
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.plane.has_port(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.operational {
                    return single_effect(Effect::Drop(DropReason::ComponentDown));
                }
                let ingress_port = ingress.port.clone();
                let received_at_us = ingress.received_at_us;
                self.expire_pat(received_at_us);
                let mut frame = match self.plane.receive(ingress) {
                    ReceiveOutcome::Handled(effects) => return effects,
                    ReceiveOutcome::Local {
                        ingress: local_port,
                        frame,
                    } if self.inside_ports.contains(&local_port) => {
                        return local_response(local_port, frame);
                    }
                    ReceiveOutcome::Local { frame, .. } | ReceiveOutcome::Transit { frame, .. } => {
                        frame
                    }
                };
                let translation = if self.inside_ports.contains(&ingress_port) {
                    self.translate_outbound(&mut frame, received_at_us)
                } else {
                    self.translate_inbound(&mut frame, received_at_us)
                };
                let detail = match translation {
                    Ok(detail) => detail,
                    Err(DropReason::NoTranslation)
                        if ingress_port == self.outside_port
                            && addressed_echo_request(&frame, self.outside_address) =>
                    {
                        return local_response(ingress_port, frame);
                    }
                    Err(reason) => return single_effect(Effect::Drop(reason)),
                };
                let mut effects = single_effect(Effect::Observe { detail });
                for effect in self.plane.forward(frame, received_at_us) {
                    effects
                        .push(effect)
                        .expect("combined NAT effects exceed capacity");
                }
                effects
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                single_effect(Effect::Observe {
                    detail: runtime_text(format_args!("operational={operational}")),
                })
            }
            SimulationEvent::Ipv4Egress(_)
            | SimulationEvent::Process(_)
            | SimulationEvent::FirewallHa(_) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
        }
    }
}

const fn pat_timeout_us(protocol: TransportProtocol) -> u64 {
    match protocol {
        TransportProtocol::Tcp => TCP_PAT_TIMEOUT_US,
        TransportProtocol::Udp => UDP_PAT_TIMEOUT_US,
        TransportProtocol::Icmp | TransportProtocol::Other(_) => ICMP_PAT_TIMEOUT_US,
    }
}

fn addressed_echo_request(frame: &hearthline_model::EthernetFrame, address: Ipv4Addr) -> bool {
    matches!(
        &frame.payload,
        NetworkPayload::Ipv4(packet)
            if packet.destination == address
                && matches!(
                    packet.transport,
                    Transport::Icmp(IcmpMessage::EchoRequest { .. })
                )
    )
}
