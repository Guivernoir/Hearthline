use core::net::Ipv4Addr;

use hearthline_model::{ArpOperation, ArpPacket, EthernetFrame, MacAddress, NetworkPayload};

use super::RoutedInterface;

pub(super) fn invalid_ipv4_source(address: Ipv4Addr) -> bool {
    address.is_unspecified() || address.is_multicast() || address == Ipv4Addr::BROADCAST
}

pub(super) fn request(
    interface: &RoutedInterface,
    sender_ip: Ipv4Addr,
    sender_mac: MacAddress,
    target_ip: Ipv4Addr,
) -> EthernetFrame {
    EthernetFrame {
        source: sender_mac,
        destination: MacAddress::BROADCAST,
        vlan: interface.vlan,
        payload: NetworkPayload::Arp(ArpPacket {
            operation: ArpOperation::Request,
            sender_mac,
            sender_ip,
            target_mac: None,
            target_ip,
        }),
        wire_len_bytes: EthernetFrame::MIN_WIRE_LEN_BYTES,
    }
}

pub(super) fn reply(
    interface: &RoutedInterface,
    request: ArpPacket,
    response_mac: MacAddress,
) -> EthernetFrame {
    EthernetFrame {
        source: response_mac,
        destination: request.sender_mac,
        vlan: interface.vlan,
        payload: NetworkPayload::Arp(ArpPacket {
            operation: ArpOperation::Reply,
            sender_mac: response_mac,
            sender_ip: request.target_ip,
            target_mac: Some(request.sender_mac),
            target_ip: request.sender_ip,
        }),
        wire_len_bytes: EthernetFrame::MIN_WIRE_LEN_BYTES,
    }
}
