use std::fmt::{self, Display, Formatter};
use std::net::Ipv4Addr;

use crate::{PortId, ServiceKind};

/// IEEE 802 MAC address used by the abstract Ethernet model.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MacAddress([u8; 6]);

impl MacAddress {
    pub const BROADCAST: Self = Self([0xff; 6]);

    pub const fn new(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 6] {
        self.0
    }

    pub fn is_broadcast(self) -> bool {
        self.0 == Self::BROADCAST.0
    }

    pub const fn is_multicast(self) -> bool {
        self.0[0] & 1 == 1
    }
}

impl Display for MacAddress {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

/// IEEE 802.1Q VLAN identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VlanId(u16);

impl VlanId {
    pub fn new(value: u16) -> Option<Self> {
        (1..=4094).contains(&value).then_some(Self(value))
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

/// IPv4 prefix with longest-prefix matching support.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Ipv4Cidr {
    network: Ipv4Addr,
    prefix: u8,
}

impl Ipv4Cidr {
    pub fn new(address: Ipv4Addr, prefix: u8) -> Option<Self> {
        if prefix > 32 {
            return None;
        }
        let mask = prefix_mask(prefix);
        let network = Ipv4Addr::from(u32::from(address) & mask);
        Some(Self { network, prefix })
    }

    pub const fn network(self) -> Ipv4Addr {
        self.network
    }

    pub const fn prefix(self) -> u8 {
        self.prefix
    }

    pub fn contains(self, address: Ipv4Addr) -> bool {
        let mask = prefix_mask(self.prefix);
        u32::from(address) & mask == u32::from(self.network)
    }
}

impl Display for Ipv4Cidr {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.network, self.prefix)
    }
}

const fn prefix_mask(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    }
}

/// One route in an appliance forwarding table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Route {
    pub destination: Ipv4Cidr,
    pub egress: PortId,
    pub next_hop: Option<Ipv4Addr>,
    pub metric: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TransportProtocol {
    Icmp,
    Tcp,
    Udp,
    Other(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IcmpMessage {
    EchoRequest { identifier: u16, sequence: u16 },
    EchoReply { identifier: u16, sequence: u16 },
    DestinationUnreachable,
    TimeExceeded,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TcpFlags {
    pub syn: bool,
    pub ack: bool,
    pub fin: bool,
    pub rst: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TcpSegment {
    pub source_port: u16,
    pub destination_port: u16,
    pub flags: TcpFlags,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UdpDatagram {
    pub source_port: u16,
    pub destination_port: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Transport {
    Icmp(IcmpMessage),
    Tcp(TcpSegment),
    Udp(UdpDatagram),
    Other(u8),
}

impl Transport {
    pub const fn protocol(self) -> TransportProtocol {
        match self {
            Self::Icmp(_) => TransportProtocol::Icmp,
            Self::Tcp(_) => TransportProtocol::Tcp,
            Self::Udp(_) => TransportProtocol::Udp,
            Self::Other(number) => TransportProtocol::Other(number),
        }
    }

    pub const fn source_port(self) -> Option<u16> {
        match self {
            Self::Tcp(segment) => Some(segment.source_port),
            Self::Udp(datagram) => Some(datagram.source_port),
            Self::Icmp(_) | Self::Other(_) => None,
        }
    }

    pub const fn destination_port(self) -> Option<u16> {
        match self {
            Self::Tcp(segment) => Some(segment.destination_port),
            Self::Udp(datagram) => Some(datagram.destination_port),
            Self::Icmp(_) | Self::Other(_) => None,
        }
    }

    pub const fn source_token(self) -> Option<u16> {
        match self {
            Self::Icmp(IcmpMessage::EchoRequest { identifier, .. })
            | Self::Icmp(IcmpMessage::EchoReply { identifier, .. }) => Some(identifier),
            Self::Tcp(segment) => Some(segment.source_port),
            Self::Udp(datagram) => Some(datagram.source_port),
            Self::Icmp(IcmpMessage::DestinationUnreachable | IcmpMessage::TimeExceeded)
            | Self::Other(_) => None,
        }
    }

    pub const fn destination_token(self) -> Option<u16> {
        match self {
            Self::Icmp(IcmpMessage::EchoRequest { identifier, .. })
            | Self::Icmp(IcmpMessage::EchoReply { identifier, .. }) => Some(identifier),
            Self::Tcp(segment) => Some(segment.destination_port),
            Self::Udp(datagram) => Some(datagram.destination_port),
            Self::Icmp(IcmpMessage::DestinationUnreachable | IcmpMessage::TimeExceeded)
            | Self::Other(_) => None,
        }
    }

    pub fn rewrite_source_port(&mut self, port: u16) {
        match self {
            Self::Tcp(segment) => segment.source_port = port,
            Self::Udp(datagram) => datagram.source_port = port,
            Self::Icmp(_) | Self::Other(_) => {}
        }
    }

    pub fn rewrite_destination_port(&mut self, port: u16) {
        match self {
            Self::Tcp(segment) => segment.destination_port = port,
            Self::Udp(datagram) => datagram.destination_port = port,
            Self::Icmp(_) | Self::Other(_) => {}
        }
    }

    pub fn rewrite_source_token(&mut self, token: u16) {
        match self {
            Self::Icmp(IcmpMessage::EchoRequest { identifier, .. })
            | Self::Icmp(IcmpMessage::EchoReply { identifier, .. }) => *identifier = token,
            Self::Tcp(segment) => segment.source_port = token,
            Self::Udp(datagram) => datagram.source_port = token,
            Self::Icmp(IcmpMessage::DestinationUnreachable | IcmpMessage::TimeExceeded)
            | Self::Other(_) => {}
        }
    }

    pub fn rewrite_destination_token(&mut self, token: u16) {
        match self {
            Self::Icmp(IcmpMessage::EchoRequest { identifier, .. })
            | Self::Icmp(IcmpMessage::EchoReply { identifier, .. }) => *identifier = token,
            Self::Tcp(segment) => segment.destination_port = token,
            Self::Udp(datagram) => datagram.destination_port = token,
            Self::Icmp(IcmpMessage::DestinationUnreachable | IcmpMessage::TimeExceeded)
            | Self::Other(_) => {}
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
    Options,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplicationData {
    None,
    DnsQuery {
        name: String,
    },
    DnsAnswer {
        name: String,
        address: Option<Ipv4Addr>,
    },
    HttpRequest {
        method: HttpMethod,
        host: String,
        path: String,
        body_bytes: usize,
    },
    HttpResponse {
        status: u16,
    },
    Service(ServiceKind),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ipv4Packet {
    pub source: Ipv4Addr,
    pub destination: Ipv4Addr,
    pub ttl: u8,
    pub transport: Transport,
    pub application: ApplicationData,
}

impl Ipv4Packet {
    pub fn flow_key(&self) -> FlowKey {
        FlowKey {
            source: self.source,
            destination: self.destination,
            protocol: self.transport.protocol(),
            source_port: self.transport.source_port(),
            destination_port: self.transport.destination_port(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FlowKey {
    pub source: Ipv4Addr,
    pub destination: Ipv4Addr,
    pub protocol: TransportProtocol,
    pub source_port: Option<u16>,
    pub destination_port: Option<u16>,
}

impl FlowKey {
    pub const fn reverse(self) -> Self {
        Self {
            source: self.destination,
            destination: self.source,
            protocol: self.protocol,
            source_port: self.destination_port,
            destination_port: self.source_port,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArpOperation {
    Request,
    Reply,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArpPacket {
    pub operation: ArpOperation,
    pub sender_mac: MacAddress,
    pub sender_ip: Ipv4Addr,
    pub target_mac: Option<MacAddress>,
    pub target_ip: Ipv4Addr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NetworkPayload {
    Arp(ArpPacket),
    Ipv4(Ipv4Packet),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetFrame {
    pub source: MacAddress,
    pub destination: MacAddress,
    pub vlan: VlanId,
    pub payload: NetworkPayload,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cidr_normalizes_and_matches() {
        let prefix =
            Ipv4Cidr::new(Ipv4Addr::new(192, 168, 0, 55), 24).expect("test prefix must be valid");
        assert_eq!(prefix.network(), Ipv4Addr::new(192, 168, 0, 0));
        assert!(prefix.contains(Ipv4Addr::new(192, 168, 0, 200)));
        assert!(!prefix.contains(Ipv4Addr::new(192, 168, 1, 1)));
    }

    #[test]
    fn reverse_flow_swaps_endpoints() {
        let flow = FlowKey {
            source: Ipv4Addr::new(10, 0, 0, 10),
            destination: Ipv4Addr::new(10, 0, 1, 20),
            protocol: TransportProtocol::Tcp,
            source_port: Some(50_000),
            destination_port: Some(443),
        };
        assert_eq!(flow.reverse().source_port, Some(443));
        assert_eq!(flow.reverse().destination_port, Some(50_000));
    }
}
