use core::net::Ipv4Addr;

use heapless::Vec as FixedList;
use hearthline_model::{
    ArpOperation, ArpPacket, EthernetFrame, MacAddress, NetworkPayload, PortId, Route,
};

use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{DropReason, Effect, EffectList, NetworkIngress};

use super::arp::{invalid_ipv4_source, reply as arp_reply, request as arp_request};
use super::interface::RoutedInterface;
use super::neighbor::{NeighborCache, NeighborEntry};
use super::router::RoutingTable;

const PENDING_CAPACITY: usize = 16;
const PROXY_ADDRESS_CAPACITY: usize = 16;

#[derive(Clone, Debug)]
struct PendingPacket {
    neighbor: Ipv4Addr,
    egress: PortId,
    frame: EthernetFrame,
}

// Keeping effects inline preserves the engine's allocator-free runtime contract.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub(crate) enum ReceiveOutcome {
    Handled(EffectList),
    Local {
        ingress: PortId,
        frame: EthernetFrame,
    },
    Transit {
        frame: EthernetFrame,
        received_at_us: u64,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct ForwardingPlane {
    interfaces: FixedList<RoutedInterface, 16>,
    routes: RoutingTable,
    neighbors: NeighborCache,
    pending: FixedList<PendingPacket, PENDING_CAPACITY>,
    proxy_addresses: FixedList<(Ipv4Addr, PortId), PROXY_ADDRESS_CAPACITY>,
}

impl ForwardingPlane {
    pub fn new(
        interfaces: impl IntoIterator<Item = RoutedInterface>,
        routes: RoutingTable,
    ) -> Self {
        Self {
            interfaces: collect_fixed(interfaces),
            routes,
            neighbors: NeighborCache::default(),
            pending: FixedList::new(),
            proxy_addresses: FixedList::new(),
        }
    }

    pub fn has_port(&self, port: &PortId) -> bool {
        self.interfaces
            .iter()
            .any(|interface| interface.id == *port)
    }

    pub fn neighbor(
        &self,
        address: Ipv4Addr,
        port: &PortId,
        now_us: u64,
    ) -> Option<&NeighborEntry> {
        self.neighbors.entry(address, port, now_us)
    }

    pub fn route(&self, destination: Ipv4Addr) -> Option<&Route> {
        self.routes.lookup(destination)
    }

    pub fn interface_is_on_link(&self, port: &PortId, address: Ipv4Addr) -> bool {
        self.interfaces
            .iter()
            .find(|interface| interface.id == *port)
            .is_some_and(|interface| interface.is_on_link(address))
    }

    pub fn add_proxy_address(&mut self, port: PortId, address: Ipv4Addr) -> Result<(), ()> {
        if self
            .interfaces
            .iter()
            .any(|interface| interface.has_address(address))
            || self
                .proxy_addresses
                .iter()
                .any(|candidate| candidate.0 == address && candidate.1 == port)
        {
            return Ok(());
        }
        if !self.interface_is_on_link(&port, address) {
            return Err(());
        }
        self.proxy_addresses.push((address, port)).map_err(|_| ())
    }

    pub fn set_first_hop_active(&mut self, port: &PortId, address: Ipv4Addr, active: bool) -> bool {
        self.interfaces
            .iter_mut()
            .find(|interface| interface.id == *port)
            .is_some_and(|interface| interface.set_first_hop_active(address, active))
    }

    pub fn set_all_first_hop_active(&mut self, port: &PortId, active: bool) -> bool {
        self.interfaces
            .iter_mut()
            .find(|interface| interface.id == *port)
            .is_some_and(|interface| interface.set_all_first_hop_active(active))
    }

    pub fn first_hop_announcements(&self, ports: &[PortId]) -> EffectList {
        let mut effects = EffectList::new();
        for interface in self
            .interfaces
            .iter()
            .filter(|interface| ports.contains(&interface.id))
        {
            for first_hop in interface
                .first_hop_addresses()
                .iter()
                .filter(|address| address.active)
            {
                effects
                    .push(Effect::Transmit {
                        egress: interface.id.clone(),
                        next_hop: None,
                        frame: arp_request(
                            interface,
                            first_hop.address,
                            first_hop.mac,
                            first_hop.address,
                        ),
                        delay_ms: 0,
                    })
                    .expect("first-hop announcement count fits effect capacity");
            }
        }
        effects
    }

    pub fn receive(&mut self, ingress: NetworkIngress) -> ReceiveOutcome {
        let Some(interface) = self
            .interfaces
            .iter()
            .find(|interface| interface.id == ingress.port)
            .cloned()
        else {
            return ReceiveOutcome::Handled(single_effect(Effect::Drop(
                DropReason::InvalidIngress(ingress.port),
            )));
        };
        if !interface.forwarding {
            return ReceiveOutcome::Handled(single_effect(Effect::Drop(DropReason::PortDown(
                ingress.port,
            ))));
        }
        if ingress.frame.vlan != interface.vlan {
            return ReceiveOutcome::Handled(single_effect(Effect::Drop(
                DropReason::VlanNotAllowed(ingress.frame.vlan.get()),
            )));
        }
        if !ingress.frame.has_valid_wire_length() {
            return ReceiveOutcome::Handled(single_effect(Effect::Drop(
                DropReason::InvalidEthernetFrame,
            )));
        }
        if !interface.accepts_wire_len(ingress.frame.wire_len_bytes) {
            return ReceiveOutcome::Handled(single_effect(Effect::Drop(
                DropReason::InterfaceMtuExceeded {
                    port: interface.id,
                    wire_bytes: ingress.frame.wire_len_bytes,
                    maximum: u32::from(interface.mtu)
                        .saturating_add(EthernetFrame::VLAN_OVERHEAD_BYTES),
                },
            )));
        }
        if !ingress.frame.source.is_unicast() {
            return ReceiveOutcome::Handled(single_effect(Effect::Drop(
                DropReason::InvalidSourceMac(ingress.frame.source),
            )));
        }

        match ingress.frame.payload {
            NetworkPayload::Arp(packet) => ReceiveOutcome::Handled(self.handle_arp(
                interface,
                ingress.frame,
                packet,
                ingress.received_at_us,
            )),
            NetworkPayload::FirewallHa(_) => ReceiveOutcome::Handled(single_effect(Effect::Drop(
                DropReason::UnsupportedProtocol,
            ))),
            NetworkPayload::Ipv4(ref packet) => {
                if !interface.accepts_destination_mac(ingress.frame.destination) {
                    return ReceiveOutcome::Handled(single_effect(Effect::Drop(
                        DropReason::L2DestinationMismatch {
                            expected: interface.mac,
                            actual: ingress.frame.destination,
                        },
                    )));
                }
                if invalid_ipv4_source(packet.source) {
                    return ReceiveOutcome::Handled(single_effect(Effect::Drop(
                        DropReason::InvalidSourceIp(packet.source),
                    )));
                }
                if self
                    .interfaces
                    .iter()
                    .any(|candidate| candidate.owns_active_address(packet.destination))
                    || self.proxy_addresses.iter().any(|(address, port)| {
                        *address == packet.destination && *port == interface.id
                    })
                {
                    ReceiveOutcome::Local {
                        ingress: interface.id,
                        frame: ingress.frame,
                    }
                } else {
                    ReceiveOutcome::Transit {
                        frame: ingress.frame,
                        received_at_us: ingress.received_at_us,
                    }
                }
            }
        }
    }

    pub fn forward(&mut self, mut frame: EthernetFrame, now_us: u64) -> EffectList {
        let NetworkPayload::Ipv4(packet) = &mut frame.payload else {
            return single_effect(Effect::Drop(DropReason::UnsupportedProtocol));
        };
        if packet.ttl <= 1 {
            return single_effect(Effect::Drop(DropReason::TtlExpired));
        }
        let Some(route) = self.routes.lookup(packet.destination) else {
            return single_effect(Effect::Drop(DropReason::NoRoute(packet.destination)));
        };
        let Some(interface) = self
            .interfaces
            .iter()
            .find(|interface| interface.id == route.egress)
            .cloned()
        else {
            return single_effect(Effect::Drop(DropReason::InvalidIngress(
                route.egress.clone(),
            )));
        };
        if !interface.accepts_wire_len(frame.wire_len_bytes) {
            return single_effect(Effect::Drop(DropReason::InterfaceMtuExceeded {
                port: interface.id,
                wire_bytes: frame.wire_len_bytes,
                maximum: u32::from(interface.mtu)
                    .saturating_add(EthernetFrame::VLAN_OVERHEAD_BYTES),
            }));
        }
        let neighbor = route.next_hop.unwrap_or(packet.destination);
        if !interface.is_on_link(neighbor) {
            return single_effect(Effect::Drop(DropReason::NextHopOffLink {
                next_hop: neighbor,
                egress: interface.id,
            }));
        }
        packet.ttl -= 1;
        let Some((sender_ip, sender_mac)) = interface.egress_identity() else {
            return single_effect(Effect::Drop(DropReason::NoInterfaceAddress(interface.id)));
        };
        if let Some(destination_mac) = self.neighbors.lookup(neighbor, &interface.id, now_us) {
            frame.source = sender_mac;
            frame.destination = destination_mac;
            frame.vlan = interface.vlan;
            return single_effect(Effect::Transmit {
                egress: interface.id,
                next_hop: Some(neighbor),
                frame,
                delay_ms: 0,
            });
        }

        let resolution_pending = self
            .pending
            .iter()
            .any(|pending| pending.neighbor == neighbor && pending.egress == interface.id);
        if self
            .pending
            .push(PendingPacket {
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
                    "queued IPv4 packet pending ARP resolution for {neighbor}"
                )),
            });
        }
        single_effect(Effect::Transmit {
            egress: interface.id.clone(),
            next_hop: Some(neighbor),
            frame: arp_request(&interface, sender_ip, sender_mac, neighbor),
            delay_ms: 0,
        })
    }

    fn handle_arp(
        &mut self,
        interface: RoutedInterface,
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
        if packet.sender_ip != Ipv4Addr::UNSPECIFIED {
            self.neighbors
                .learn(packet.sender_ip, packet.sender_mac, &interface.id, now_us);
        }
        match packet.operation {
            ArpOperation::Request => {
                if packet.target_mac.is_some()
                    || !(frame.destination.is_broadcast()
                        || interface.accepts_destination_mac(frame.destination))
                {
                    return single_effect(Effect::Drop(DropReason::InvalidArp));
                }
                let is_proxy = self
                    .proxy_addresses
                    .iter()
                    .any(|(address, port)| *address == packet.target_ip && *port == interface.id);
                let response_mac = if is_proxy {
                    Some(interface.mac)
                } else {
                    interface.response_mac(packet.target_ip)
                };
                let Some(response_mac) = response_mac else {
                    return single_effect(Effect::Observe {
                        detail: runtime_text(format_args!(
                            "ignored ARP request for {}",
                            packet.target_ip
                        )),
                    });
                };
                single_effect(Effect::Transmit {
                    egress: interface.id.clone(),
                    next_hop: Some(packet.sender_ip),
                    frame: arp_reply(&interface, packet, response_mac),
                    delay_ms: 0,
                })
            }
            ArpOperation::Reply => {
                if packet.target_mac != Some(frame.destination)
                    || interface.response_mac(packet.target_ip) != packet.target_mac
                {
                    return single_effect(Effect::Drop(DropReason::InvalidArp));
                }
                self.release_pending(&interface, packet.sender_ip, packet.sender_mac, now_us)
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
                let Some((_, sender_mac)) = interface.egress_identity() else {
                    return single_effect(Effect::Drop(DropReason::NoInterfaceAddress(
                        interface.id.clone(),
                    )));
                };
                pending.frame.source = sender_mac;
                pending.frame.destination = destination_mac;
                pending.frame.vlan = interface.vlan;
                effects
                    .push(Effect::Transmit {
                        egress: interface.id.clone(),
                        next_hop: Some(neighbor),
                        frame: pending.frame,
                        delay_ms: 0,
                    })
                    .expect("pending queue cannot exceed effect capacity");
            } else {
                index += 1;
            }
        }
        if effects.is_empty() {
            effects
                .push(Effect::Observe {
                    detail: runtime_text(format_args!(
                        "learned ARP neighbor {neighbor} without queued traffic"
                    )),
                })
                .expect("single observation fits effect capacity");
        }
        effects
    }
}
