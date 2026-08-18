use std::net::Ipv4Addr;

use hearthline_engine::{
    DropReason, Effect, LinkAppliance, LinkMode, NetworkIngress, SimulatedComponent,
    SimulationEvent,
};
use hearthline_model::{
    ApplicationData, ComponentId, ComponentKind, EthernetFrame, IcmpMessage, Ipv4Packet,
    MacAddress, NetworkPayload, PortId, Transport, VlanId,
};

fn id(value: &str) -> ComponentId {
    ComponentId::new(value).expect("test ID")
}

fn port(value: &str) -> PortId {
    PortId::new(value).expect("test port")
}

fn frame(source: MacAddress) -> EthernetFrame {
    EthernetFrame {
        source,
        destination: MacAddress::BROADCAST,
        vlan: VlanId::new(1).expect("VLAN"),
        payload: NetworkPayload::Ipv4(Ipv4Packet {
            source: Ipv4Addr::new(192, 0, 2, 1),
            destination: Ipv4Addr::new(192, 0, 2, 2),
            ttl: 64,
            transport: Transport::Icmp(IcmpMessage::EchoRequest {
                identifier: 1,
                sequence: 1,
            }),
            application: ApplicationData::None,
        }),
        wire_len_bytes: EthernetFrame::MIN_WIRE_LEN_BYTES,
    }
}

fn ingress(port_id: &str, frame: EthernetFrame) -> SimulationEvent {
    SimulationEvent::Network(NetworkIngress {
        port: port(port_id),
        frame,
        received_at_us: 0,
    })
}

#[test]
fn transparent_link_validates_ethernet_and_internal_port_state() {
    let mut link = LinkAppliance::new(
        id("cpe-01"),
        ComponentKind::TransparentCpe,
        [port("customer"), port("access")],
        LinkMode::Transparent,
    );
    let source = MacAddress::new([0x02, 0, 0, 0, 0, 1]);

    let invalid = link.handle(ingress("customer", frame(MacAddress::BROADCAST)));
    assert_eq!(
        invalid.as_slice(),
        &[Effect::Drop(DropReason::InvalidSourceMac(
            MacAddress::BROADCAST
        ))]
    );

    assert!(link.set_port_forwarding(&port("access"), false));
    let no_egress = link.handle(ingress("customer", frame(source)));
    assert_eq!(no_egress.as_slice(), &[Effect::Drop(DropReason::LinkLoss)]);
    let down_ingress = link.handle(ingress("access", frame(source)));
    assert_eq!(
        down_ingress.as_slice(),
        &[Effect::Drop(DropReason::PortDown(port("access")))]
    );
}

#[test]
fn embedded_virtual_switch_supports_dense_control_hosts() {
    let switch = LinkAppliance::embedded_virtual_switch(
        id("control-host-01"),
        (0..20).map(|index| port(&format!("virtual-{index}"))),
    );

    assert!(switch.has_port(&port("virtual-19")));
}
