use std::net::Ipv4Addr;

use hearthline_engine::{
    DropReason, Effect, FirewallAction, FirewallHaControl, FirewallHaRuntimeConfig, FirewallRule,
    NatRouter, NetworkIngress, RoutedInterface, RoutingTable, SimulatedComponent, SimulationEvent,
    StatefulFirewall, StaticNat, StaticNatError,
};
use hearthline_model::{
    ApplicationData, ArpOperation, ArpPacket, ComponentId, EthernetFrame, IcmpMessage, Ipv4Cidr,
    Ipv4InterfaceAddress, Ipv4Packet, MacAddress, NetworkPayload, PortId, ProcessEvent, Route,
    TcpFlags, TcpSegment, Transport, TransportProtocol, UdpDatagram, VlanId,
};
fn id(value: &str) -> ComponentId {
    ComponentId::new(value).expect("test ID")
}
fn port(value: &str) -> PortId {
    PortId::new(value).expect("test port")
}
fn tcp_frame(
    source: Ipv4Addr,
    destination: Ipv4Addr,
    source_port: u16,
    destination_port: u16,
    source_mac: MacAddress,
    destination_mac: MacAddress,
) -> EthernetFrame {
    EthernetFrame {
        source: source_mac,
        destination: destination_mac,
        vlan: VlanId::new(10).expect("VLAN"),
        payload: NetworkPayload::Ipv4(Ipv4Packet {
            source,
            destination,
            ttl: 64,
            transport: Transport::Tcp(TcpSegment {
                source_port,
                destination_port,
                flags: TcpFlags {
                    syn: true,
                    ..TcpFlags::default()
                },
            }),
            application: ApplicationData::None,
        }),
        wire_len_bytes: 64,
    }
}
fn with_tcp_flags(mut frame: EthernetFrame, flags: TcpFlags) -> EthernetFrame {
    let NetworkPayload::Ipv4(packet) = &mut frame.payload else {
        panic!("test frame must contain IPv4");
    };
    let Transport::Tcp(segment) = &mut packet.transport else {
        panic!("test frame must contain TCP");
    };
    segment.flags = flags;
    frame
}
fn routed_interface(port_id: &str, mac: MacAddress, address: Ipv4Addr) -> RoutedInterface {
    RoutedInterface::new(
        port(port_id),
        mac,
        [Ipv4InterfaceAddress::new(address, 24).expect("interface address")],
        VlanId::new(10).expect("VLAN"),
        1_500,
    )
}

fn arp_request(
    port_id: &str,
    sender_mac: MacAddress,
    sender_ip: Ipv4Addr,
    target_ip: Ipv4Addr,
) -> SimulationEvent {
    SimulationEvent::Network(NetworkIngress {
        port: port(port_id),
        frame: EthernetFrame {
            source: sender_mac,
            destination: MacAddress::BROADCAST,
            vlan: VlanId::new(10).expect("VLAN"),
            payload: NetworkPayload::Arp(ArpPacket {
                operation: ArpOperation::Request,
                sender_mac,
                sender_ip,
                target_mac: None,
                target_ip,
            }),
            wire_len_bytes: EthernetFrame::MIN_WIRE_LEN_BYTES,
        },
        received_at_us: 0,
    })
}

fn edge_routes(internal: Ipv4Cidr) -> RoutingTable {
    RoutingTable::new([
        Route {
            destination: internal,
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
    ])
}

fn ha_config(active: bool, sync_mac: MacAddress) -> FirewallHaRuntimeConfig {
    FirewallHaRuntimeConfig::new(
        "test-firewall-ha".into(),
        port("ha-sync"),
        sync_mac,
        [port("outside"), port("dmz")],
        active,
        true,
        250_000,
        750_000,
    )
}

#[test]
fn firewall_permits_https_tracks_reverse_state_and_defaults_to_deny() {
    let dmz = Ipv4Cidr::new(Ipv4Addr::new(172, 16, 10, 0), 24).expect("DMZ");
    let outside_mac = MacAddress::new([0x02, 0, 0, 0, 0, 1]);
    let dmz_mac = MacAddress::new([0x02, 0, 0, 0, 0, 2]);
    let client_mac = MacAddress::new([0x02, 0, 0, 0, 0, 10]);
    let server_mac = MacAddress::new([0x02, 0, 0, 0, 0, 20]);
    let mut firewall = StatefulFirewall::new(
        id("business-frw-01a"),
        [
            (port("outside"), "outside".into()),
            (port("dmz"), "dmz".into()),
        ],
        [
            routed_interface("outside", outside_mac, Ipv4Addr::new(203, 0, 113, 1)),
            routed_interface("dmz", dmz_mac, Ipv4Addr::new(172, 16, 10, 1)),
        ],
        RoutingTable::new([
            Route {
                destination: dmz,
                egress: port("dmz"),
                next_hop: None,
                metric: 0,
            },
            Route {
                destination: Ipv4Cidr::new(Ipv4Addr::UNSPECIFIED, 0).expect("default"),
                egress: port("outside"),
                next_hop: None,
                metric: 10,
            },
        ]),
        [FirewallRule {
            id: "allow-public-https".into(),
            source_zone: Some("outside".into()),
            destination_zone: Some("dmz".into()),
            source: None,
            destination: Some(dmz),
            protocol: Some(TransportProtocol::Tcp),
            destination_port: Some(443),
            action: FirewallAction::Permit,
        }],
    );
    let client = Ipv4Addr::new(203, 0, 113, 2);
    let server = Ipv4Addr::new(172, 16, 10, 2);
    firewall.handle(arp_request(
        "outside",
        client_mac,
        client,
        Ipv4Addr::new(203, 0, 113, 1),
    ));
    firewall.handle(arp_request(
        "dmz",
        server_mac,
        server,
        Ipv4Addr::new(172, 16, 10, 1),
    ));
    let mut standby = firewall.clone();
    firewall.configure_ha(ha_config(true, MacAddress::new([0x02, 0, 0, 0, 0xa0, 1])));
    standby.configure_ha(ha_config(false, MacAddress::new([0x02, 0, 0, 0, 0xb0, 1])));
    assert!(firewall.has_port(&port("ha-sync")));
    let allowed = firewall.handle(SimulationEvent::Network(NetworkIngress {
        port: port("outside"),
        frame: tcp_frame(client, server, 50_000, 443, client_mac, outside_mac),
        received_at_us: 0,
    }));
    assert!(matches!(allowed[1], Effect::Transmit { .. }));
    assert_eq!(firewall.session_count(), 1);
    assert_eq!(standby.session_count(), 0);
    let blocked = standby.handle(SimulationEvent::Network(NetworkIngress {
        port: port("dmz"),
        frame: with_tcp_flags(
            tcp_frame(server, client, 443, 50_000, server_mac, dmz_mac),
            TcpFlags {
                ack: true,
                ..TcpFlags::default()
            },
        ),
        received_at_us: 0,
    }));
    assert_eq!(
        blocked.as_slice(),
        &[Effect::Drop(DropReason::FirewallStandby)]
    );
    for effect in &allowed {
        let Effect::Transmit { egress, frame, .. } = effect else {
            continue;
        };
        if *egress == port("ha-sync") {
            standby.handle(SimulationEvent::Network(NetworkIngress {
                port: port("ha-sync"),
                frame: frame.clone(),
                received_at_us: 1_000,
            }));
        }
    }
    let synchronized = standby.ha_status().expect("HA status");
    assert_eq!(synchronized.session_count, 1);
    assert_eq!(synchronized.replicated_updates, 1);
    assert_eq!(synchronized.last_heartbeat_us, Some(1_000));
    let mut state_lost = standby.clone();
    let cleared = state_lost.handle(SimulationEvent::FirewallHa(
        FirewallHaControl::ClearReplicatedSessions { at_us: 500_000 },
    ));
    assert!(matches!(cleared[0], Effect::Observe { .. }));
    assert_eq!(state_lost.session_count(), 0);
    assert_eq!(
        state_lost.ha_status().expect("HA status").last_heartbeat_us,
        Some(1_000)
    );
    standby.handle(SimulationEvent::FirewallHa(
        FirewallHaControl::EvaluatePeer {
            at_us: 751_000,
            peer_failure_confirmed: true,
        },
    ));
    assert!(standby.ha_active());
    let replicated_return = standby.handle(SimulationEvent::Network(NetworkIngress {
        port: port("dmz"),
        frame: with_tcp_flags(
            tcp_frame(server, client, 443, 50_000, server_mac, dmz_mac),
            TcpFlags {
                ack: true,
                ..TcpFlags::default()
            },
        ),
        received_at_us: 0,
    }));
    assert!(matches!(replicated_return[1], Effect::Transmit { .. }));
    let returned = firewall.handle(SimulationEvent::Network(NetworkIngress {
        port: port("dmz"),
        frame: with_tcp_flags(
            tcp_frame(server, client, 443, 50_000, server_mac, dmz_mac),
            TcpFlags {
                ack: true,
                ..TcpFlags::default()
            },
        ),
        received_at_us: 0,
    }));
    assert!(matches!(returned[1], Effect::Transmit { .. }));
    let invalid_ack = firewall.handle(SimulationEvent::Network(NetworkIngress {
        port: port("outside"),
        frame: with_tcp_flags(
            tcp_frame(client, server, 50_001, 443, client_mac, outside_mac),
            TcpFlags {
                ack: true,
                ..TcpFlags::default()
            },
        ),
        received_at_us: 1,
    }));
    assert_eq!(
        invalid_ack.as_slice(),
        &[Effect::Drop(DropReason::InvalidTcpState)]
    );
    let expired = firewall.handle(SimulationEvent::Network(NetworkIngress {
        port: port("dmz"),
        frame: with_tcp_flags(
            tcp_frame(server, client, 443, 50_000, server_mac, dmz_mac),
            TcpFlags {
                ack: true,
                ..TcpFlags::default()
            },
        ),
        received_at_us: 300_000_001,
    }));
    assert!(matches!(expired[0], Effect::Observe { .. }));
    assert!(matches!(
        expired[1],
        Effect::Drop(DropReason::PolicyDenied { rule: None })
    ));
    assert_eq!(firewall.session_count(), 0);
    let denied = firewall.handle(SimulationEvent::Network(NetworkIngress {
        port: port("outside"),
        frame: tcp_frame(client, server, 50_001, 22, client_mac, outside_mac),
        received_at_us: 0,
    }));
    assert!(matches!(
        denied[0],
        Effect::Drop(DropReason::PolicyDenied { rule: None })
    ));
}

#[test]
fn pat_creates_state_and_restores_return_destination() {
    let inside = Ipv4Cidr::new(Ipv4Addr::new(192, 168, 0, 0), 24).expect("inside");
    let outside = Ipv4Addr::new(203, 0, 113, 2);
    let internal = Ipv4Addr::new(192, 168, 0, 2);
    let remote = Ipv4Addr::new(192, 0, 2, 10);
    let inside_mac = MacAddress::new([0x02, 0, 0, 0, 0, 1]);
    let outside_mac = MacAddress::new([0x02, 0, 0, 0, 0, 2]);
    let internal_mac = MacAddress::new([0x02, 0, 0, 0, 0, 10]);
    let gateway_mac = MacAddress::new([0x02, 0, 0, 0, 0, 20]);
    let mut nat = NatRouter::new(
        id("customer-rtr-01"),
        [
            routed_interface("inside", inside_mac, Ipv4Addr::new(192, 168, 0, 1)),
            routed_interface("outside", outside_mac, outside),
        ],
        [port("inside")],
        outside,
        edge_routes(inside),
    );
    nat.handle(arp_request(
        "inside",
        internal_mac,
        internal,
        Ipv4Addr::new(192, 168, 0, 1),
    ));
    nat.handle(arp_request(
        "outside",
        gateway_mac,
        Ipv4Addr::new(203, 0, 113, 1),
        outside,
    ));
    let outbound = nat.handle(SimulationEvent::Network(NetworkIngress {
        port: port("inside"),
        frame: tcp_frame(internal, remote, 50_000, 443, internal_mac, inside_mac),
        received_at_us: 0,
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

    let forged = nat.handle(SimulationEvent::Network(NetworkIngress {
        port: port("outside"),
        frame: with_tcp_flags(
            tcp_frame(
                Ipv4Addr::new(198, 51, 100, 9),
                outside,
                443,
                external_port,
                gateway_mac,
                outside_mac,
            ),
            TcpFlags {
                ack: true,
                ..TcpFlags::default()
            },
        ),
        received_at_us: 1,
    }));
    assert_eq!(
        forged.as_slice(),
        &[Effect::Drop(DropReason::NoTranslation)]
    );

    let mut return_frame = tcp_frame(
        remote,
        outside,
        443,
        external_port,
        gateway_mac,
        outside_mac,
    );
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
        received_at_us: 0,
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

    let expired = nat.handle(SimulationEvent::Network(NetworkIngress {
        port: port("outside"),
        frame: with_tcp_flags(
            tcp_frame(
                remote,
                outside,
                443,
                external_port,
                gateway_mac,
                outside_mac,
            ),
            TcpFlags {
                ack: true,
                ..TcpFlags::default()
            },
        ),
        received_at_us: 300_000_001,
    }));
    assert_eq!(
        expired.as_slice(),
        &[Effect::Drop(DropReason::NoTranslation)]
    );
    assert_eq!(nat.translation_count(), 0);
}

#[test]
fn static_nat_answers_proxy_arp_for_an_on_link_public_address() {
    let inside = Ipv4Cidr::new(Ipv4Addr::new(192, 168, 0, 0), 24).expect("inside");
    let outside = Ipv4Addr::new(203, 0, 113, 2);
    let published = Ipv4Addr::new(203, 0, 113, 10);
    let private = Ipv4Addr::new(192, 168, 0, 10);
    let outside_mac = MacAddress::new([0x02, 0, 0, 0, 0, 2]);
    let upstream_mac = MacAddress::new([0x02, 0, 0, 0, 0, 20]);
    let mut nat = NatRouter::new(
        id("customer-rtr-01"),
        [
            routed_interface(
                "inside",
                MacAddress::new([0x02, 0, 0, 0, 0, 1]),
                Ipv4Addr::new(192, 168, 0, 1),
            ),
            routed_interface("outside", outside_mac, outside),
        ],
        [port("inside")],
        outside,
        edge_routes(inside),
    );
    nat.add_static_nat(StaticNat {
        public_address: published,
        private_address: private,
    })
    .expect("valid static mapping");

    let effects = nat.handle(arp_request(
        "outside",
        upstream_mac,
        Ipv4Addr::new(203, 0, 113, 1),
        published,
    ));
    let Effect::Transmit {
        egress,
        frame,
        next_hop,
        ..
    } = &effects[0]
    else {
        panic!("expected proxy ARP reply");
    };
    assert_eq!(egress, &port("outside"));
    assert_eq!(*next_hop, Some(Ipv4Addr::new(203, 0, 113, 1)));
    assert_eq!(frame.source, outside_mac);
    assert_eq!(frame.destination, upstream_mac);
    let NetworkPayload::Arp(reply) = frame.payload else {
        panic!("expected ARP reply");
    };
    assert_eq!(reply.operation, ArpOperation::Reply);
    assert_eq!(reply.sender_ip, published);
    assert_eq!(reply.sender_mac, outside_mac);

    assert_eq!(
        nat.add_static_nat(StaticNat {
            public_address: Ipv4Addr::new(198, 51, 100, 10),
            private_address: Ipv4Addr::new(192, 168, 0, 11),
        }),
        Err(StaticNatError::PublicAddressOffLink(Ipv4Addr::new(
            198, 51, 100, 10
        )))
    );
}

#[test]
fn static_nat_validates_capacity_and_translates_both_directions() {
    let inside = Ipv4Cidr::new(Ipv4Addr::new(192, 168, 0, 0), 24).unwrap();
    let outside = Ipv4Addr::new(203, 0, 113, 2);
    let inside_mac = MacAddress::new([0x02, 0, 0, 0, 0, 1]);
    let outside_mac = MacAddress::new([0x02, 0, 0, 0, 0, 2]);
    let private_mac = MacAddress::new([0x02, 0, 0, 0, 0, 10]);
    let upstream_mac = MacAddress::new([0x02, 0, 0, 0, 0, 20]);
    let private = Ipv4Addr::new(192, 168, 0, 10);
    let published = Ipv4Addr::new(203, 0, 113, 10);
    let remote = Ipv4Addr::new(192, 0, 2, 10);
    let mut nat = NatRouter::new(
        id("static-nat-validation"),
        [
            routed_interface("inside", inside_mac, Ipv4Addr::new(192, 168, 0, 1)),
            routed_interface("outside", outside_mac, outside),
        ],
        [port("inside")],
        outside,
        edge_routes(inside),
    );
    let first = StaticNat {
        public_address: published,
        private_address: private,
    };
    nat.add_static_nat(first).expect("first static mapping");
    assert_eq!(
        nat.add_static_nat(first),
        Err(StaticNatError::DuplicatePublicAddress(published))
    );
    assert_eq!(
        nat.add_static_nat(StaticNat {
            public_address: Ipv4Addr::new(203, 0, 113, 11),
            private_address: private,
        }),
        Err(StaticNatError::DuplicatePrivateAddress(private))
    );
    let unreachable = Ipv4Addr::new(10, 0, 0, 10);
    assert_eq!(
        nat.add_static_nat(StaticNat {
            public_address: Ipv4Addr::new(203, 0, 113, 11),
            private_address: unreachable,
        }),
        Err(StaticNatError::PrivateAddressUnreachable(unreachable))
    );

    nat.handle(arp_request(
        "inside",
        private_mac,
        private,
        Ipv4Addr::new(192, 168, 0, 1),
    ));
    nat.handle(arp_request(
        "outside",
        upstream_mac,
        Ipv4Addr::new(203, 0, 113, 1),
        outside,
    ));
    let outbound = nat.handle(SimulationEvent::Network(NetworkIngress {
        port: port("inside"),
        frame: tcp_frame(private, remote, 50_000, 443, private_mac, inside_mac),
        received_at_us: 1,
    }));
    let Effect::Transmit { frame, .. } = &outbound[1] else {
        panic!("static source translation must forward");
    };
    let NetworkPayload::Ipv4(packet) = &frame.payload else {
        panic!("translated IPv4 packet");
    };
    assert_eq!(packet.source, published);
    assert_eq!(nat.translation_count(), 0, "static NAT does not consume PAT state");

    let inbound = nat.handle(SimulationEvent::Network(NetworkIngress {
        port: port("outside"),
        frame: tcp_frame(remote, published, 443, 50_000, upstream_mac, outside_mac),
        received_at_us: 2,
    }));
    let Effect::Transmit { egress, frame, .. } = &inbound[1] else {
        panic!("static destination translation must forward");
    };
    let NetworkPayload::Ipv4(packet) = &frame.payload else {
        panic!("translated IPv4 packet");
    };
    assert_eq!(egress, &port("inside"));
    assert_eq!(packet.destination, private);

    for suffix in 11..=25 {
        nat.add_static_nat(StaticNat {
            public_address: Ipv4Addr::new(203, 0, 113, suffix),
            private_address: Ipv4Addr::new(192, 168, 0, suffix),
        })
        .expect("mapping within reviewed static capacity");
    }
    assert_eq!(
        nat.add_static_nat(StaticNat {
            public_address: Ipv4Addr::new(203, 0, 113, 26),
            private_address: Ipv4Addr::new(192, 168, 0, 26),
        }),
        Err(StaticNatError::TableFull)
    );
}

#[test]
fn nat_routes_untranslated_traffic_answers_local_echo_and_bounds_pat_state() {
    let inside = Ipv4Cidr::new(Ipv4Addr::new(192, 168, 0, 0), 24).unwrap();
    let outside = Ipv4Addr::new(203, 0, 113, 2);
    let internal = Ipv4Addr::new(192, 168, 0, 2);
    let remote = Ipv4Addr::new(192, 0, 2, 10);
    let inside_mac = MacAddress::new([0x02, 0, 0, 0, 0, 1]);
    let outside_mac = MacAddress::new([0x02, 0, 0, 0, 0, 2]);
    let internal_mac = MacAddress::new([0x02, 0, 0, 0, 0, 10]);
    let upstream_mac = MacAddress::new([0x02, 0, 0, 0, 0, 20]);
    let mut nat = NatRouter::new(
        id("pat-boundary"),
        [
            routed_interface("inside", inside_mac, Ipv4Addr::new(192, 168, 0, 1)),
            routed_interface("outside", outside_mac, outside),
        ],
        [port("inside")],
        outside,
        edge_routes(inside),
    );
    nat.handle(arp_request(
        "outside",
        upstream_mac,
        Ipv4Addr::new(203, 0, 113, 1),
        outside,
    ));

    let routed = nat.handle(SimulationEvent::Network(NetworkIngress {
        port: port("outside"),
        frame: tcp_frame(
            remote,
            Ipv4Addr::new(198, 51, 100, 20),
            443,
            50_000,
            upstream_mac,
            outside_mac,
        ),
        received_at_us: 0,
    }));
    assert!(matches!(routed[0], Effect::Observe { .. }));

    let echo = EthernetFrame {
        source: upstream_mac,
        destination: outside_mac,
        vlan: VlanId::new(10).unwrap(),
        payload: NetworkPayload::Ipv4(Ipv4Packet {
            source: remote,
            destination: outside,
            ttl: 64,
            transport: Transport::Icmp(IcmpMessage::EchoRequest {
                identifier: 7,
                sequence: 1,
            }),
            application: ApplicationData::None,
        }),
        wire_len_bytes: 64,
    };
    let reply = nat.handle(SimulationEvent::Network(NetworkIngress {
        port: port("outside"),
        frame: echo,
        received_at_us: 1,
    }));
    assert!(matches!(reply[0], Effect::Transmit { .. }));

    let send = |nat: &mut NatRouter, source_port, now_us| {
        nat.handle(SimulationEvent::Network(NetworkIngress {
            port: port("inside"),
            frame: tcp_frame(
                internal,
                remote,
                source_port,
                443,
                internal_mac,
                inside_mac,
            ),
            received_at_us: now_us,
        }))
    };
    assert!(matches!(send(&mut nat, 10_000, 2)[1], Effect::Transmit { .. }));
    assert!(matches!(send(&mut nat, 10_000, 3)[1], Effect::Transmit { .. }));
    assert_eq!(nat.translation_count(), 1, "same flow refreshes one PAT entry");
    for source_port in 10_001..=10_063 {
        assert!(matches!(
            send(&mut nat, source_port, u64::from(source_port))[1],
            Effect::Transmit { .. }
        ));
    }
    assert_eq!(nat.translation_count(), 64);
    let full = send(&mut nat, 10_064, 20_000);
    assert_eq!(full.as_slice(), &[Effect::Drop(DropReason::NatTableFull)]);
    assert_eq!(nat.active_translation_count(20_000), 64);
    assert_eq!(nat.active_translations(20_000).count(), 64);

    let expired = nat.handle(SimulationEvent::Network(NetworkIngress {
        port: port("outside"),
        frame: tcp_frame(remote, outside, 443, 65_000, upstream_mac, outside_mac),
        received_at_us: 300_020_001,
    }));
    assert_eq!(expired.as_slice(), &[Effect::Drop(DropReason::NoTranslation)]);
    assert_eq!(nat.translation_count(), 0);

    assert!(matches!(
        nat.handle(SimulationEvent::SetOperational(false))[0],
        Effect::Observe { .. }
    ));
    assert_eq!(
        nat.handle(SimulationEvent::Network(NetworkIngress {
            port: port("inside"),
            frame: tcp_frame(internal, remote, 50_000, 443, internal_mac, inside_mac),
            received_at_us: 0,
        }))
        .as_slice(),
        &[Effect::Drop(DropReason::ComponentDown)]
    );
    assert_eq!(
        nat.handle(SimulationEvent::Process(ProcessEvent::Tick { elapsed_ms: 1 }))
            .as_slice(),
        &[Effect::Drop(DropReason::UnsupportedProtocol)]
    );
}

#[test]
fn pat_uses_protocol_specific_timeouts_and_rejects_unaddressable_transports() {
    let inside = Ipv4Cidr::new(Ipv4Addr::new(192, 168, 0, 0), 24).unwrap();
    let outside = Ipv4Addr::new(203, 0, 113, 2);
    let internal = Ipv4Addr::new(192, 168, 0, 2);
    let remote = Ipv4Addr::new(192, 0, 2, 10);
    let inside_mac = MacAddress::new([0x02, 0, 0, 0, 0, 1]);
    let outside_mac = MacAddress::new([0x02, 0, 0, 0, 0, 2]);
    let internal_mac = MacAddress::new([0x02, 0, 0, 0, 0, 10]);
    let mut nat = NatRouter::new(
        id("pat-protocol-timeouts"),
        [
            routed_interface("inside", inside_mac, Ipv4Addr::new(192, 168, 0, 1)),
            routed_interface("outside", outside_mac, outside),
        ],
        [port("inside")],
        outside,
        edge_routes(inside),
    );
    let frame = |transport| EthernetFrame {
        source: internal_mac,
        destination: inside_mac,
        vlan: VlanId::new(10).unwrap(),
        payload: NetworkPayload::Ipv4(Ipv4Packet {
            source: internal,
            destination: remote,
            ttl: 64,
            transport,
            application: ApplicationData::None,
        }),
        wire_len_bytes: 64,
    };
    for transport in [
        Transport::Udp(UdpDatagram {
            source_port: 53_000,
            destination_port: 53,
        }),
        Transport::Icmp(IcmpMessage::EchoRequest {
            identifier: 90,
            sequence: 1,
        }),
    ] {
        let effects = nat.handle(SimulationEvent::Network(NetworkIngress {
            port: port("inside"),
            frame: frame(transport),
            received_at_us: 100,
        }));
        assert!(matches!(effects[0], Effect::Observe { .. }));
    }
    let expiries = nat
        .active_translations(100)
        .map(|translation| (translation.protocol, translation.expires_at_us))
        .collect::<Vec<_>>();
    assert!(expiries.contains(&(TransportProtocol::Udp, 60_000_100)));
    assert!(expiries.contains(&(TransportProtocol::Icmp, 30_000_100)));

    let unsupported = nat.handle(SimulationEvent::Network(NetworkIngress {
        port: port("inside"),
        frame: frame(Transport::Other(99)),
        received_at_us: 101,
    }));
    assert_eq!(
        unsupported.as_slice(),
        &[Effect::Drop(DropReason::UnsupportedProtocol)]
    );
}
