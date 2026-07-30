use core::net::Ipv4Addr;

use hearthline_model::{ComponentId, ComponentKind, FlowKey, Ipv4Cidr, PortId, TransportProtocol};

#[test]
fn identifiers_are_stable_and_restricted() {
    assert!(ComponentId::new("business-frw-01a").is_ok());
    assert!(ComponentId::new("Business FRW-01A").is_err());
    assert!(PortId::new("gigabit-ethernet-0-1").is_ok());
    assert!(PortId::new("").is_err());
}

#[test]
fn every_component_kind_has_a_behavior_family() {
    for kind in ComponentKind::ALL {
        assert!(!kind.behavior_family().to_string().is_empty());
        assert_eq!(kind.to_string().parse::<ComponentKind>(), Ok(kind));
    }
}

#[test]
fn cidr_normalizes_and_matches() {
    let prefix =
        Ipv4Cidr::new(Ipv4Addr::new(192, 168, 0, 55), 24).expect("test prefix must be valid");
    assert_eq!(prefix.network(), Ipv4Addr::new(192, 168, 0, 0));
    assert!(prefix.contains(Ipv4Addr::new(192, 168, 0, 200)));
    assert!(!prefix.contains(Ipv4Addr::new(192, 168, 1, 1)));
}

#[test]
fn reverse_flow_swaps_endpoints() {
    let flow = FlowKey {
        source: Ipv4Addr::new(10, 0, 0, 10),
        destination: Ipv4Addr::new(10, 0, 1, 20),
        protocol: TransportProtocol::Tcp,
        source_port: Some(50_000),
        destination_port: Some(443),
    };
    assert_eq!(flow.reverse().source_port, Some(443));
    assert_eq!(flow.reverse().destination_port, Some(50_000));
}
