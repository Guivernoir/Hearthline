use std::net::Ipv4Addr;

use hearthline_engine::{
    DnsServer, DropReason, Effect, LearningSwitch, NetworkIngress, ReverseProxyWaf,
    RoutedInterface, Router, RoutingTable, ServiceNode, SimulatedComponent, SimulationEvent,
    SwitchPort,
};
use hearthline_model::{
    ApplicationData, ArpOperation, ArpPacket, ComponentId, ComponentKind, EthernetFrame,
    HttpMethod, IcmpMessage, Ipv4Cidr, Ipv4InterfaceAddress, Ipv4Packet, MacAddress,
    NetworkPayload, PortId, Route, TcpFlags, TcpSegment, Transport, UdpDatagram, VlanId,
};

fn id(value: &str) -> ComponentId {
    ComponentId::new(value).expect("test ID")
}

fn port(value: &str) -> PortId {
    PortId::new(value).expect("test port")
}

fn host_interface(
    port_id: &str,
    mac: MacAddress,
    address: Ipv4Addr,
    prefix: u8,
    vlan: VlanId,
) -> RoutedInterface {
    RoutedInterface::new(
        port(port_id),
        mac,
        [Ipv4InterfaceAddress::new(address, prefix).expect("interface address")],
        vlan,
        1_500,
    )
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
        wire_len_bytes: 64,
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
        wire_len_bytes: 64,
    }
}

#[test]
fn router_resolves_arp_before_longest_prefix_forwarding() {
    let inside_mac = MacAddress::new([0x02, 0, 0, 0, 0, 1]);
    let outside_mac = MacAddress::new([0x02, 0, 0, 0, 0, 2]);
    let host_mac = MacAddress::new([0x02, 0, 0, 0, 0, 10]);
    let inside_vlan = VlanId::new(20).expect("VLAN");
    let outside_vlan = VlanId::new(100).expect("VLAN");
    let target = Ipv4Addr::new(10, 10, 20, 10);
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
        [
            RoutedInterface::new(
                port("inside"),
                inside_mac,
                [Ipv4InterfaceAddress::new(Ipv4Addr::new(10, 10, 20, 1), 24)
                    .expect("interface address")],
                inside_vlan,
                1_500,
            ),
            RoutedInterface::new(
                port("outside"),
                outside_mac,
                [Ipv4InterfaceAddress::new(Ipv4Addr::new(203, 0, 113, 2), 24)
                    .expect("interface address")],
                outside_vlan,
                1_500,
            ),
        ],
        routes,
    );
    let routed = EthernetFrame {
        source: MacAddress::new([0, 1, 2, 3, 4, 5]),
        destination: outside_mac,
        vlan: outside_vlan,
        payload: NetworkPayload::Ipv4(Ipv4Packet {
            source: Ipv4Addr::new(198, 51, 100, 10),
            destination: target,
            ttl: 64,
            transport: Transport::Tcp(TcpSegment {
                source_port: 50_000,
                destination_port: 443,
                flags: TcpFlags::default(),
            }),
            application: ApplicationData::None,
        }),
        wire_len_bytes: 64,
    };
    let unresolved = router.handle(SimulationEvent::Network(NetworkIngress {
        port: port("outside"),
        frame: routed.clone(),
        received_at_us: 0,
    }));
    let Effect::Transmit {
        egress,
        frame: arp_request,
        ..
    } = &unresolved[0]
    else {
        panic!("expected ARP request");
    };
    assert_eq!(egress, &port("inside"));
    assert_eq!(arp_request.destination, MacAddress::BROADCAST);
    assert!(matches!(
        arp_request.payload,
        NetworkPayload::Arp(ArpPacket {
            operation: ArpOperation::Request,
            target_ip,
            ..
        }) if target_ip == target
    ));

    let resolved = router.handle(SimulationEvent::Network(NetworkIngress {
        port: port("inside"),
        frame: EthernetFrame {
            source: host_mac,
            destination: inside_mac,
            vlan: inside_vlan,
            payload: NetworkPayload::Arp(ArpPacket {
                operation: ArpOperation::Reply,
                sender_mac: host_mac,
                sender_ip: target,
                target_mac: Some(inside_mac),
                target_ip: Ipv4Addr::new(10, 10, 20, 1),
            }),
            wire_len_bytes: 64,
        },
        received_at_us: 100,
    }));
    let Effect::Transmit { egress, frame, .. } = &resolved[0] else {
        panic!("expected resolved IPv4 transmission");
    };
    assert_eq!(egress, &port("inside"));
    assert_eq!(frame.source, inside_mac);
    assert_eq!(frame.destination, host_mac);
    let NetworkPayload::Ipv4(packet) = &frame.payload else {
        panic!("expected IPv4 packet");
    };
    assert_eq!(packet.ttl, 63);
    assert_eq!(
        router
            .neighbor(target, &port("inside"), 100)
            .map(|entry| entry.mac),
        Some(host_mac)
    );

    let mut oversized = routed;
    oversized.wire_len_bytes = 1_523;
    let rejected = router.handle(SimulationEvent::Network(NetworkIngress {
        port: port("outside"),
        frame: oversized,
        received_at_us: 101,
    }));
    assert!(matches!(
        &rejected[0],
        Effect::Drop(DropReason::InterfaceMtuExceeded {
            port: rejected_port,
            ..
        }) if rejected_port == &port("outside")
    ));
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
        received_at_us: 0,
    }));
    let effects = switch.handle(SimulationEvent::Network(NetworkIngress {
        port: port("port-b"),
        frame: frame(mac_b, mac_a, vlan),
        received_at_us: 0,
    }));
    assert_eq!(switch.learned_port(vlan, mac_a), Some(&port("port-a")));
    assert_eq!(effects.len(), 1);
    assert!(matches!(
        &effects[0],
        Effect::Transmit { egress, .. } if egress == &port("port-a")
    ));
}

#[test]
fn switch_rejects_non_unicast_source_addresses() {
    let vlan = VlanId::new(1).expect("VLAN");
    let mut switch = LearningSwitch::new(
        id("switch-01"),
        [
            SwitchPort::access(port("port-a"), vlan),
            SwitchPort::access(port("port-b"), vlan),
        ],
    );
    let effects = switch.handle(SimulationEvent::Network(NetworkIngress {
        port: port("port-a"),
        frame: frame(
            MacAddress::BROADCAST,
            MacAddress::new([0, 0, 0, 0, 0, 2]),
            vlan,
        ),
        received_at_us: 0,
    }));
    assert!(matches!(
        effects[0],
        Effect::Drop(hearthline_engine::DropReason::InvalidSourceMac(
            MacAddress::BROADCAST
        ))
    ));
    assert!(switch.mac_table().is_empty());
}

#[test]
fn switch_tracks_mac_moves_and_ages_stale_entries() {
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
    switch.set_aging_time_us(100);
    switch.handle(SimulationEvent::Network(NetworkIngress {
        port: port("port-a"),
        frame: frame(mac_a, mac_b, vlan),
        received_at_us: 0,
    }));
    switch.handle(SimulationEvent::Network(NetworkIngress {
        port: port("port-b"),
        frame: frame(mac_a, mac_b, vlan),
        received_at_us: 50,
    }));
    assert_eq!(switch.learned_port(vlan, mac_a), Some(&port("port-b")));

    switch.handle(SimulationEvent::Network(NetworkIngress {
        port: port("port-c"),
        frame: frame(mac_b, mac_a, vlan),
        received_at_us: 150,
    }));
    assert_eq!(switch.learned_port(vlan, mac_a), None);
    assert_eq!(switch.learned_port(vlan, mac_b), Some(&port("port-c")));
}

#[test]
fn endpoint_answers_arp_and_rejects_wrong_destination_mac() {
    let vlan = VlanId::new(10).expect("VLAN");
    let server_ip = Ipv4Addr::new(192, 0, 2, 10);
    let server_mac = MacAddress::new([0x02, 0, 0, 0, 0, 10]);
    let client_ip = Ipv4Addr::new(192, 0, 2, 20);
    let client_mac = MacAddress::new([0x02, 0, 0, 0, 0, 20]);
    let mut server = ServiceNode::new(
        id("service-01"),
        ComponentKind::ServiceCluster,
        [host_interface("network", server_mac, server_ip, 24, vlan)],
        [hearthline_model::ServiceKind::Https],
    );
    let arp_effects = server.handle(SimulationEvent::Network(NetworkIngress {
        port: port("network"),
        frame: EthernetFrame {
            source: client_mac,
            destination: MacAddress::BROADCAST,
            vlan,
            payload: NetworkPayload::Arp(ArpPacket {
                operation: ArpOperation::Request,
                sender_mac: client_mac,
                sender_ip: client_ip,
                target_mac: None,
                target_ip: server_ip,
            }),
            wire_len_bytes: EthernetFrame::MIN_WIRE_LEN_BYTES,
        },
        received_at_us: 0,
    }));
    let Effect::Transmit { egress, frame, .. } = &arp_effects[0] else {
        panic!("expected ARP reply");
    };
    assert_eq!(egress, &port("network"));
    assert_eq!(frame.source, server_mac);
    assert_eq!(frame.destination, client_mac);
    assert!(matches!(
        frame.payload,
        NetworkPayload::Arp(ArpPacket {
            operation: ArpOperation::Reply,
            sender_ip,
            target_ip,
            ..
        }) if sender_ip == server_ip && target_ip == client_ip
    ));

    let mut invalid = request_frame(
        ApplicationData::Service(hearthline_model::ServiceKind::Https),
        Transport::Tcp(TcpSegment {
            source_port: 50_000,
            destination_port: 443,
            flags: TcpFlags::default(),
        }),
    );
    invalid.destination = MacAddress::new([0x02, 0, 0, 0, 0, 99]);
    let NetworkPayload::Ipv4(packet) = &mut invalid.payload else {
        panic!("IPv4");
    };
    packet.destination = server_ip;
    let effects = server.handle(SimulationEvent::Network(NetworkIngress {
        port: port("network"),
        frame: invalid,
        received_at_us: 1,
    }));
    assert!(matches!(
        effects[0],
        Effect::Drop(DropReason::L2DestinationMismatch {
            expected,
            actual: _
        }) if expected == server_mac
    ));
}

#[test]
fn dns_returns_declared_record() {
    let server_mac = MacAddress::new([0, 1, 2, 3, 4, 6]);
    let mut dns = DnsServer::new(
        id("isp-dns-01"),
        [host_interface(
            "network",
            server_mac,
            Ipv4Addr::new(198, 51, 100, 50),
            24,
            VlanId::new(10).expect("VLAN"),
        )],
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
        received_at_us: 0,
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
    let proxy_mac = MacAddress::new([0, 1, 2, 3, 4, 6]);
    let mut proxy = ReverseProxyWaf::new(
        id("business-web-gw-01"),
        [host_interface(
            "dmz",
            proxy_mac,
            Ipv4Addr::new(172, 16, 10, 2),
            24,
            VlanId::new(10).expect("VLAN"),
        )],
        ["www.business.example".into()],
        id("internal-app-vip"),
        Ipv4Addr::new(172, 16, 10, 10),
    );
    let request = |host: &str| {
        request_frame(
            ApplicationData::HttpRequest {
                method: HttpMethod::Get,
                host: host.into(),
                path: "/shop".into(),
                body: None,
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
        received_at_us: 0,
    }));
    assert!(matches!(denied[0], Effect::Drop(_)));
    let allowed = proxy.handle(SimulationEvent::Network(NetworkIngress {
        port: port("dmz"),
        frame: request("www.business.example"),
        received_at_us: 0,
    }));
    assert!(matches!(allowed[0], Effect::ApplicationForward { .. }));
}
