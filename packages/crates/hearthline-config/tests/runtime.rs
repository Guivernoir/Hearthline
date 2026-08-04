use std::net::Ipv4Addr;
use std::path::PathBuf;

use hearthline_config::{ConfigRepository, ConfiguredNetwork, ConnectionRepository};
use hearthline_engine::Effect;
use hearthline_model::{
    ApplicationData, ComponentId, IcmpMessage, Ipv4Packet, MacAddress, NetworkPayload, Transport,
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
                Effect::Observe { detail } if detail.contains("EchoReply")
            )
    }));
}
