use std::net::Ipv4Addr;

use hearthline_engine::{
    DropReason, Effect, FirstHopAddress, HttpInspectionRule, HttpInspectionTarget, Layer3Switch,
    LearningSwitch, NetworkIngress, ReverseProxyWaf, RoutedInterface, RoutingTable,
    SimulatedComponent, SimulationEvent, SwitchAggregationGroup, SwitchPort,
};
use hearthline_model::{
    ApplicationData, ArpOperation, ArpPacket, ComponentId, EthernetFrame, HttpMethod, Ipv4Cidr,
    Ipv4InterfaceAddress, Ipv4Packet, MacAddress, NetworkPayload, PortId, Route, TcpFlags,
    TcpSegment, Transport, VlanId,
};

fn id(value: &str) -> ComponentId {
    ComponentId::new(value).expect("test ID")
}

fn port(value: &str) -> PortId {
    PortId::new(value).expect("test port")
}

fn switch_frame(source: MacAddress, destination: MacAddress, vlan: VlanId) -> EthernetFrame {
    EthernetFrame {
        source,
        destination,
        vlan,
        payload: NetworkPayload::Ipv4(Ipv4Packet {
            source: Ipv4Addr::new(10, 10, 30, 10),
            destination: Ipv4Addr::new(10, 10, 30, 1),
            ttl: 64,
            transport: Transport::Icmp(hearthline_model::IcmpMessage::EchoRequest {
                identifier: 1,
                sequence: 1,
            }),
            application: ApplicationData::None,
        }),
        wire_len_bytes: 64,
    }
}

#[test]
fn switch_discards_only_the_spanning_tree_blocked_vlan() {
    let user_vlan = VlanId::new(30).expect("user VLAN");
    let voice_vlan = VlanId::new(40).expect("voice VLAN");
    let source = MacAddress::new([0, 0, 0, 0, 0, 1]);
    let destination = MacAddress::new([0, 0, 0, 0, 0, 2]);
    let mut switch = LearningSwitch::new(
        id("switch-01"),
        [
            SwitchPort::trunk(port("uplink-a"), [user_vlan, voice_vlan]),
            SwitchPort::trunk(port("uplink-b"), [user_vlan, voice_vlan]),
        ],
    );

    assert!(switch.set_spanning_tree_forwarding(&port("uplink-b"), user_vlan, false));
    let discarded = switch.handle(SimulationEvent::Network(NetworkIngress {
        port: port("uplink-b"),
        frame: switch_frame(source, destination, user_vlan),
        received_at_us: 0,
    }));
    assert_eq!(
        discarded.as_slice(),
        &[Effect::Drop(DropReason::SpanningTreeDiscarding {
            port: port("uplink-b"),
            vlan: 30,
        })]
    );

    let forwarded = switch.handle(SimulationEvent::Network(NetworkIngress {
        port: port("uplink-b"),
        frame: switch_frame(source, destination, voice_vlan),
        received_at_us: 1,
    }));
    assert!(matches!(
        forwarded.as_slice(),
        [Effect::Transmit { egress, .. }] if egress == &port("uplink-a")
    ));
}

#[test]
fn link_aggregation_selects_one_member_and_fails_over_without_relearning() {
    let vlan = VlanId::new(30).expect("user VLAN");
    let source = MacAddress::new([0, 0, 0, 0, 0, 1]);
    let destination = MacAddress::new([0, 0, 0, 0, 0, 2]);
    let mut switch = LearningSwitch::new(
        id("switch-01"),
        [
            SwitchPort::access(port("access"), vlan),
            SwitchPort::trunk(port("member-a"), [vlan]),
            SwitchPort::trunk(port("member-b"), [vlan]),
        ],
    );
    assert!(
        switch.add_link_aggregation_group(SwitchAggregationGroup::new(
            id("po-core"),
            id("logical-uplink"),
            [port("member-a"), port("member-b")],
            false,
        ))
    );

    let frame = switch_frame(source, destination, vlan);
    let first = switch.handle(SimulationEvent::Network(NetworkIngress {
        port: port("access"),
        frame: frame.clone(),
        received_at_us: 0,
    }));
    let Effect::Transmit { egress: active, .. } = &first[0] else {
        panic!("aggregate should select one egress member");
    };
    assert_eq!(first.len(), 1);
    let failed = active.clone();
    let learned = switch.handle(SimulationEvent::Network(NetworkIngress {
        port: failed.clone(),
        frame: switch_frame(destination, source, vlan),
        received_at_us: 1,
    }));
    assert!(
        matches!(learned.as_slice(), [Effect::Transmit { egress, .. }] if egress == &port("access"))
    );
    assert_eq!(switch.learned_port(vlan, destination), Some(&failed));
    assert!(switch.set_link_aggregation_forwarding(&failed, false));
    assert_eq!(switch.learned_port(vlan, destination), Some(&failed));

    let recovered = switch.handle(SimulationEvent::Network(NetworkIngress {
        port: port("access"),
        frame,
        received_at_us: 2,
    }));
    assert!(matches!(
        recovered.as_slice(),
        [Effect::Transmit { egress, .. }] if egress != &failed
    ));
    let discarded = switch.handle(SimulationEvent::Network(NetworkIngress {
        port: failed.clone(),
        frame: switch_frame(destination, source, vlan),
        received_at_us: 3,
    }));
    assert_eq!(
        discarded.as_slice(),
        &[Effect::Drop(DropReason::LinkAggregationDiscarding(failed))]
    );
}

fn request_frame(method: HttpMethod, destination: MacAddress, body: Option<&str>) -> EthernetFrame {
    EthernetFrame {
        source: MacAddress::new([0, 1, 2, 3, 4, 5]),
        destination,
        vlan: VlanId::new(10).expect("VLAN"),
        payload: NetworkPayload::Ipv4(Ipv4Packet {
            source: Ipv4Addr::new(203, 0, 113, 2),
            destination: Ipv4Addr::new(172, 16, 10, 2),
            ttl: 64,
            transport: Transport::Tcp(TcpSegment {
                source_port: 50_000,
                destination_port: 443,
                flags: TcpFlags {
                    syn: true,
                    ..TcpFlags::default()
                },
            }),
            application: ApplicationData::HttpRequest {
                method,
                host: "www.business.example".into(),
                path: "/shop/admin".into(),
                body: body.map(Into::into),
                body_bytes: body.map_or(0, str::len),
            },
        }),
        wire_len_bytes: 64,
    }
}

#[test]
fn reverse_proxy_uses_its_configured_http_method_allowlist() {
    let proxy_mac = MacAddress::new([0, 1, 2, 3, 4, 6]);
    let mut proxy = ReverseProxyWaf::new(
        id("business-web-gw-01"),
        [RoutedInterface::new(
            port("dmz"),
            proxy_mac,
            [Ipv4InterfaceAddress::new(Ipv4Addr::new(172, 16, 10, 2), 24)
                .expect("interface address")],
            VlanId::new(10).expect("VLAN"),
            1_500,
        )],
        ["www.business.example".into()],
        id("internal-app-vip"),
        Ipv4Addr::new(172, 16, 10, 10),
    );
    proxy.set_allowed_methods([HttpMethod::Delete]);

    let allowed = proxy.handle(SimulationEvent::Network(NetworkIngress {
        port: port("dmz"),
        frame: request_frame(HttpMethod::Delete, proxy_mac, None),
        received_at_us: 0,
    }));
    assert!(matches!(allowed[0], Effect::ApplicationForward { .. }));

    let denied = proxy.handle(SimulationEvent::Network(NetworkIngress {
        port: port("dmz"),
        frame: request_frame(HttpMethod::Get, proxy_mac, None),
        received_at_us: 0,
    }));
    assert!(matches!(
        &denied[0],
        Effect::Drop(DropReason::ApplicationRejected(reason))
            if reason.contains("HTTP method is not allowed")
    ));
}

#[test]
fn reverse_proxy_applies_case_insensitive_body_inspection_rules() {
    let proxy_mac = MacAddress::new([0, 1, 2, 3, 4, 6]);
    let mut proxy = ReverseProxyWaf::new(
        id("business-web-gw-01"),
        [RoutedInterface::new(
            port("dmz"),
            proxy_mac,
            [Ipv4InterfaceAddress::new(Ipv4Addr::new(172, 16, 10, 2), 24)
                .expect("interface address")],
            VlanId::new(10).expect("VLAN"),
            1_500,
        )],
        ["www.business.example".into()],
        id("internal-app-vip"),
        Ipv4Addr::new(172, 16, 10, 10),
    );
    proxy.set_inspection_rules([HttpInspectionRule::new(
        HttpInspectionTarget::Body,
        "' OR '1'='1".into(),
        false,
        "SQL injection tautology pattern".into(),
    )]);

    let denied = proxy.handle(SimulationEvent::Network(NetworkIngress {
        port: port("dmz"),
        frame: request_frame(
            HttpMethod::Post,
            proxy_mac,
            Some("username=admin' or '1'='1"),
        ),
        received_at_us: 0,
    }));
    assert!(matches!(
        &denied[0],
        Effect::Drop(DropReason::ApplicationRejected(reason))
            if reason.contains("SQL injection tautology pattern")
    ));

    let allowed = proxy.handle(SimulationEvent::Network(NetworkIngress {
        port: port("dmz"),
        frame: request_frame(
            HttpMethod::Post,
            proxy_mac,
            Some("username=operator&password=valid"),
        ),
        received_at_us: 0,
    }));
    assert!(matches!(allowed[0], Effect::ApplicationForward { .. }));
}

#[test]
fn layer_three_switch_routes_between_trunks_through_svis() {
    let users = VlanId::new(30).expect("user VLAN");
    let servers = VlanId::new(80).expect("server VLAN");
    let user_gateway = MacAddress::new([0x02, 0, 0, 0, 30, 1]);
    let server_gateway = MacAddress::new([0x02, 0, 0, 0, 80, 1]);
    let user_mac = MacAddress::new([0x02, 0, 0, 0, 30, 101]);
    let server_mac = MacAddress::new([0x02, 0, 0, 0, 80, 20]);
    let user_ip = Ipv4Addr::new(10, 10, 30, 101);
    let server_ip = Ipv4Addr::new(10, 10, 80, 20);
    let mut switch = Layer3Switch::new(
        id("core-switch-01"),
        [
            SwitchPort::trunk(port("users"), [users]),
            SwitchPort::trunk(port("servers"), [servers]),
        ],
        [
            RoutedInterface::new(
                port("vlan-30"),
                user_gateway,
                [Ipv4InterfaceAddress::new(Ipv4Addr::new(10, 10, 30, 1), 24).expect("user SVI")],
                users,
                1_500,
            ),
            RoutedInterface::new(
                port("vlan-80"),
                server_gateway,
                [
                    Ipv4InterfaceAddress::new(Ipv4Addr::new(10, 10, 80, 1), 24)
                        .expect("server SVI"),
                ],
                servers,
                1_500,
            ),
        ],
        [port("vlan-30"), port("vlan-80")],
        RoutingTable::new([
            Route {
                destination: Ipv4Cidr::new(Ipv4Addr::new(10, 10, 30, 0), 24).expect("user route"),
                egress: port("vlan-30"),
                next_hop: None,
                metric: 0,
            },
            Route {
                destination: Ipv4Cidr::new(Ipv4Addr::new(10, 10, 80, 0), 24).expect("server route"),
                egress: port("vlan-80"),
                next_hop: None,
                metric: 0,
            },
        ]),
    );

    let gateway_reply = switch.handle(SimulationEvent::Network(NetworkIngress {
        port: port("users"),
        frame: EthernetFrame {
            source: user_mac,
            destination: MacAddress::BROADCAST,
            vlan: users,
            payload: NetworkPayload::Arp(ArpPacket {
                operation: ArpOperation::Request,
                sender_mac: user_mac,
                sender_ip: user_ip,
                target_mac: None,
                target_ip: Ipv4Addr::new(10, 10, 30, 1),
            }),
            wire_len_bytes: 64,
        },
        received_at_us: 0,
    }));
    assert!(matches!(
        &gateway_reply[0],
        Effect::Transmit { egress, frame, .. }
            if egress == &port("users")
                && frame.source == user_gateway
                && matches!(
                    frame.payload,
                    NetworkPayload::Arp(ArpPacket {
                        operation: ArpOperation::Reply,
                        ..
                    })
                )
    ));

    let resolution = switch.handle(SimulationEvent::Network(NetworkIngress {
        port: port("users"),
        frame: EthernetFrame {
            source: user_mac,
            destination: user_gateway,
            vlan: users,
            payload: NetworkPayload::Ipv4(Ipv4Packet {
                source: user_ip,
                destination: server_ip,
                ttl: 64,
                transport: Transport::Tcp(TcpSegment {
                    source_port: 50_000,
                    destination_port: 443,
                    flags: TcpFlags::default(),
                }),
                application: ApplicationData::None,
            }),
            wire_len_bytes: 128,
        },
        received_at_us: 10,
    }));
    assert!(matches!(
        &resolution[0],
        Effect::Transmit { egress, frame, .. }
            if egress == &port("servers")
                && frame.vlan == servers
                && matches!(
                    frame.payload,
                    NetworkPayload::Arp(ArpPacket {
                        operation: ArpOperation::Request,
                        target_ip,
                        ..
                    }) if target_ip == server_ip
                )
    ));

    let routed = switch.handle(SimulationEvent::Network(NetworkIngress {
        port: port("servers"),
        frame: EthernetFrame {
            source: server_mac,
            destination: server_gateway,
            vlan: servers,
            payload: NetworkPayload::Arp(ArpPacket {
                operation: ArpOperation::Reply,
                sender_mac: server_mac,
                sender_ip: server_ip,
                target_mac: Some(server_gateway),
                target_ip: Ipv4Addr::new(10, 10, 80, 1),
            }),
            wire_len_bytes: 64,
        },
        received_at_us: 20,
    }));
    assert!(matches!(
        &routed[0],
        Effect::Transmit { egress, frame, .. }
            if egress == &port("servers")
                && frame.source == server_gateway
                && frame.destination == server_mac
                && matches!(
                    frame.payload,
                    NetworkPayload::Ipv4(Ipv4Packet { ttl: 63, .. })
                )
    ));
    assert!(!switch.has_port(&port("vlan-30")));
}

#[test]
fn first_hop_gateway_answers_arp_only_while_active() {
    let users = VlanId::new(30).expect("user VLAN");
    let physical_mac = MacAddress::new([0x02, 0, 0, 0, 30, 2]);
    let virtual_mac = MacAddress::new([0x00, 0x00, 0x5e, 0x00, 0x01, 30]);
    let virtual_ip = Ipv4Addr::new(10, 10, 30, 1);
    let user_mac = MacAddress::new([0x02, 0, 0, 0, 30, 101]);
    let user_ip = Ipv4Addr::new(10, 10, 30, 101);
    let mut svi = RoutedInterface::new(
        port("vlan-30"),
        physical_mac,
        [Ipv4InterfaceAddress::new(Ipv4Addr::new(10, 10, 30, 2), 24)
            .expect("physical SVI address")],
        users,
        1_500,
    );
    svi.add_first_hop_address(FirstHopAddress::new(virtual_ip, virtual_mac, false))
        .expect("first-hop address");
    let mut switch = Layer3Switch::new(
        id("core-switch-01"),
        [SwitchPort::trunk(port("users"), [users])],
        [svi],
        [port("vlan-30")],
        RoutingTable::new([Route {
            destination: Ipv4Cidr::new(Ipv4Addr::new(10, 10, 30, 0), 24).expect("user route"),
            egress: port("vlan-30"),
            next_hop: None,
            metric: 0,
        }]),
    );
    let request = || NetworkIngress {
        port: port("users"),
        frame: EthernetFrame {
            source: user_mac,
            destination: MacAddress::BROADCAST,
            vlan: users,
            payload: NetworkPayload::Arp(ArpPacket {
                operation: ArpOperation::Request,
                sender_mac: user_mac,
                sender_ip: user_ip,
                target_mac: None,
                target_ip: virtual_ip,
            }),
            wire_len_bytes: 64,
        },
        received_at_us: 0,
    };

    let standby = switch.handle(SimulationEvent::Network(request()));
    assert!(matches!(standby[0], Effect::Observe { .. }));

    assert!(switch.set_first_hop_active(&port("vlan-30"), virtual_ip, true));
    let active = switch.handle(SimulationEvent::Network(request()));
    assert!(matches!(
        &active[0],
        Effect::Transmit { frame, .. }
            if frame.source == virtual_mac
                && matches!(
                    frame.payload,
                    NetworkPayload::Arp(ArpPacket {
                        operation: ArpOperation::Reply,
                        sender_ip,
                        ..
                    }) if sender_ip == virtual_ip
                )
    ));
}
