use std::net::Ipv4Addr;
use std::path::PathBuf;

use hearthline_config::{ConfigRepository, ConfiguredNetwork, ConnectionRepository};
use hearthline_engine::{
    DropReason, Effect, NetworkIngress, RoutedInterface, ServiceNode, SimulatedComponent,
    SimulationEvent,
};
use hearthline_model::{
    ApplicationData, ComponentId, ComponentKind, EthernetFrame, IcmpMessage, Ipv4InterfaceAddress,
    Ipv4Packet, MacAddress, NetworkPayload, PortId, ServiceKind, Transport, VlanId,
};

fn project_config() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config")
}

#[test]
fn selected_customer_yaml_runs_arp_switching_and_router_reply() {
    let config = project_config();
    let appliances =
        ConfigRepository::load(config.join("appliances")).expect("appliance repository");
    let connections = ConnectionRepository::load(config.join("connections"), &appliances)
        .expect("connection repository");
    let mut network = ConfiguredNetwork::from_selection(
        &appliances,
        &connections,
        ["customer-pc-01", "customer-sw-01", "customer-rtr-01"],
    )
    .expect("configured customer LAN");
    assert_eq!(network.appliance_count(), 3);
    assert_eq!(network.link_count(), 2);

    let router = ComponentId::new("customer-rtr-01").expect("router ID");
    let client = ComponentId::new("customer-pc-01").expect("client ID");
    let trace = network
        .run_ipv4(
            &client,
            Ipv4Packet {
                source: Ipv4Addr::new(192, 168, 0, 2),
                destination: Ipv4Addr::new(192, 168, 0, 1),
                ttl: 64,
                transport: Transport::Icmp(IcmpMessage::EchoRequest {
                    identifier: 7,
                    sequence: 1,
                }),
                application: ApplicationData::None,
            },
            32,
        )
        .expect("configured simulation");
    assert!(
        trace.iter().any(|entry| {
            entry.component == router
                && matches!(
                    &entry.effect,
                    Effect::Transmit { frame, .. }
                        if frame.source == MacAddress::new([0x02, 0, 0, 0, 1, 1])
                            && matches!(
                                &frame.payload,
                                NetworkPayload::Ipv4(Ipv4Packet {
                                    transport: Transport::Icmp(IcmpMessage::EchoReply { .. }),
                                    ..
                                })
                            )
                )
        }),
        "configured trace did not contain the router echo reply: {trace:#?}"
    );
    assert!(
        trace
            .iter()
            .filter(|entry| matches!(entry.effect, Effect::MediaTransit { .. }))
            .count()
            >= 4
    );
    assert!(trace.iter().any(|entry| {
        entry.component == client
            && matches!(
                &entry.effect,
                Effect::Deliver { service, detail }
                    if *service == hearthline_model::ServiceKind::IcmpEcho
                        && detail.contains("echo reply")
            )
    }));
}

#[test]
fn endpoint_can_disable_icmp_echo_responses() {
    let server_ip = Ipv4Addr::new(192, 0, 2, 10);
    let server_mac = MacAddress::new([0x02, 0, 0, 0, 0, 10]);
    let port = PortId::new("network").expect("port");
    let mut server = ServiceNode::new(
        ComponentId::new("service-01").expect("component"),
        ComponentKind::ServiceCluster,
        [RoutedInterface::new(
            port.clone(),
            server_mac,
            [Ipv4InterfaceAddress::new(server_ip, 24).expect("address")],
            VlanId::new(10).expect("VLAN"),
            1_500,
        )],
        [],
    );
    server.set_respond_to_icmp(false);
    let effects = server.handle(SimulationEvent::Network(NetworkIngress {
        port,
        frame: EthernetFrame {
            source: MacAddress::new([0x02, 0, 0, 0, 0, 20]),
            destination: server_mac,
            vlan: VlanId::new(10).expect("VLAN"),
            payload: NetworkPayload::Ipv4(Ipv4Packet {
                source: Ipv4Addr::new(192, 0, 2, 20),
                destination: server_ip,
                ttl: 64,
                transport: Transport::Icmp(IcmpMessage::EchoRequest {
                    identifier: 7,
                    sequence: 1,
                }),
                application: ApplicationData::None,
            }),
            wire_len_bytes: 64,
        },
        received_at_us: 0,
    }));
    assert!(matches!(
        effects[0],
        Effect::Drop(DropReason::ServiceUnavailable(ServiceKind::IcmpEcho))
    ));
}
