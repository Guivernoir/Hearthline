use core::net::Ipv4Addr;

use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use hearthline_engine::{
    ConnectionMedium, CopperCategory, CopperMedium, CopperWiring, LinkAppliance, LinkEndpoint,
    LinkMode, MediaLink, MediaLinkConfig, PortDuplex, PortHardwareKind, PortSettings, PortState,
    PortStateConfig, RoutedInterface, RoutingTable, ServiceNode, SimulatedPort, Simulator,
};
use hearthline_model::{
    ApplicationData, ComponentId, ComponentKind, EthernetFrame, Ipv4Cidr, Ipv4InterfaceAddress,
    Ipv4Packet, MacAddress, NetworkPayload, PortId, Route, ServiceKind, TcpFlags, TcpSegment,
    Transport, VlanId,
};

fn id(value: &str) -> ComponentId {
    ComponentId::new(value).expect("benchmark ID")
}

fn port(value: &str) -> PortId {
    PortId::new(value).expect("benchmark port")
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

fn benchmark_routes(criterion: &mut Criterion) {
    let table = RoutingTable::new([
        Route {
            destination: Ipv4Cidr::new(Ipv4Addr::UNSPECIFIED, 0).expect("default"),
            egress: port("outside"),
            next_hop: Some(Ipv4Addr::new(203, 0, 113, 1)),
            metric: 10,
        },
        Route {
            destination: Ipv4Cidr::new(Ipv4Addr::new(10, 10, 0, 0), 16).expect("site"),
            egress: port("inside"),
            next_hop: None,
            metric: 1,
        },
        Route {
            destination: Ipv4Cidr::new(Ipv4Addr::new(10, 10, 20, 0), 24).expect("zone"),
            egress: port("process"),
            next_hop: None,
            metric: 0,
        },
    ]);
    criterion.bench_function("longest_prefix_lookup", |bencher| {
        bencher.iter(|| table.lookup(black_box(Ipv4Addr::new(10, 10, 20, 42))));
    });
}

fn demo_frame() -> EthernetFrame {
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
    }
}

fn benchmark_traversal(criterion: &mut Criterion) {
    criterion.bench_function("transparent_link_to_service", |bencher| {
        bencher.iter_batched(
            || {
                let cpe = LinkAppliance::new(
                    id("customer-inet-cpe-01"),
                    ComponentKind::TransparentCpe,
                    [port("customer"), port("access")],
                    LinkMode::Transparent,
                );
                let service = ServiceNode::new(
                    id("public-service-01"),
                    ComponentKind::ServiceCluster,
                    [RoutedInterface::new(
                        port("network"),
                        MacAddress::new([0, 1, 2, 3, 4, 6]),
                        [Ipv4InterfaceAddress::new(Ipv4Addr::new(192, 0, 2, 10), 24)
                            .expect("interface address")],
                        VlanId::new(10).expect("VLAN"),
                        1_500,
                    )],
                    [ServiceKind::Https],
                );
                let connection = MediaLink::new(
                    id("cpe-to-service"),
                    endpoint(&id("customer-inet-cpe-01"), "access"),
                    endpoint(&id("public-service-01"), "network"),
                    MediaLinkConfig::default(),
                    ConnectionMedium::Copper {
                        config: CopperMedium {
                            wiring: CopperWiring::StraightThrough,
                            category: CopperCategory::Cat6a,
                            length_m: 10.0,
                        },
                    },
                )
                .expect("benchmark media link");
                (cpe, service, connection, demo_frame())
            },
            |(mut cpe, mut service, mut connection, frame)| {
                let mut simulator = Simulator::new();
                simulator.add(&mut cpe).expect("CPE registry slot");
                simulator.add(&mut service).expect("service registry slot");
                simulator.add_link(&mut connection).expect("benchmark link");
                simulator
                    .inject_network(&id("customer-inet-cpe-01"), &port("customer"), frame)
                    .expect("benchmark event");
                black_box(simulator.run(8).expect("benchmark simulation").len())
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(runtime, benchmark_routes, benchmark_traversal);
criterion_main!(runtime);
