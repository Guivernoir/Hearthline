use std::collections::BTreeSet;
use std::net::Ipv4Addr;

use hearthline_engine::{
    Effect, LinkAppliance, LinkMode, RENDERED_ROLE_CONTRACTS, ServiceNode, Simulator,
    appliance_contracts,
};
use hearthline_model::{
    ApplicationData, ComponentId, ComponentKind, EthernetFrame, Ipv4Packet, MacAddress,
    NetworkPayload, PortId, ServiceKind, TcpFlags, TcpSegment, Transport, VlanId,
};

fn id(value: &str) -> ComponentId {
    ComponentId::new(value).expect("test ID")
}

fn port(value: &str) -> PortId {
    PortId::new(value).expect("test port")
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
    assert!(bytes <= 300_000, "simulator occupies {bytes} bytes");
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
        [port("network")],
        [Ipv4Addr::new(192, 0, 2, 10)],
        [ServiceKind::Https],
    );
    let mut simulator = Simulator::new();
    simulator.add(&mut cpe).expect("add CPE");
    simulator.add(&mut service).expect("add service");
    simulator
        .connect(&cpe_id, &port("access"), &server_id, &port("network"))
        .expect("connect");

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
