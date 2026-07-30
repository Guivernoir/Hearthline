use std::net::Ipv4Addr;

use hearthline_engine::{
    DnsServer, Effect, LearningSwitch, NetworkIngress, ReverseProxyWaf, Router, RoutingTable,
    SimulatedComponent, SimulationEvent, SwitchPort,
};
use hearthline_model::{
    ApplicationData, ComponentId, ComponentKind, EthernetFrame, HttpMethod, IcmpMessage, Ipv4Cidr,
    Ipv4Packet, MacAddress, NetworkPayload, PortId, Route, TcpFlags, TcpSegment, Transport,
    UdpDatagram, VlanId,
};

fn id(value: &str) -> ComponentId {
    ComponentId::new(value).expect("test ID")
}

fn port(value: &str) -> PortId {
    PortId::new(value).expect("test port")
}

fn frame(source: MacAddress, destination: MacAddress, vlan: VlanId) -> EthernetFrame {
    EthernetFrame {
        source,
        destination,
        vlan,
        payload: NetworkPayload::Ipv4(Ipv4Packet {
            source: Ipv4Addr::new(192, 168, 0, 2),
            destination: Ipv4Addr::new(192, 168, 0, 1),
            ttl: 64,
            transport: Transport::Icmp(IcmpMessage::EchoRequest {
                identifier: 1,
                sequence: 1,
            }),
            application: ApplicationData::None,
        }),
    }
}

fn request_frame(application: ApplicationData, transport: Transport) -> EthernetFrame {
    EthernetFrame {
        source: MacAddress::new([0, 1, 2, 3, 4, 5]),
        destination: MacAddress::new([0, 1, 2, 3, 4, 6]),
        vlan: VlanId::new(10).expect("VLAN"),
        payload: NetworkPayload::Ipv4(Ipv4Packet {
            source: Ipv4Addr::new(203, 0, 113, 2),
            destination: Ipv4Addr::new(172, 16, 10, 2),
            ttl: 64,
            transport,
            application,
        }),
    }
}

#[test]
fn longest_prefix_route_wins_and_ttl_decrements() {
    let routes = RoutingTable::new([
        Route {
            destination: Ipv4Cidr::new(Ipv4Addr::UNSPECIFIED, 0).expect("default"),
            egress: port("outside"),
            next_hop: Some(Ipv4Addr::new(203, 0, 113, 1)),
            metric: 10,
        },
        Route {
            destination: Ipv4Cidr::new(Ipv4Addr::new(10, 10, 20, 0), 24).expect("internal"),
            egress: port("inside"),
            next_hop: None,
            metric: 0,
        },
    ]);
    let mut router = Router::new(
        id("router-01"),
        ComponentKind::Router,
        [port("inside"), port("outside")],
        routes,
    );
    let routed = EthernetFrame {
        source: MacAddress::new([0, 1, 2, 3, 4, 5]),
        destination: MacAddress::new([0, 1, 2, 3, 4, 6]),
        vlan: VlanId::new(20).expect("VLAN"),
        payload: NetworkPayload::Ipv4(Ipv4Packet {
            source: Ipv4Addr::new(10, 10, 30, 10),
            destination: Ipv4Addr::new(10, 10, 20, 10),
            ttl: 64,
            transport: Transport::Tcp(TcpSegment {
                source_port: 50_000,
                destination_port: 443,
                flags: TcpFlags::default(),
            }),
            application: ApplicationData::None,
        }),
    };
    let effects = router.handle(SimulationEvent::Network(NetworkIngress {
        port: port("inside"),
        frame: routed,
    }));
    let Effect::Transmit { egress, frame, .. } = &effects[0] else {
        panic!("expected transmission");
    };
    assert_eq!(egress, &port("inside"));
    let NetworkPayload::Ipv4(packet) = &frame.payload else {
        panic!("expected IPv4 packet");
    };
    assert_eq!(packet.ttl, 63);
}

#[test]
fn switch_learns_source_and_uses_known_unicast() {
    let vlan = VlanId::new(1).expect("VLAN");
    let mac_a = MacAddress::new([0, 0, 0, 0, 0, 1]);
    let mac_b = MacAddress::new([0, 0, 0, 0, 0, 2]);
    let mut switch = LearningSwitch::new(
        id("switch-01"),
        [
            SwitchPort::access(port("port-a"), vlan),
            SwitchPort::access(port("port-b"), vlan),
            SwitchPort::access(port("port-c"), vlan),
        ],
    );
    switch.handle(SimulationEvent::Network(NetworkIngress {
        port: port("port-a"),
        frame: frame(mac_a, mac_b, vlan),
    }));
    let effects = switch.handle(SimulationEvent::Network(NetworkIngress {
        port: port("port-b"),
        frame: frame(mac_b, mac_a, vlan),
    }));
    assert_eq!(switch.learned_port(vlan, mac_a), Some(&port("port-a")));
    assert_eq!(effects.len(), 1);
    assert!(matches!(
        &effects[0],
        Effect::Transmit { egress, .. } if egress == &port("port-a")
    ));
}

#[test]
fn dns_returns_declared_record() {
    let mut dns = DnsServer::new(
        id("isp-dns-01"),
        [port("network")],
        [Ipv4Addr::new(198, 51, 100, 50)],
        [("www.business.example".into(), Ipv4Addr::new(192, 0, 2, 10))],
    );
    let mut request = request_frame(
        ApplicationData::DnsQuery {
            name: "www.business.example".into(),
        },
        Transport::Udp(UdpDatagram {
            source_port: 50_000,
            destination_port: 53,
        }),
    );
    let NetworkPayload::Ipv4(packet) = &mut request.payload else {
        panic!("IPv4");
    };
    packet.destination = Ipv4Addr::new(198, 51, 100, 50);
    let effects = dns.handle(SimulationEvent::Network(NetworkIngress {
        port: port("network"),
        frame: request,
    }));
    let Effect::Transmit { frame, .. } = &effects[0] else {
        panic!("DNS response");
    };
    let NetworkPayload::Ipv4(packet) = &frame.payload else {
        panic!("IPv4");
    };
    assert_eq!(
        packet.application,
        ApplicationData::DnsAnswer {
            name: "www.business.example".into(),
            address: Some(Ipv4Addr::new(192, 0, 2, 10)),
        }
    );
}

#[test]
fn reverse_proxy_rejects_unknown_host_and_forwards_allowed_host() {
    let mut proxy = ReverseProxyWaf::new(
        id("business-web-gw-01"),
        [port("dmz")],
        [Ipv4Addr::new(172, 16, 10, 2)],
        ["www.business.example".into()],
        id("internal-app-vip"),
    );
    let request = |host: &str| {
        request_frame(
            ApplicationData::HttpRequest {
                method: HttpMethod::Get,
                host: host.into(),
                path: "/shop".into(),
                body_bytes: 0,
            },
            Transport::Tcp(TcpSegment {
                source_port: 50_000,
                destination_port: 443,
                flags: TcpFlags {
                    syn: true,
                    ..TcpFlags::default()
                },
            }),
        )
    };
    let denied = proxy.handle(SimulationEvent::Network(NetworkIngress {
        port: port("dmz"),
        frame: request("invalid.example"),
    }));
    assert!(matches!(denied[0], Effect::Drop(_)));
    let allowed = proxy.handle(SimulationEvent::Network(NetworkIngress {
        port: port("dmz"),
        frame: request("www.business.example"),
    }));
    assert!(matches!(allowed[0], Effect::ApplicationForward { .. }));
}
