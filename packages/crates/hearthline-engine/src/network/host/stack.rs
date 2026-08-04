use core::mem;
use core::net::Ipv4Addr;

use heapless::Vec as FixedList;
use hearthline_model::{
    ApplicationData, ArpOperation, ArpPacket, EthernetFrame, IcmpMessage, MacAddress,
    NetworkPayload, PortId, Route, TcpFlags, Transport,
};

use crate::network::forwarding::NeighborCache;
use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{
    DropReason, Effect, EffectList, Ipv4Egress, NetworkIngress, RoutedInterface, RoutingTable,
};

const PENDING_CAPACITY: usize = 16;

#[derive(Clone, Debug)]
struct PendingIpv4 {
    neighbor: Ipv4Addr,
    egress: PortId,
    frame: EthernetFrame,
}

// Keeping effects inline preserves the engine's allocator-free runtime contract.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub(crate) enum EndpointReceive {
    Handled(EffectList),
    Ipv4 {
        interface: RoutedInterface,
        frame: EthernetFrame,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct EndpointStack {
    interfaces: FixedList<RoutedInterface, 16>,
    routes: RoutingTable,
    neighbors: NeighborCache,
    pending: FixedList<PendingIpv4, PENDING_CAPACITY>,
}

impl EndpointStack {
    pub fn new(interfaces: impl IntoIterator<Item = RoutedInterface>) -> Self {
        Self::with_routes(interfaces, None, [])
    }

    pub fn with_default_gateway(
        interfaces: impl IntoIterator<Item = RoutedInterface>,
        default_gateway: Ipv4Addr,
    ) -> Self {
        Self::with_routes(interfaces, Some(default_gateway), [])
    }

    pub fn with_routes(
        interfaces: impl IntoIterator<Item = RoutedInterface>,
        default_gateway: Option<Ipv4Addr>,
        configured_routes: impl IntoIterator<Item = Route>,
    ) -> Self {
        let interfaces = collect_fixed(interfaces);
        assert!(
            !interfaces.is_empty(),
            "network endpoint requires at least one interface"
        );
        let mut routes: FixedList<Route, 16> = FixedList::new();
        for interface in &interfaces {
            for address in &interface.addresses {
                routes
                    .push(Route {
                        destination: address.subnet(),
                        egress: interface.id.clone(),
                        next_hop: None,
                        metric: 0,
                    })
                    .expect("endpoint connected routes exceed capacity");
            }
        }
        if let Some(gateway) = default_gateway {
            let interface = interfaces
                .iter()
                .find(|interface| interface.is_on_link(gateway))
                .expect("default gateway must be on-link");
            routes
                .push(Route {
                    destination: hearthline_model::Ipv4Cidr::new(Ipv4Addr::UNSPECIFIED, 0)
                        .expect("default route"),
                    egress: interface.id.clone(),
                    next_hop: Some(gateway),
                    metric: 10,
                })
                .expect("endpoint route table exceeds capacity");
        }
        for route in configured_routes {
            let interface = interfaces
                .iter()
                .find(|interface| interface.id == route.egress)
                .expect("endpoint route must reference a configured interface");
            if let Some(next_hop) = route.next_hop {
                assert!(
                    interface.is_on_link(next_hop),
                    "endpoint route next hop must be on-link"
                );
            }
            routes
                .push(route)
                .expect("endpoint route table exceeds capacity");
        }
        Self {
            interfaces,
            routes: RoutingTable::new(routes),
            neighbors: NeighborCache::default(),
            pending: FixedList::new(),
        }
    }

    pub fn has_port(&self, port: &PortId) -> bool {
        self.interfaces
            .iter()
            .any(|interface| interface.id == *port)
    }

    pub fn receive(&mut self, ingress: NetworkIngress) -> EndpointReceive {
        let Some(interface) = self
            .interfaces
            .iter()
            .find(|interface| interface.id == ingress.port)
            .cloned()
        else {
            return handled(DropReason::InvalidIngress(ingress.port));
        };
        if !interface.forwarding {
            return handled(DropReason::PortDown(ingress.port));
        }
        if ingress.frame.vlan != interface.vlan {
            return handled(DropReason::VlanNotAllowed(ingress.frame.vlan.get()));
        }
        if !ingress.frame.has_valid_wire_length() {
            return handled(DropReason::InvalidEthernetFrame);
        }
        if !interface.accepts_wire_len(ingress.frame.wire_len_bytes) {
            return handled(DropReason::InterfaceMtuExceeded {
                port: interface.id,
                wire_bytes: ingress.frame.wire_len_bytes,
                maximum: u32::from(interface.mtu)
                    .saturating_add(EthernetFrame::VLAN_OVERHEAD_BYTES),
            });
        }
        if !ingress.frame.source.is_unicast() {
            return handled(DropReason::InvalidSourceMac(ingress.frame.source));
        }

        match ingress.frame.payload {
            NetworkPayload::Arp(packet) => EndpointReceive::Handled(self.handle_arp(
                &interface,
                ingress.frame,
                packet,
                ingress.received_at_us,
            )),
            NetworkPayload::FirewallHa(_) => handled(DropReason::UnsupportedProtocol),
            NetworkPayload::Ipv4(ref packet) => {
                if ingress.frame.destination != interface.mac {
                    return handled(DropReason::L2DestinationMismatch {
                        expected: interface.mac,
                        actual: ingress.frame.destination,
                    });
                }
                if invalid_ipv4_source(packet.source) {
                    return handled(DropReason::InvalidSourceIp(packet.source));
                }
                if !self
                    .interfaces
                    .iter()
                    .any(|candidate| candidate.has_address(packet.destination))
                {
                    return handled(DropReason::NotAddressedToComponent);
                }
                EndpointReceive::Ipv4 {
                    interface,
                    frame: ingress.frame,
                }
            }
        }
    }

    pub fn send(&mut self, egress: Ipv4Egress) -> EffectList {
        if egress.wire_len_bytes < EthernetFrame::MIN_WIRE_LEN_BYTES {
            return single_effect(Effect::Drop(DropReason::InvalidEthernetFrame));
        }
        if egress.packet.ttl == 0 {
            return single_effect(Effect::Drop(DropReason::TtlExpired));
        }
        let Some(route) = self.routes.lookup(egress.packet.destination).cloned() else {
            return single_effect(Effect::Drop(DropReason::NoRoute(egress.packet.destination)));
        };
        let interface = self
            .interfaces
            .iter()
            .find(|interface| interface.id == route.egress)
            .cloned()
            .expect("endpoint route egress must reference an interface");
        if !interface.forwarding {
            return single_effect(Effect::Drop(DropReason::PortDown(interface.id)));
        }
        if !interface.accepts_wire_len(egress.wire_len_bytes) {
            return single_effect(Effect::Drop(DropReason::InterfaceMtuExceeded {
                port: interface.id,
                wire_bytes: egress.wire_len_bytes,
                maximum: u32::from(interface.mtu)
                    .saturating_add(EthernetFrame::VLAN_OVERHEAD_BYTES),
            }));
        }
        if !interface.has_address(egress.packet.source) {
            return single_effect(Effect::Drop(DropReason::InvalidSourceIp(
                egress.packet.source,
            )));
        }
        let neighbor = route.next_hop.unwrap_or(egress.packet.destination);
        if !interface.is_on_link(neighbor) {
            return single_effect(Effect::Drop(DropReason::NextHopOffLink {
                next_hop: neighbor,
                egress: interface.id,
            }));
        }
        let source_ip = egress.packet.source;
        let mut frame = EthernetFrame {
            source: interface.mac,
            destination: MacAddress::new([0; 6]),
            vlan: interface.vlan,
            payload: NetworkPayload::Ipv4(egress.packet),
            wire_len_bytes: egress.wire_len_bytes,
        };
        if let Some(destination_mac) =
            self.neighbors
                .lookup(neighbor, &interface.id, egress.sent_at_us)
        {
            frame.destination = destination_mac;
            return transmit(&interface, neighbor, frame);
        }

        let resolution_pending = self
            .pending
            .iter()
            .any(|pending| pending.neighbor == neighbor && pending.egress == interface.id);
        if self
            .pending
            .push(PendingIpv4 {
                neighbor,
                egress: interface.id.clone(),
                frame,
            })
            .is_err()
        {
            return single_effect(Effect::Drop(DropReason::NeighborQueueFull));
        }
        if resolution_pending {
            return single_effect(Effect::Observe {
                detail: runtime_text(format_args!(
                    "queued originated IPv4 packet pending ARP resolution for {neighbor}"
                )),
            });
        }
        single_effect(Effect::Transmit {
            egress: interface.id.clone(),
            next_hop: Some(neighbor),
            frame: arp_request(&interface, source_ip, neighbor),
            delay_ms: 0,
        })
    }

    fn handle_arp(
        &mut self,
        interface: &RoutedInterface,
        frame: EthernetFrame,
        packet: ArpPacket,
        now_us: u64,
    ) -> EffectList {
        if packet.sender_mac != frame.source || !packet.sender_mac.is_unicast() {
            return single_effect(Effect::Drop(DropReason::InvalidArp));
        }
        if packet.sender_ip != Ipv4Addr::UNSPECIFIED && !interface.is_on_link(packet.sender_ip) {
            return single_effect(Effect::Drop(DropReason::InvalidArp));
        }
        match packet.operation {
            ArpOperation::Request => {
                if packet.target_mac.is_some()
                    || !(frame.destination.is_broadcast() || frame.destination == interface.mac)
                {
                    return single_effect(Effect::Drop(DropReason::InvalidArp));
                }
                if !interface.has_address(packet.target_ip) {
                    return single_effect(Effect::Observe {
                        detail: runtime_text(format_args!(
                            "ignored ARP request for {}",
                            packet.target_ip
                        )),
                    });
                }
                if packet.sender_ip != Ipv4Addr::UNSPECIFIED {
                    self.neighbors.learn(
                        packet.sender_ip,
                        packet.sender_mac,
                        &interface.id,
                        now_us,
                    );
                }
                let mut effects = single_effect(Effect::Transmit {
                    egress: interface.id.clone(),
                    next_hop: Some(packet.sender_ip),
                    frame: arp_reply(interface, packet),
                    delay_ms: 0,
                });
                if packet.sender_ip != Ipv4Addr::UNSPECIFIED {
                    append_effects(
                        &mut effects,
                        self.release_pending(
                            interface,
                            packet.sender_ip,
                            packet.sender_mac,
                            now_us,
                        ),
                    );
                }
                effects
            }
            ArpOperation::Reply => {
                if packet.sender_ip == Ipv4Addr::UNSPECIFIED
                    || packet.target_mac != Some(interface.mac)
                    || frame.destination != interface.mac
                    || !interface.has_address(packet.target_ip)
                {
                    return single_effect(Effect::Drop(DropReason::InvalidArp));
                }
                self.neighbors
                    .learn(packet.sender_ip, packet.sender_mac, &interface.id, now_us);
                let effects =
                    self.release_pending(interface, packet.sender_ip, packet.sender_mac, now_us);
                if effects.is_empty() {
                    single_effect(Effect::Observe {
                        detail: runtime_text(format_args!(
                            "learned ARP neighbor {} as {}",
                            packet.sender_ip, packet.sender_mac
                        )),
                    })
                } else {
                    effects
                }
            }
        }
    }

    fn release_pending(
        &mut self,
        interface: &RoutedInterface,
        neighbor: Ipv4Addr,
        destination_mac: MacAddress,
        now_us: u64,
    ) -> EffectList {
        self.neighbors
            .learn(neighbor, destination_mac, &interface.id, now_us);
        let mut effects = EffectList::new();
        let mut index = 0;
        while index < self.pending.len() {
            if self.pending[index].neighbor == neighbor
                && self.pending[index].egress == interface.id
            {
                let mut pending = self.pending.swap_remove(index);
                pending.frame.destination = destination_mac;
                append_effects(&mut effects, transmit(interface, neighbor, pending.frame));
            } else {
                index += 1;
            }
        }
        effects
    }
}

fn invalid_ipv4_source(address: Ipv4Addr) -> bool {
    address.is_unspecified() || address.is_multicast() || address == Ipv4Addr::BROADCAST
}

fn handled(reason: DropReason) -> EndpointReceive {
    EndpointReceive::Handled(single_effect(Effect::Drop(reason)))
}

fn arp_reply(interface: &RoutedInterface, request: ArpPacket) -> EthernetFrame {
    EthernetFrame {
        source: interface.mac,
        destination: request.sender_mac,
        vlan: interface.vlan,
        payload: NetworkPayload::Arp(ArpPacket {
            operation: ArpOperation::Reply,
            sender_mac: interface.mac,
            sender_ip: request.target_ip,
            target_mac: Some(request.sender_mac),
            target_ip: request.sender_ip,
        }),
        wire_len_bytes: EthernetFrame::MIN_WIRE_LEN_BYTES,
    }
}

fn arp_request(
    interface: &RoutedInterface,
    sender_ip: Ipv4Addr,
    target_ip: Ipv4Addr,
) -> EthernetFrame {
    EthernetFrame {
        source: interface.mac,
        destination: MacAddress::BROADCAST,
        vlan: interface.vlan,
        payload: NetworkPayload::Arp(ArpPacket {
            operation: ArpOperation::Request,
            sender_mac: interface.mac,
            sender_ip,
            target_mac: None,
            target_ip,
        }),
        wire_len_bytes: EthernetFrame::MIN_WIRE_LEN_BYTES,
    }
}

fn transmit(interface: &RoutedInterface, neighbor: Ipv4Addr, frame: EthernetFrame) -> EffectList {
    single_effect(Effect::Transmit {
        egress: interface.id.clone(),
        next_hop: Some(neighbor),
        frame,
        delay_ms: 0,
    })
}

fn append_effects(target: &mut EffectList, source: EffectList) {
    for effect in source {
        target
            .push(effect)
            .expect("endpoint effects exceed runtime capacity");
    }
}

pub(crate) fn response_frame(
    interface: &RoutedInterface,
    mut frame: EthernetFrame,
    application: ApplicationData,
) -> EthernetFrame {
    frame.destination = frame.source;
    frame.source = interface.mac;
    let NetworkPayload::Ipv4(packet) = &mut frame.payload else {
        return frame;
    };
    mem::swap(&mut packet.source, &mut packet.destination);
    packet.ttl = 64;
    packet.application = application;
    packet.transport = match packet.transport {
        Transport::Icmp(IcmpMessage::EchoRequest {
            identifier,
            sequence,
        }) => Transport::Icmp(IcmpMessage::EchoReply {
            identifier,
            sequence,
        }),
        Transport::Tcp(segment) => Transport::Tcp(hearthline_model::TcpSegment {
            source_port: segment.destination_port,
            destination_port: segment.source_port,
            flags: TcpFlags {
                ack: true,
                ..TcpFlags::default()
            },
        }),
        Transport::Udp(datagram) => Transport::Udp(hearthline_model::UdpDatagram {
            source_port: datagram.destination_port,
            destination_port: datagram.source_port,
        }),
        other => other,
    };
    frame
}
