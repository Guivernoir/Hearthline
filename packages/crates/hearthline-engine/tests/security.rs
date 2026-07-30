use std::net::Ipv4Addr;

use hearthline_engine::{
    DropReason, Effect, FirewallAction, FirewallRule, NatRouter, NetworkIngress, RoutingTable,
    SimulatedComponent, SimulationEvent, StatefulFirewall,
};
use hearthline_model::{
    ApplicationData, ComponentId, EthernetFrame, Ipv4Cidr, Ipv4Packet, MacAddress, NetworkPayload,
    PortId, Route, TcpFlags, TcpSegment, Transport, TransportProtocol, VlanId,
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
) -> EthernetFrame {
    EthernetFrame {
        source: MacAddress::new([0, 1, 2, 3, 4, 5]),
        destination: MacAddress::new([0, 1, 2, 3, 4, 6]),
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
    }
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
            next_hop: None,
            metric: 10,
        },
    ])
}

#[test]
fn firewall_permits_https_tracks_reverse_state_and_defaults_to_deny() {
    let dmz = Ipv4Cidr::new(Ipv4Addr::new(172, 16, 10, 0), 24).expect("DMZ");
    let mut firewall = StatefulFirewall::new(
        id("business-frw-01a"),
        [
            (port("outside"), "outside".into()),
            (port("dmz"), "dmz".into()),
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
    let allowed = firewall.handle(SimulationEvent::Network(NetworkIngress {
        port: port("outside"),
        frame: tcp_frame(client, server, 50_000, 443),
    }));
    assert!(matches!(allowed[1], Effect::Transmit { .. }));
    assert_eq!(firewall.session_count(), 1);
    let returned = firewall.handle(SimulationEvent::Network(NetworkIngress {
        port: port("dmz"),
        frame: tcp_frame(server, client, 443, 50_000),
    }));
    assert!(matches!(returned[1], Effect::Transmit { .. }));
    let denied = firewall.handle(SimulationEvent::Network(NetworkIngress {
        port: port("outside"),
        frame: tcp_frame(client, server, 50_001, 22),
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
    let mut nat = NatRouter::new(
        id("customer-rtr-01"),
        [port("inside"), port("outside")],
        [port("inside")],
        outside,
        edge_routes(inside),
    );
    let outbound = nat.handle(SimulationEvent::Network(NetworkIngress {
        port: port("inside"),
        frame: tcp_frame(internal, remote, 50_000, 443),
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

    let mut return_frame = tcp_frame(remote, outside, 443, external_port);
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
