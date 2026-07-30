use std::collections::{BTreeMap, BTreeSet};
use std::net::Ipv4Addr;

use hearthline_model::{
    ComponentId, ComponentKind, FlowKey, NetworkPayload, PortId, TransportProtocol,
};

use crate::{DropReason, Effect, Router, RoutingTable, SimulatedComponent, SimulationEvent};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticNat {
    pub public_address: Ipv4Addr,
    pub private_address: Ipv4Addr,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PatFlow {
    flow: FlowKey,
    source_token: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PatMapping {
    internal_address: Ipv4Addr,
    internal_token: u16,
}

#[derive(Clone, Debug)]
pub struct NatRouter {
    id: ComponentId,
    ports: BTreeSet<PortId>,
    inside_ports: BTreeSet<PortId>,
    outside_address: Ipv4Addr,
    routes: RoutingTable,
    static_nat: Vec<StaticNat>,
    outbound_pat: BTreeMap<PatFlow, u16>,
    inbound_pat: BTreeMap<(TransportProtocol, u16), PatMapping>,
    next_pat_token: u16,
    operational: bool,
}

impl NatRouter {
    pub fn new(
        id: ComponentId,
        ports: impl IntoIterator<Item = PortId>,
        inside_ports: impl IntoIterator<Item = PortId>,
        outside_address: Ipv4Addr,
        routes: RoutingTable,
    ) -> Self {
        Self {
            id,
            ports: ports.into_iter().collect(),
            inside_ports: inside_ports.into_iter().collect(),
            outside_address,
            routes,
            static_nat: Vec::new(),
            outbound_pat: BTreeMap::new(),
            inbound_pat: BTreeMap::new(),
            next_pat_token: 49_152,
            operational: true,
        }
    }

    pub fn add_static_nat(&mut self, mapping: StaticNat) {
        self.static_nat.push(mapping);
    }

    pub fn translation_count(&self) -> usize {
        self.inbound_pat.len()
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
                .inbound_pat
                .keys()
                .any(|(_, token)| *token == candidate)
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
    ) -> Result<String, DropReason> {
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
            return Ok(format!(
                "static source NAT {original} -> {}",
                mapping.public_address
            ));
        }

        let Some(internal_token) = packet.transport.source_token() else {
            return Err(DropReason::UnsupportedProtocol);
        };
        let flow = PatFlow {
            flow: packet.flow_key(),
            source_token: internal_token,
        };
        let external_token = if let Some(token) = self.outbound_pat.get(&flow) {
            *token
        } else {
            let token = self.allocate_token().ok_or(DropReason::QueueLimit)?;
            self.outbound_pat.insert(flow, token);
            self.inbound_pat.insert(
                (packet.transport.protocol(), token),
                PatMapping {
                    internal_address: packet.source,
                    internal_token,
                },
            );
            token
        };

        let internal_address = packet.source;
        packet.source = self.outside_address;
        packet.transport.rewrite_source_token(external_token);
        Ok(format!(
            "PAT {internal_address}:{internal_token} -> {}:{external_token}",
            self.outside_address
        ))
    }

    fn translate_inbound(
        &self,
        frame: &mut hearthline_model::EthernetFrame,
    ) -> Result<String, DropReason> {
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
            return Ok(format!(
                "static destination NAT {original} -> {}",
                mapping.private_address
            ));
        }

        if packet.destination != self.outside_address {
            return Ok("routed without destination translation".into());
        }
        let Some(external_token) = packet.transport.destination_token() else {
            return Err(DropReason::NoTranslation);
        };
        let Some(mapping) = self
            .inbound_pat
            .get(&(packet.transport.protocol(), external_token))
        else {
            return Err(DropReason::NoTranslation);
        };

        packet.destination = mapping.internal_address;
        packet
            .transport
            .rewrite_destination_token(mapping.internal_token);
        Ok(format!(
            "reverse PAT {}:{external_token} -> {}:{}",
            self.outside_address, mapping.internal_address, mapping.internal_token
        ))
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
        self.ports.contains(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> Vec<Effect> {
        match event {
            SimulationEvent::Network(mut ingress) => {
                if !self.operational {
                    return vec![Effect::Drop(DropReason::ComponentDown)];
                }
                if !self.ports.contains(&ingress.port) {
                    return vec![Effect::Drop(DropReason::InvalidIngress(ingress.port))];
                }
                let translation = if self.inside_ports.contains(&ingress.port) {
                    self.translate_outbound(&mut ingress.frame)
                } else {
                    self.translate_inbound(&mut ingress.frame)
                };
                let detail = match translation {
                    Ok(detail) => detail,
                    Err(reason) => return vec![Effect::Drop(reason)],
                };
                let mut effects = vec![Effect::Observe { detail }];
                effects.extend(Router::forward(&self.routes, ingress.frame));
                effects
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                vec![Effect::Observe {
                    detail: format!("operational={operational}"),
                }]
            }
            SimulationEvent::Process(_) => vec![Effect::Drop(DropReason::UnsupportedProtocol)],
        }
    }
}

#[cfg(test)]
mod tests {
    use hearthline_model::{
        ApplicationData, EthernetFrame, IcmpMessage, Ipv4Cidr, Ipv4Packet, MacAddress, Route,
        TcpFlags, TcpSegment, Transport, VlanId,
    };

    use super::*;
    use crate::NetworkIngress;

    fn id(value: &str) -> ComponentId {
        ComponentId::new(value).expect("test ID")
    }

    fn port(value: &str) -> PortId {
        PortId::new(value).expect("test port")
    }

    fn tcp_frame(source: Ipv4Addr, destination: Ipv4Addr, source_port: u16) -> EthernetFrame {
        EthernetFrame {
            source: MacAddress::new([0, 1, 2, 3, 4, 5]),
            destination: MacAddress::new([0, 1, 2, 3, 4, 6]),
            vlan: VlanId::new(1).expect("VLAN"),
            payload: NetworkPayload::Ipv4(Ipv4Packet {
                source,
                destination,
                ttl: 64,
                transport: Transport::Tcp(TcpSegment {
                    source_port,
                    destination_port: 443,
                    flags: TcpFlags {
                        syn: true,
                        ..TcpFlags::default()
                    },
                }),
                application: ApplicationData::None,
            }),
        }
    }

    #[test]
    fn pat_creates_state_and_restores_return_destination() {
        let routes = RoutingTable::new(vec![
            Route {
                destination: Ipv4Cidr::new(Ipv4Addr::new(192, 168, 0, 0), 24).expect("inside"),
                egress: port("inside"),
                next_hop: None,
                metric: 0,
            },
            Route {
                destination: Ipv4Cidr::new(Ipv4Addr::UNSPECIFIED, 0).expect("default"),
                egress: port("outside"),
                next_hop: Some(Ipv4Addr::new(203, 0, 113, 1)),
                metric: 10,
            },
        ]);
        let outside = Ipv4Addr::new(203, 0, 113, 2);
        let internal = Ipv4Addr::new(192, 168, 0, 2);
        let remote = Ipv4Addr::new(192, 0, 2, 10);
        let mut nat = NatRouter::new(
            id("customer-rtr-01"),
            [port("inside"), port("outside")],
            [port("inside")],
            outside,
            routes,
        );

        let outbound = nat.handle(SimulationEvent::Network(NetworkIngress {
            port: port("inside"),
            frame: tcp_frame(internal, remote, 50_000),
        }));
        assert_eq!(nat.translation_count(), 1);
        let Effect::Transmit { frame, .. } = &outbound[1] else {
            panic!("expected translated transmission");
        };
        let NetworkPayload::Ipv4(packet) = &frame.payload else {
            panic!("expected IPv4");
        };
        let external_port = packet.transport.source_token().expect("PAT token");
        assert_eq!(packet.source, outside);

        let mut return_frame = tcp_frame(remote, outside, 443);
        let NetworkPayload::Ipv4(return_packet) = &mut return_frame.payload else {
            panic!("expected IPv4");
        };
        return_packet.transport = Transport::Tcp(TcpSegment {
            source_port: 443,
            destination_port: external_port,
            flags: TcpFlags {
                ack: true,
                ..TcpFlags::default()
            },
        });
        let inbound = nat.handle(SimulationEvent::Network(NetworkIngress {
            port: port("outside"),
            frame: return_frame,
        }));
        let Effect::Transmit { frame, egress, .. } = &inbound[1] else {
            panic!("expected reverse transmission");
        };
        let NetworkPayload::Ipv4(packet) = &frame.payload else {
            panic!("expected IPv4");
        };
        assert_eq!(egress, &port("inside"));
        assert_eq!(packet.destination, internal);
        assert_eq!(packet.transport.destination_token(), Some(50_000));
    }

    #[test]
    fn pat_translates_icmp_echo_identifier_in_both_directions() {
        let routes = RoutingTable::new(vec![
            Route {
                destination: Ipv4Cidr::new(Ipv4Addr::new(192, 168, 0, 0), 24).expect("inside"),
                egress: port("inside"),
                next_hop: None,
                metric: 0,
            },
            Route {
                destination: Ipv4Cidr::new(Ipv4Addr::UNSPECIFIED, 0).expect("default"),
                egress: port("outside"),
                next_hop: Some(Ipv4Addr::new(203, 0, 113, 1)),
                metric: 10,
            },
        ]);
        let outside = Ipv4Addr::new(203, 0, 113, 2);
        let internal = Ipv4Addr::new(192, 168, 0, 2);
        let remote = Ipv4Addr::new(198, 51, 100, 50);
        let mut nat = NatRouter::new(
            id("customer-rtr-01"),
            [port("inside"), port("outside")],
            [port("inside")],
            outside,
            routes,
        );
        let echo_frame = |source, destination, message| EthernetFrame {
            source: MacAddress::new([0, 1, 2, 3, 4, 5]),
            destination: MacAddress::new([0, 1, 2, 3, 4, 6]),
            vlan: VlanId::new(1).expect("VLAN"),
            payload: NetworkPayload::Ipv4(Ipv4Packet {
                source,
                destination,
                ttl: 64,
                transport: Transport::Icmp(message),
                application: ApplicationData::None,
            }),
        };

        let outbound = nat.handle(SimulationEvent::Network(NetworkIngress {
            port: port("inside"),
            frame: echo_frame(
                internal,
                remote,
                IcmpMessage::EchoRequest {
                    identifier: 42,
                    sequence: 1,
                },
            ),
        }));
        let Effect::Transmit { frame, .. } = &outbound[1] else {
            panic!("expected translated echo request");
        };
        let NetworkPayload::Ipv4(packet) = &frame.payload else {
            panic!("expected IPv4");
        };
        let external_identifier = packet.transport.source_token().expect("translated ID");
        assert_eq!(packet.source, outside);
        assert_ne!(external_identifier, 42);

        let inbound = nat.handle(SimulationEvent::Network(NetworkIngress {
            port: port("outside"),
            frame: echo_frame(
                remote,
                outside,
                IcmpMessage::EchoReply {
                    identifier: external_identifier,
                    sequence: 1,
                },
            ),
        }));
        let Effect::Transmit { frame, egress, .. } = &inbound[1] else {
            panic!("expected reverse-translated echo reply");
        };
        let NetworkPayload::Ipv4(packet) = &frame.payload else {
            panic!("expected IPv4");
        };
        assert_eq!(egress, &port("inside"));
        assert_eq!(packet.destination, internal);
        assert_eq!(packet.transport.destination_token(), Some(42));
    }
}
