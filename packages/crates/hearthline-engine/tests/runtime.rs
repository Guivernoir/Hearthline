use std::collections::BTreeSet;
use std::net::Ipv4Addr;

use hearthline_engine::{
    ConnectionMedium, CopperCategory, CopperMedium, CopperWiring, Effect, LearningSwitch,
    LinkAppliance, LinkEndpoint, LinkMode, MediaLink, MediaLinkConfig, PortDuplex,
    PortHardwareKind, PortSettings, PortState, PortStateConfig, RENDERED_ROLE_CONTRACTS,
    RadioMedium, RoutedInterface, Router, RoutingTable, ServiceNode, SimulatedPort, Simulator,
    SwitchPort, appliance_contracts,
};
use hearthline_model::{
    ApplicationData, ArpOperation, ArpPacket, ComponentId, ComponentKind, EthernetFrame, Ipv4Cidr,
    Ipv4InterfaceAddress, Ipv4Packet, MacAddress, NetworkPayload, PortId, Route, ServiceKind,
    TcpFlags, TcpSegment, Transport, VlanId,
};

fn id(value: &str) -> ComponentId {
    ComponentId::new(value).expect("test ID")
}

fn port(value: &str) -> PortId {
    PortId::new(value).expect("test port")
}

fn endpoint(component: &ComponentId, port_id: &str) -> LinkEndpoint {
    LinkEndpoint {
        component: component.clone(),
        port: port(port_id),
        profile: SimulatedPort {
            hardware: PortHardwareKind::EthernetRj45,
            state: PortStateConfig {
                administrative: PortState::Up,
                initial_operational: PortState::Up,
            },
            settings: PortSettings {
                speed_mbps: 1_000,
                duplex: PortDuplex::Full,
                mtu: 1_500,
            },
        },
    }
}

fn radio_endpoint(component: &ComponentId, port_id: &str) -> LinkEndpoint {
    LinkEndpoint {
        component: component.clone(),
        port: port(port_id),
        profile: SimulatedPort {
            hardware: PortHardwareKind::WirelessRadio,
            state: PortStateConfig {
                administrative: PortState::Up,
                initial_operational: PortState::Up,
            },
            settings: PortSettings {
                speed_mbps: 1_000,
                duplex: PortDuplex::Half,
                mtu: 1_500,
            },
        },
    }
}

fn radio_medium() -> ConnectionMedium {
    ConnectionMedium::Radio {
        config: RadioMedium {
            standard: "802.11ax".into(),
            ssid: "hearthline-test".into(),
            security: "wpa3-personal".into(),
            distance_m: 10.0,
        },
    }
}

fn copper_medium() -> ConnectionMedium {
    ConnectionMedium::Copper {
        config: CopperMedium {
            wiring: CopperWiring::StraightThrough,
            category: CopperCategory::Cat6a,
            length_m: 10.0,
        },
    }
}

fn host_interface(
    port_id: &str,
    mac: MacAddress,
    address: Ipv4Addr,
    vlan: VlanId,
) -> RoutedInterface {
    RoutedInterface::new(
        port(port_id),
        mac,
        [Ipv4InterfaceAddress::new(address, 24).expect("interface address")],
        vlan,
        1_500,
    )
}

#[test]
fn catalog_covers_every_appliance_kind() {
    let contracts = appliance_contracts().collect::<Vec<_>>();
    let kinds = contracts
        .iter()
        .map(|contract| contract.kind)
        .collect::<BTreeSet<_>>();
    assert_eq!(contracts.len(), ComponentKind::ALL.len());
    assert_eq!(kinds.len(), ComponentKind::ALL.len());
    assert!(
        contracts
            .iter()
            .all(|contract| !contract.baseline.is_empty())
    );
}

#[test]
fn every_rendered_role_resolves_to_a_catalog_kind() {
    let catalog = ComponentKind::ALL.into_iter().collect::<BTreeSet<_>>();
    assert!(
        RENDERED_ROLE_CONTRACTS
            .iter()
            .all(|contract| catalog.contains(&contract.kind))
    );
}

#[test]
fn simulator_has_a_bounded_inline_footprint() {
    let bytes = std::mem::size_of::<Simulator>();
    let trace_entry_bytes = std::mem::size_of::<hearthline_engine::TraceEntry>();
    assert!(
        bytes <= 300_000,
        "simulator occupies {bytes} bytes with {trace_entry_bytes}-byte trace entries"
    );
}

#[test]
fn forwards_through_transparent_cpe_to_service() {
    let cpe_id = id("customer-inet-cpe-01");
    let server_id = id("public-service-01");
    let mut cpe = LinkAppliance::new(
        cpe_id.clone(),
        ComponentKind::TransparentCpe,
        [port("customer"), port("access")],
        LinkMode::Transparent,
    );
    let mut service = ServiceNode::new(
        server_id.clone(),
        ComponentKind::ServiceCluster,
        [host_interface(
            "network",
            MacAddress::new([0, 1, 2, 3, 4, 6]),
            Ipv4Addr::new(192, 0, 2, 10),
            VlanId::new(10).expect("VLAN"),
        )],
        [ServiceKind::Https],
    );
    let mut connection = MediaLink::new(
        id("cpe-to-service"),
        endpoint(&cpe_id, "access"),
        endpoint(&server_id, "network"),
        MediaLinkConfig::default(),
        ConnectionMedium::Copper {
            config: CopperMedium {
                wiring: CopperWiring::StraightThrough,
                category: CopperCategory::Cat6a,
                length_m: 10.0,
            },
        },
    )
    .expect("media link");
    let mut simulator = Simulator::new();
    simulator.add(&mut cpe).expect("add CPE");
    simulator.add(&mut service).expect("add service");
    simulator.add_link(&mut connection).expect("connect");

    simulator
        .inject_network(
            &cpe_id,
            &port("customer"),
            EthernetFrame {
                source: MacAddress::new([0, 1, 2, 3, 4, 5]),
                destination: MacAddress::new([0, 1, 2, 3, 4, 6]),
                vlan: VlanId::new(10).expect("VLAN"),
                payload: NetworkPayload::Ipv4(Ipv4Packet {
                    source: Ipv4Addr::new(203, 0, 113, 2),
                    destination: Ipv4Addr::new(192, 0, 2, 10),
                    ttl: 64,
                    transport: Transport::Tcp(TcpSegment {
                        source_port: 50_000,
                        destination_port: 443,
                        flags: TcpFlags {
                            syn: true,
                            ..TcpFlags::default()
                        },
                    }),
                    application: ApplicationData::Service(ServiceKind::Https),
                }),
                wire_len_bytes: 64,
            },
        )
        .expect("inject");
    let trace = simulator.run(10).expect("simulation");
    assert!(trace.iter().any(|entry| {
        entry.component == server_id
            && matches!(
                entry.effect,
                Effect::Deliver {
                    service: ServiceKind::Https,
                    ..
                }
            )
    }));
}

#[test]
fn shared_radio_port_fans_out_through_each_media_association() {
    let bridge_id = id("guest-ap-01");
    let client_a_id = id("guest-client-01");
    let client_b_id = id("guest-client-02");
    let mut bridge = LinkAppliance::new(
        bridge_id.clone(),
        ComponentKind::TransparentCpe,
        [port("ingress"), port("radio")],
        LinkMode::Transparent,
    );
    let mut client_a = ServiceNode::new(
        client_a_id.clone(),
        ComponentKind::Workstation,
        [host_interface(
            "radio",
            MacAddress::new([0x02, 0, 0, 0, 0, 10]),
            Ipv4Addr::new(192, 168, 50, 10),
            VlanId::new(50).expect("VLAN"),
        )],
        [ServiceKind::Https],
    );
    let mut client_b = ServiceNode::new(
        client_b_id.clone(),
        ComponentKind::Workstation,
        [host_interface(
            "radio",
            MacAddress::new([0x02, 0, 0, 0, 0, 11]),
            Ipv4Addr::new(192, 168, 50, 11),
            VlanId::new(50).expect("VLAN"),
        )],
        [ServiceKind::Https],
    );
    let mut association_a = MediaLink::new(
        id("guest-radio-a"),
        radio_endpoint(&bridge_id, "radio"),
        radio_endpoint(&client_a_id, "radio"),
        MediaLinkConfig::default(),
        radio_medium(),
    )
    .expect("radio association A");
    let mut association_b = MediaLink::new(
        id("guest-radio-b"),
        radio_endpoint(&bridge_id, "radio"),
        radio_endpoint(&client_b_id, "radio"),
        MediaLinkConfig::default(),
        radio_medium(),
    )
    .expect("radio association B");
    let mut simulator = Simulator::new();
    simulator.add(&mut bridge).expect("add bridge");
    simulator.add(&mut client_a).expect("add client A");
    simulator.add(&mut client_b).expect("add client B");
    simulator
        .add_link(&mut association_a)
        .expect("add association A");
    simulator
        .add_link(&mut association_b)
        .expect("add association B");

    simulator
        .inject_network(
            &bridge_id,
            &port("ingress"),
            EthernetFrame {
                source: MacAddress::new([0, 1, 2, 3, 4, 5]),
                destination: MacAddress::BROADCAST,
                vlan: VlanId::new(50).expect("VLAN"),
                payload: NetworkPayload::Ipv4(Ipv4Packet {
                    source: Ipv4Addr::new(192, 168, 50, 1),
                    destination: Ipv4Addr::new(192, 168, 50, 255),
                    ttl: 1,
                    transport: Transport::Tcp(TcpSegment {
                        source_port: 50_000,
                        destination_port: 443,
                        flags: TcpFlags::default(),
                    }),
                    application: ApplicationData::None,
                }),
                wire_len_bytes: 64,
            },
        )
        .expect("inject broadcast");
    let trace = simulator.run(20).expect("simulation");
    let destinations = trace
        .iter()
        .filter_map(|entry| match &entry.effect {
            Effect::MediaTransit {
                destination_component,
                ..
            } => Some(destination_component),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert!(destinations.contains(&client_a_id));
    assert!(destinations.contains(&client_b_id));
}

#[test]
fn host_reaches_service_through_switch_router_and_media() {
    let client_id = id("client-01");
    let switch_id = id("access-sw-01");
    let router_id = id("router-01");
    let server_id = id("server-01");
    let client_vlan = VlanId::new(10).expect("client VLAN");
    let server_vlan = VlanId::new(20).expect("server VLAN");
    let client_ip = Ipv4Addr::new(192, 168, 1, 10);
    let gateway_ip = Ipv4Addr::new(192, 168, 1, 1);
    let server_ip = Ipv4Addr::new(192, 0, 2, 10);
    let client_mac = MacAddress::new([0x02, 0, 0, 0, 0, 10]);
    let inside_mac = MacAddress::new([0x02, 0, 0, 0, 1, 1]);
    let outside_mac = MacAddress::new([0x02, 0, 0, 0, 2, 1]);
    let server_mac = MacAddress::new([0x02, 0, 0, 0, 2, 10]);

    let mut client = ServiceNode::with_default_gateway(
        client_id.clone(),
        ComponentKind::Workstation,
        [host_interface(
            "network",
            client_mac,
            client_ip,
            client_vlan,
        )],
        gateway_ip,
        [],
    );
    let mut switch = LearningSwitch::new(
        switch_id.clone(),
        [
            SwitchPort::access(port("client"), client_vlan),
            SwitchPort::access(port("router"), client_vlan),
        ],
    );
    let mut router = Router::new(
        router_id.clone(),
        ComponentKind::Router,
        [
            host_interface("inside", inside_mac, gateway_ip, client_vlan),
            host_interface(
                "outside",
                outside_mac,
                Ipv4Addr::new(192, 0, 2, 1),
                server_vlan,
            ),
        ],
        RoutingTable::new([
            Route {
                destination: Ipv4Cidr::new(Ipv4Addr::new(192, 168, 1, 0), 24)
                    .expect("client subnet"),
                egress: port("inside"),
                next_hop: None,
                metric: 0,
            },
            Route {
                destination: Ipv4Cidr::new(Ipv4Addr::new(192, 0, 2, 0), 24).expect("server subnet"),
                egress: port("outside"),
                next_hop: None,
                metric: 0,
            },
        ]),
    );
    let mut server = ServiceNode::new(
        server_id.clone(),
        ComponentKind::ServiceCluster,
        [host_interface(
            "network",
            server_mac,
            server_ip,
            server_vlan,
        )],
        [ServiceKind::Https],
    );
    let mut client_link = MediaLink::new(
        id("client-to-switch"),
        endpoint(&client_id, "network"),
        endpoint(&switch_id, "client"),
        MediaLinkConfig::default(),
        copper_medium(),
    )
    .expect("client link");
    let mut router_link = MediaLink::new(
        id("switch-to-router"),
        endpoint(&switch_id, "router"),
        endpoint(&router_id, "inside"),
        MediaLinkConfig::default(),
        copper_medium(),
    )
    .expect("router link");
    let mut server_link = MediaLink::new(
        id("router-to-server"),
        endpoint(&router_id, "outside"),
        endpoint(&server_id, "network"),
        MediaLinkConfig::default(),
        copper_medium(),
    )
    .expect("server link");
    let mut simulator = Simulator::new();
    simulator.add(&mut client).expect("add client");
    simulator.add(&mut switch).expect("add switch");
    simulator.add(&mut router).expect("add router");
    simulator.add(&mut server).expect("add server");
    simulator.add_link(&mut client_link).expect("client link");
    simulator.add_link(&mut router_link).expect("router link");
    simulator.add_link(&mut server_link).expect("server link");

    simulator
        .inject_ipv4(
            &client_id,
            Ipv4Packet {
                source: client_ip,
                destination: server_ip,
                ttl: 64,
                transport: Transport::Tcp(TcpSegment {
                    source_port: 50_000,
                    destination_port: 443,
                    flags: TcpFlags {
                        syn: true,
                        ..TcpFlags::default()
                    },
                }),
                application: ApplicationData::Service(ServiceKind::Https),
            },
            EthernetFrame::MIN_WIRE_LEN_BYTES,
        )
        .expect("originate HTTPS packet");
    let trace = simulator.run(32).expect("end-to-end simulation");
    assert!(trace.iter().any(|entry| {
        entry.component == client_id
            && matches!(
                &entry.effect,
                Effect::Transmit {
                    frame:
                        EthernetFrame {
                            payload:
                                NetworkPayload::Arp(ArpPacket {
                                    operation: ArpOperation::Request,
                                    target_ip,
                                    ..
                                }),
                            ..
                        },
                    ..
                } if *target_ip == gateway_ip
            )
    }));
    assert!(trace.iter().any(|entry| {
        entry.component == router_id
            && matches!(
                &entry.effect,
                Effect::Transmit { frame, .. }
                    if frame.source == outside_mac
                        && frame.destination == server_mac
                        && matches!(
                            &frame.payload,
                            NetworkPayload::Ipv4(packet) if packet.ttl == 63
                        )
            )
    }));
    assert!(trace.iter().any(|entry| {
        entry.component == server_id
            && matches!(
                entry.effect,
                Effect::Deliver {
                    service: ServiceKind::Https,
                    ..
                }
            )
    }));
}
