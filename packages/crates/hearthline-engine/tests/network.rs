use std::net::Ipv4Addr;

use hearthline_engine::{
    DnsServer, DropReason, Effect, FirstHopAddress, Ipv4Egress, LearningSwitch, NetworkIngress,
    PassiveSensor, ReverseProxyWaf, RoutedInterface, Router, RoutingTable, ServiceNode,
    SimulatedComponent, SimulationEvent, SwitchPort, UnaddressedNode,
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

#[test]
fn routed_interface_enforces_first_hop_identity_and_capacity() {
    let vlan = VlanId::new(10).unwrap();
    let primary_ip = Ipv4Addr::new(192, 0, 2, 1);
    let primary_mac = MacAddress::new([0x02, 0, 0, 0, 0, 1]);
    let mut interface = host_interface("inside", primary_mac, primary_ip, 24, vlan);

    for address in [
        Ipv4Addr::UNSPECIFIED,
        Ipv4Addr::BROADCAST,
        Ipv4Addr::new(224, 0, 0, 1),
    ] {
        assert!(
            std::panic::catch_unwind(|| {
                FirstHopAddress::new(address, MacAddress::new([0x02, 0, 0, 0, 1, 1]), true)
            })
            .is_err()
        );
    }
    assert!(
        std::panic::catch_unwind(|| {
            FirstHopAddress::new(Ipv4Addr::new(192, 0, 2, 2), MacAddress::BROADCAST, true)
        })
        .is_err()
    );

    assert_eq!(
        interface.add_first_hop_address(FirstHopAddress::new(
            Ipv4Addr::new(198, 51, 100, 2),
            MacAddress::new([0x02, 0, 0, 0, 1, 2]),
            true,
        )),
        Err("first-hop address is not on-link")
    );
    assert_eq!(
        interface.add_first_hop_address(FirstHopAddress::new(
            primary_ip,
            MacAddress::new([0x02, 0, 0, 0, 1, 3]),
            true,
        )),
        Err("first-hop address duplicates an interface address")
    );

    let virtual_ip = Ipv4Addr::new(192, 0, 2, 254);
    let virtual_mac = MacAddress::new([0x02, 0, 0, 0, 1, 254]);
    assert_eq!(
        interface.add_first_hop_address(FirstHopAddress::new(virtual_ip, virtual_mac, false,)),
        Ok(())
    );
    assert_eq!(
        interface.add_first_hop_address(FirstHopAddress::new(
            virtual_ip,
            MacAddress::new([0x02, 0, 0, 0, 1, 4]),
            true,
        )),
        Err("first-hop address or MAC is already configured")
    );
    assert_eq!(
        interface.add_first_hop_address(FirstHopAddress::new(
            Ipv4Addr::new(192, 0, 2, 253),
            virtual_mac,
            true,
        )),
        Err("first-hop address or MAC is already configured")
    );

    assert!(interface.owns_active_address(primary_ip));
    assert!(!interface.owns_active_address(virtual_ip));
    assert!(interface.accepts_destination_mac(primary_mac));
    assert!(!interface.accepts_destination_mac(virtual_mac));
    assert_eq!(interface.response_mac(primary_ip), Some(primary_mac));
    assert_eq!(interface.response_mac(virtual_ip), None);
    assert_eq!(interface.egress_identity(), Some((primary_ip, primary_mac)));
    assert!(!interface.set_first_hop_active(Ipv4Addr::new(192, 0, 2, 99), true));
    assert!(interface.set_first_hop_active(virtual_ip, true));
    assert!(interface.owns_active_address(virtual_ip));
    assert!(interface.accepts_destination_mac(virtual_mac));
    assert_eq!(interface.response_mac(virtual_ip), Some(virtual_mac));
    assert_eq!(interface.egress_identity(), Some((virtual_ip, virtual_mac)));
    assert!(interface.set_all_first_hop_active(false));
    assert_eq!(interface.egress_identity(), Some((primary_ip, primary_mac)));

    let mut capacity_reached = false;
    for suffix in 2..=253_u8 {
        let result = interface.add_first_hop_address(FirstHopAddress::new(
            Ipv4Addr::new(192, 0, 2, suffix),
            MacAddress::new([0x02, 0, 0, 1, 0, suffix]),
            true,
        ));
        if result == Err("first-hop address capacity exceeded") {
            capacity_reached = true;
            break;
        }
    }
    assert!(capacity_reached);

    let mut addressless = RoutedInterface::new(
        port("empty"),
        MacAddress::new([0x02, 0, 0, 0, 2, 1]),
        [],
        vlan,
        68,
    );
    assert_eq!(addressless.primary_address(), None);
    assert_eq!(addressless.egress_identity(), None);
    assert!(!addressless.set_all_first_hop_active(true));
    assert!(!addressless.is_on_link(primary_ip));
    assert!(addressless.accepts_wire_len(90));
    assert!(!addressless.accepts_wire_len(91));
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
fn passive_and_unaddressed_hosts_have_explicit_bounded_failure_behavior() {
    let vlan = VlanId::new(10).unwrap();
    let source = MacAddress::new([0x02, 0, 0, 0, 0, 1]);
    let destination = MacAddress::new([0x02, 0, 0, 0, 0, 2]);
    let ingress = |port_id: &str, frame| {
        SimulationEvent::Network(NetworkIngress {
            port: port(port_id),
            frame,
            received_at_us: 0,
        })
    };

    let mut sensor = PassiveSensor::new(id("sensor-01"), [port("capture")]);
    assert!(sensor.has_port(&port("capture")));
    assert!(!sensor.has_port(&port("missing")));
    assert!(matches!(
        sensor.handle(ingress("missing", frame(source, destination, vlan)))[0],
        Effect::Drop(DropReason::InvalidIngress(_))
    ));
    let mut malformed = frame(source, destination, vlan);
    malformed.wire_len_bytes = 63;
    assert_eq!(
        sensor.handle(ingress("capture", malformed)).as_slice(),
        &[Effect::Drop(DropReason::InvalidEthernetFrame)]
    );
    assert!(matches!(
        sensor.handle(ingress("capture", frame(source, destination, vlan)))[0],
        Effect::Observe { .. }
    ));
    assert_eq!(sensor.observation_count(), 1);
    assert!(matches!(
        sensor.handle(SimulationEvent::SetOperational(false))[0],
        Effect::Observe { .. }
    ));
    assert_eq!(
        sensor
            .handle(ingress("capture", frame(source, destination, vlan)))
            .as_slice(),
        &[Effect::Drop(DropReason::ComponentDown)]
    );
    assert_eq!(
        sensor
            .handle(SimulationEvent::Process(
                hearthline_model::ProcessEvent::Tick { elapsed_ms: 1 }
            ))
            .as_slice(),
        &[Effect::Drop(DropReason::UnsupportedProtocol)]
    );

    assert!(
        std::panic::catch_unwind(|| {
            UnaddressedNode::new(id("empty-node"), ComponentKind::Printer, [])
        })
        .is_err()
    );
    let mut node =
        UnaddressedNode::new(id("printer-01"), ComponentKind::Printer, [port("network")]);
    assert_eq!(node.kind(), ComponentKind::Printer);
    assert!(matches!(
        node.handle(ingress("missing", frame(source, destination, vlan)))[0],
        Effect::Drop(DropReason::InvalidIngress(_))
    ));
    assert!(matches!(
        node.handle(ingress("network", frame(source, destination, vlan)))[0],
        Effect::Drop(DropReason::NoInterfaceAddress(_))
    ));
    let packet = Ipv4Packet {
        source: Ipv4Addr::new(192, 0, 2, 1),
        destination: Ipv4Addr::new(198, 51, 100, 1),
        ttl: 64,
        transport: Transport::Icmp(IcmpMessage::EchoRequest {
            identifier: 1,
            sequence: 1,
        }),
        application: ApplicationData::None,
    };
    assert!(matches!(
        node.handle(SimulationEvent::Ipv4Egress(Ipv4Egress {
            packet,
            wire_len_bytes: 64,
            sent_at_us: 0,
        }))[0],
        Effect::Drop(DropReason::NoRoute(_))
    ));
    assert!(
        node.handle(SimulationEvent::SetOperational(false))
            .is_empty()
    );
    assert_eq!(
        node.handle(ingress("network", frame(source, destination, vlan)))
            .as_slice(),
        &[Effect::Drop(DropReason::ComponentDown)]
    );
    assert_eq!(
        node.handle(SimulationEvent::Process(
            hearthline_model::ProcessEvent::Tick { elapsed_ms: 1 }
        ))
        .as_slice(),
        &[Effect::Drop(DropReason::UnsupportedProtocol)]
    );
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
fn router_bounds_and_releases_packets_waiting_for_one_arp_resolution() {
    let inside_mac = MacAddress::new([0x02, 0, 0, 0, 0, 1]);
    let outside_mac = MacAddress::new([0x02, 0, 0, 0, 0, 2]);
    let neighbor_mac = MacAddress::new([0x02, 0, 0, 0, 0, 3]);
    let inside_vlan = VlanId::new(20).unwrap();
    let outside_vlan = VlanId::new(100).unwrap();
    let neighbor = Ipv4Addr::new(203, 0, 113, 1);
    let destination = Ipv4Addr::new(198, 51, 100, 20);
    let mut router = Router::new(
        id("router-arp-queue"),
        ComponentKind::Router,
        [
            host_interface(
                "inside",
                inside_mac,
                Ipv4Addr::new(192, 0, 2, 1),
                24,
                inside_vlan,
            ),
            host_interface(
                "outside",
                outside_mac,
                Ipv4Addr::new(203, 0, 113, 2),
                24,
                outside_vlan,
            ),
        ],
        RoutingTable::new([Route {
            destination: Ipv4Cidr::new(Ipv4Addr::UNSPECIFIED, 0).unwrap(),
            egress: port("outside"),
            next_hop: Some(neighbor),
            metric: 1,
        }]),
    );
    let packet = |sequence| EthernetFrame {
        source: MacAddress::new([0x02, 0, 0, 0, 1, sequence]),
        destination: inside_mac,
        vlan: inside_vlan,
        payload: NetworkPayload::Ipv4(Ipv4Packet {
            source: Ipv4Addr::new(192, 0, 2, sequence),
            destination,
            ttl: 64,
            transport: Transport::Icmp(IcmpMessage::EchoRequest {
                identifier: 1,
                sequence: u16::from(sequence),
            }),
            application: ApplicationData::None,
        }),
        wire_len_bytes: 64,
    };

    for sequence in 1..=17 {
        let effects = router.handle(SimulationEvent::Network(NetworkIngress {
            port: port("inside"),
            frame: packet(sequence),
            received_at_us: u64::from(sequence),
        }));
        match sequence {
            1 => assert!(matches!(effects[0], Effect::Transmit { .. })),
            2..=16 => assert!(matches!(effects[0], Effect::Observe { .. })),
            17 => assert!(matches!(
                effects[0],
                Effect::Drop(DropReason::NeighborQueueFull)
            )),
            _ => unreachable!(),
        }
    }

    let reply = || EthernetFrame {
        source: neighbor_mac,
        destination: outside_mac,
        vlan: outside_vlan,
        payload: NetworkPayload::Arp(ArpPacket {
            operation: ArpOperation::Reply,
            sender_mac: neighbor_mac,
            sender_ip: neighbor,
            target_mac: Some(outside_mac),
            target_ip: Ipv4Addr::new(203, 0, 113, 2),
        }),
        wire_len_bytes: 64,
    };
    let released = router.handle(SimulationEvent::Network(NetworkIngress {
        port: port("outside"),
        frame: reply(),
        received_at_us: 100,
    }));
    assert_eq!(released.len(), 16);
    assert!(
        released
            .iter()
            .all(|effect| matches!(effect, Effect::Transmit {
        egress,
        next_hop: Some(address),
        ..
    } if egress == &port("outside") && *address == neighbor))
    );

    let no_pending = router.handle(SimulationEvent::Network(NetworkIngress {
        port: port("outside"),
        frame: reply(),
        received_at_us: 101,
    }));
    assert!(matches!(no_pending[0], Effect::Observe { .. }));
}

#[test]
fn router_forwarding_rejects_malformed_ingress_and_invalid_transit_routes() {
    let inside_ip = Ipv4Addr::new(192, 168, 0, 1);
    let inside_mac = MacAddress::new([0x02, 0, 0, 0, 0, 1]);
    let outside_ip = Ipv4Addr::new(203, 0, 113, 2);
    let outside_mac = MacAddress::new([0x02, 0, 0, 0, 0, 2]);
    let client_ip = Ipv4Addr::new(192, 168, 0, 10);
    let client_mac = MacAddress::new([0x02, 0, 0, 0, 0, 10]);
    let inside_vlan = VlanId::new(20).unwrap();
    let outside_vlan = VlanId::new(100).unwrap();
    let destination = Ipv4Addr::new(10, 10, 20, 10);
    let default_interfaces = || {
        [
            host_interface("inside", inside_mac, inside_ip, 24, inside_vlan),
            host_interface("outside", outside_mac, outside_ip, 24, outside_vlan),
        ]
    };
    let route = |egress: &str, next_hop| Route {
        destination: Ipv4Cidr::new(Ipv4Addr::new(10, 10, 20, 0), 24).unwrap(),
        egress: port(egress),
        next_hop,
        metric: 1,
    };
    let mut router = Router::new(
        id("router-edge-cases"),
        ComponentKind::Router,
        default_interfaces(),
        RoutingTable::new([route("outside", Some(Ipv4Addr::new(203, 0, 113, 1)))]),
    );
    let transit_frame = |destination_ip: Ipv4Addr| EthernetFrame {
        source: client_mac,
        destination: inside_mac,
        vlan: inside_vlan,
        payload: NetworkPayload::Ipv4(Ipv4Packet {
            source: client_ip,
            destination: destination_ip,
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
    let ingress = |port_id: &str, frame: EthernetFrame| {
        SimulationEvent::Network(NetworkIngress {
            port: port(port_id),
            frame,
            received_at_us: 10,
        })
    };
    let dropped = |effects: &[Effect], expected: fn(&DropReason) -> bool| {
        assert!(
            matches!(&effects[0], Effect::Drop(reason) if expected(reason)),
            "unexpected router effect: {effects:?}"
        );
    };

    dropped(
        &router.handle(ingress("missing", transit_frame(destination))),
        |reason| matches!(reason, DropReason::InvalidIngress(_)),
    );
    let mut wrong_vlan = transit_frame(destination);
    wrong_vlan.vlan = outside_vlan;
    dropped(&router.handle(ingress("inside", wrong_vlan)), |reason| {
        matches!(reason, DropReason::VlanNotAllowed(100))
    });
    let mut short = transit_frame(destination);
    short.wire_len_bytes = 63;
    dropped(&router.handle(ingress("inside", short)), |reason| {
        matches!(reason, DropReason::InvalidEthernetFrame)
    });
    let mut oversized = transit_frame(destination);
    oversized.wire_len_bytes = 1_523;
    dropped(&router.handle(ingress("inside", oversized)), |reason| {
        matches!(reason, DropReason::InterfaceMtuExceeded { .. })
    });
    let mut invalid_source_mac = transit_frame(destination);
    invalid_source_mac.source = MacAddress::BROADCAST;
    dropped(
        &router.handle(ingress("inside", invalid_source_mac)),
        |reason| matches!(reason, DropReason::InvalidSourceMac(_)),
    );
    let mut wrong_destination_mac = transit_frame(destination);
    wrong_destination_mac.destination = outside_mac;
    dropped(
        &router.handle(ingress("inside", wrong_destination_mac)),
        |reason| matches!(reason, DropReason::L2DestinationMismatch { .. }),
    );
    for source in [
        Ipv4Addr::UNSPECIFIED,
        Ipv4Addr::BROADCAST,
        Ipv4Addr::new(224, 0, 0, 1),
    ] {
        let mut invalid_source = transit_frame(destination);
        let NetworkPayload::Ipv4(packet) = &mut invalid_source.payload else {
            unreachable!()
        };
        packet.source = source;
        dropped(
            &router.handle(ingress("inside", invalid_source)),
            |reason| matches!(reason, DropReason::InvalidSourceIp(_)),
        );
    }
    let mut unsupported = transit_frame(destination);
    unsupported.payload =
        NetworkPayload::FirewallHa(hearthline_model::FirewallHaMessage::Heartbeat {
            domain: "ha".into(),
            sequence: 1,
            sent_at_us: 0,
        });
    dropped(&router.handle(ingress("inside", unsupported)), |reason| {
        matches!(reason, DropReason::UnsupportedProtocol)
    });

    let mut local_tcp = transit_frame(inside_ip);
    dropped(
        &router.handle(ingress("inside", local_tcp.clone())),
        |reason| matches!(reason, DropReason::UnsupportedProtocol),
    );
    let NetworkPayload::Ipv4(packet) = &mut local_tcp.payload else {
        unreachable!()
    };
    packet.transport = Transport::Icmp(IcmpMessage::EchoRequest {
        identifier: 7,
        sequence: 9,
    });
    assert!(matches!(
        router.handle(ingress("inside", local_tcp))[0],
        Effect::Transmit { .. }
    ));

    let mut ttl = transit_frame(destination);
    let NetworkPayload::Ipv4(packet) = &mut ttl.payload else {
        unreachable!()
    };
    packet.ttl = 1;
    dropped(&router.handle(ingress("inside", ttl)), |reason| {
        matches!(reason, DropReason::TtlExpired)
    });

    let mut no_route = Router::new(
        id("router-no-route"),
        ComponentKind::Router,
        default_interfaces(),
        RoutingTable::default(),
    );
    dropped(
        &no_route.handle(ingress("inside", transit_frame(destination))),
        |reason| matches!(reason, DropReason::NoRoute(_)),
    );
    let mut missing_egress = Router::new(
        id("router-missing-egress"),
        ComponentKind::Router,
        default_interfaces(),
        RoutingTable::new([route("missing", None)]),
    );
    dropped(
        &missing_egress.handle(ingress("inside", transit_frame(destination))),
        |reason| matches!(reason, DropReason::InvalidIngress(_)),
    );
    let mut off_link = Router::new(
        id("router-off-link"),
        ComponentKind::Router,
        default_interfaces(),
        RoutingTable::new([route("outside", Some(Ipv4Addr::new(198, 51, 100, 1)))]),
    );
    dropped(
        &off_link.handle(ingress("inside", transit_frame(destination))),
        |reason| matches!(reason, DropReason::NextHopOffLink { .. }),
    );
    let mut down_interfaces = default_interfaces();
    down_interfaces[1].forwarding = false;
    let mut down_egress = Router::new(
        id("router-down-egress"),
        ComponentKind::Router,
        down_interfaces,
        RoutingTable::new([route("outside", Some(Ipv4Addr::new(203, 0, 113, 1)))]),
    );
    dropped(
        &down_egress.handle(ingress("inside", transit_frame(destination))),
        |reason| matches!(reason, DropReason::PortDown(_)),
    );
    let mut narrow_interfaces = default_interfaces();
    narrow_interfaces[1].mtu = 68;
    let mut narrow_egress = Router::new(
        id("router-narrow-egress"),
        ComponentKind::Router,
        narrow_interfaces,
        RoutingTable::new([route("outside", Some(Ipv4Addr::new(203, 0, 113, 1)))]),
    );
    let mut large = transit_frame(destination);
    large.wire_len_bytes = 100;
    dropped(&narrow_egress.handle(ingress("inside", large)), |reason| {
        matches!(reason, DropReason::InterfaceMtuExceeded { .. })
    });

    assert!(matches!(
        router.handle(SimulationEvent::SetOperational(false))[0],
        Effect::Observe { .. }
    ));
    dropped(
        &router.handle(ingress("inside", transit_frame(destination))),
        |reason| matches!(reason, DropReason::ComponentDown),
    );
    dropped(
        &router.handle(SimulationEvent::Process(
            hearthline_model::ProcessEvent::Tick { elapsed_ms: 1 },
        )),
        |reason| matches!(reason, DropReason::UnsupportedProtocol),
    );
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
fn endpoint_stack_rejects_malformed_paths_and_bounds_neighbor_resolution() {
    let vlan = VlanId::new(10).expect("VLAN");
    let server_ip = Ipv4Addr::new(192, 0, 2, 10);
    let server_mac = MacAddress::new([0x02, 0, 0, 0, 0, 10]);
    let client_ip = Ipv4Addr::new(192, 0, 2, 20);
    let client_mac = MacAddress::new([0x02, 0, 0, 0, 0, 20]);
    let interface = host_interface("network", server_mac, server_ip, 24, vlan);
    let mut server = ServiceNode::new(
        id("service-edge-cases"),
        ComponentKind::ServiceCluster,
        [interface.clone()],
        [hearthline_model::ServiceKind::Https],
    );

    let ingress = |port_id: &str, frame: EthernetFrame| {
        SimulationEvent::Network(NetworkIngress {
            port: port(port_id),
            frame,
            received_at_us: 10,
        })
    };
    let ipv4_frame = |source_ip: Ipv4Addr, destination_ip: Ipv4Addr| EthernetFrame {
        source: client_mac,
        destination: server_mac,
        vlan,
        payload: NetworkPayload::Ipv4(Ipv4Packet {
            source: source_ip,
            destination: destination_ip,
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
    let dropped = |effects: &[Effect], expected: fn(&DropReason) -> bool| {
        assert!(
            matches!(&effects[0], Effect::Drop(reason) if expected(reason)),
            "unexpected endpoint effect: {effects:?}"
        );
    };

    let effects = server.handle(ingress("missing", ipv4_frame(client_ip, server_ip)));
    dropped(&effects, |reason| {
        matches!(reason, DropReason::InvalidIngress(_))
    });

    let mut wrong_vlan = ipv4_frame(client_ip, server_ip);
    wrong_vlan.vlan = VlanId::new(11).unwrap();
    dropped(&server.handle(ingress("network", wrong_vlan)), |reason| {
        matches!(reason, DropReason::VlanNotAllowed(11))
    });

    let mut short = ipv4_frame(client_ip, server_ip);
    short.wire_len_bytes = 20;
    dropped(&server.handle(ingress("network", short)), |reason| {
        matches!(reason, DropReason::InvalidEthernetFrame)
    });

    let mut oversized = ipv4_frame(client_ip, server_ip);
    oversized.wire_len_bytes = 1_523;
    dropped(&server.handle(ingress("network", oversized)), |reason| {
        matches!(reason, DropReason::InterfaceMtuExceeded { .. })
    });

    let mut broadcast_source = ipv4_frame(client_ip, server_ip);
    broadcast_source.source = MacAddress::BROADCAST;
    dropped(
        &server.handle(ingress("network", broadcast_source)),
        |reason| matches!(reason, DropReason::InvalidSourceMac(_)),
    );

    for source in [
        Ipv4Addr::UNSPECIFIED,
        Ipv4Addr::BROADCAST,
        Ipv4Addr::new(224, 0, 0, 1),
    ] {
        dropped(
            &server.handle(ingress("network", ipv4_frame(source, server_ip))),
            |reason| matches!(reason, DropReason::InvalidSourceIp(_)),
        );
    }
    dropped(
        &server.handle(ingress(
            "network",
            ipv4_frame(client_ip, Ipv4Addr::new(192, 0, 2, 99)),
        )),
        |reason| matches!(reason, DropReason::NotAddressedToComponent),
    );

    let originated = |destination: Ipv4Addr, source: Ipv4Addr, ttl: u8, wire_len_bytes: u16| {
        SimulationEvent::Ipv4Egress(Ipv4Egress {
            packet: Ipv4Packet {
                source,
                destination,
                ttl,
                transport: Transport::Udp(UdpDatagram {
                    source_port: 10_000,
                    destination_port: 53,
                }),
                application: ApplicationData::None,
            },
            wire_len_bytes,
            sent_at_us: 20,
        })
    };
    dropped(
        &server.handle(originated(client_ip, server_ip, 64, 20)),
        |reason| matches!(reason, DropReason::InvalidEthernetFrame),
    );
    dropped(
        &server.handle(originated(client_ip, server_ip, 0, 64)),
        |reason| matches!(reason, DropReason::TtlExpired),
    );
    dropped(
        &server.handle(originated(
            Ipv4Addr::new(198, 51, 100, 1),
            server_ip,
            64,
            64,
        )),
        |reason| matches!(reason, DropReason::NoRoute(_)),
    );
    dropped(
        &server.handle(originated(client_ip, Ipv4Addr::new(192, 0, 2, 11), 64, 64)),
        |reason| matches!(reason, DropReason::InvalidSourceIp(_)),
    );
    dropped(
        &server.handle(originated(client_ip, server_ip, 64, 1_523)),
        |reason| matches!(reason, DropReason::InterfaceMtuExceeded { .. }),
    );

    let first = server.handle(originated(client_ip, server_ip, 64, 64));
    assert!(matches!(first[0], Effect::Transmit { .. }));
    let second = server.handle(originated(client_ip, server_ip, 64, 64));
    assert!(matches!(second[0], Effect::Observe { .. }));
    for sequence in 0..32_u8 {
        let destination = Ipv4Addr::new(192, 0, 2, 100_u8.saturating_add(sequence));
        let effects = server.handle(originated(destination, server_ip, 64, 64));
        if matches!(effects[0], Effect::Drop(DropReason::NeighborQueueFull)) {
            break;
        }
    }
    assert!(matches!(
        server.handle(originated(Ipv4Addr::new(192, 0, 2, 250), server_ip, 64, 64))[0],
        Effect::Drop(DropReason::NeighborQueueFull)
    ));

    let arp =
        |operation, sender_ip, sender_mac, target_ip, target_mac, destination| EthernetFrame {
            source: sender_mac,
            destination,
            vlan,
            payload: NetworkPayload::Arp(ArpPacket {
                operation,
                sender_mac,
                sender_ip,
                target_mac,
                target_ip,
            }),
            wire_len_bytes: EthernetFrame::MIN_WIRE_LEN_BYTES,
        };
    let valid_reply = arp(
        ArpOperation::Reply,
        client_ip,
        client_mac,
        server_ip,
        Some(server_mac),
        server_mac,
    );
    assert!(matches!(
        server.handle(ingress("network", valid_reply.clone()))[0],
        Effect::Transmit { .. }
    ));
    assert!(matches!(
        server.handle(ingress("network", valid_reply))[0],
        Effect::Observe { .. }
    ));

    let unknown_request = arp(
        ArpOperation::Request,
        client_ip,
        client_mac,
        Ipv4Addr::new(192, 0, 2, 99),
        None,
        MacAddress::BROADCAST,
    );
    assert!(matches!(
        server.handle(ingress("network", unknown_request))[0],
        Effect::Observe { .. }
    ));
    let zero_sender = arp(
        ArpOperation::Request,
        Ipv4Addr::UNSPECIFIED,
        client_mac,
        server_ip,
        None,
        MacAddress::BROADCAST,
    );
    assert!(matches!(
        server.handle(ingress("network", zero_sender))[0],
        Effect::Transmit { .. }
    ));

    for invalid in [
        arp(
            ArpOperation::Request,
            client_ip,
            client_mac,
            server_ip,
            Some(server_mac),
            MacAddress::BROADCAST,
        ),
        arp(
            ArpOperation::Request,
            client_ip,
            client_mac,
            server_ip,
            None,
            MacAddress::new([2, 9, 9, 9, 9, 9]),
        ),
        arp(
            ArpOperation::Request,
            Ipv4Addr::new(198, 51, 100, 1),
            client_mac,
            server_ip,
            None,
            MacAddress::BROADCAST,
        ),
        arp(
            ArpOperation::Reply,
            Ipv4Addr::UNSPECIFIED,
            client_mac,
            server_ip,
            Some(server_mac),
            server_mac,
        ),
        arp(
            ArpOperation::Reply,
            client_ip,
            client_mac,
            server_ip,
            None,
            server_mac,
        ),
        arp(
            ArpOperation::Reply,
            client_ip,
            client_mac,
            server_ip,
            Some(server_mac),
            MacAddress::BROADCAST,
        ),
        arp(
            ArpOperation::Reply,
            client_ip,
            client_mac,
            Ipv4Addr::new(192, 0, 2, 99),
            Some(server_mac),
            server_mac,
        ),
    ] {
        dropped(&server.handle(ingress("network", invalid)), |reason| {
            matches!(reason, DropReason::InvalidArp)
        });
    }

    let mut down_interface = interface;
    down_interface.forwarding = false;
    let mut down = ServiceNode::new(
        id("down-service"),
        ComponentKind::ServiceCluster,
        [down_interface],
        [],
    );
    dropped(
        &down.handle(ingress("network", ipv4_frame(client_ip, server_ip))),
        |reason| matches!(reason, DropReason::PortDown(_)),
    );
    dropped(
        &down.handle(originated(client_ip, server_ip, 64, 64)),
        |reason| matches!(reason, DropReason::PortDown(_)),
    );
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
