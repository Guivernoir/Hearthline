use std::fs;
use std::net::Ipv4Addr;
use std::path::PathBuf;

use hearthline_config::{ConfigRepository, ConnectionConfig, ConnectionRepository};
use hearthline_engine::{
    ConnectionMedium, CopperCategory, CopperMedium, CopperWiring, LinkDirection, LinkEndpoint,
    MediaDropReason, MediaLink, MediaLinkConfig, PortDuplex, PortHardwareKind, PortSettings,
    PortState, PortStateConfig, RadioMedium, SimulatedPort, VirtualMedium,
};
use hearthline_model::{
    ArpOperation, ArpPacket, ComponentId, EthernetFrame, MacAddress, NetworkPayload, PortId, VlanId,
};

fn endpoint(appliance: &str, interface: &str) -> LinkEndpoint {
    LinkEndpoint {
        component: ComponentId::new(appliance).expect("valid component"),
        port: PortId::new(interface).expect("valid port"),
        profile: SimulatedPort {
            hardware: PortHardwareKind::VirtualNic,
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

fn frame(wire_len_bytes: u16) -> EthernetFrame {
    EthernetFrame {
        source: MacAddress::new([0x02, 0, 0, 0, 0, 1]),
        destination: MacAddress::new([0x02, 0, 0, 0, 0, 2]),
        vlan: VlanId::new(10).expect("valid VLAN"),
        payload: NetworkPayload::Arp(ArpPacket {
            operation: ArpOperation::Request,
            sender_mac: MacAddress::new([0x02, 0, 0, 0, 0, 1]),
            sender_ip: Ipv4Addr::new(192, 0, 2, 1),
            target_mac: None,
            target_ip: Ipv4Addr::new(192, 0, 2, 2),
        }),
        wire_len_bytes,
    }
}

fn virtual_medium() -> ConnectionMedium {
    ConnectionMedium::Virtual {
        config: VirtualMedium {
            technology: "test bridge".into(),
        },
    }
}

fn link(config: MediaLinkConfig) -> MediaLink {
    MediaLink::new(
        ComponentId::new("connection-01").expect("valid ID"),
        endpoint("switch-01", "ethernet-1"),
        endpoint("router-01", "ethernet-1"),
        config,
        virtual_medium(),
    )
    .expect("valid connection")
}

#[test]
fn connector_applies_serialization_and_fixed_delay() {
    let mut connector = link(MediaLinkConfig {
        capacity_mbps: 1,
        latency_ms: 2,
        ..MediaLinkConfig::default()
    });
    let transit = connector
        .transmit(
            &ComponentId::new("switch-01").expect("component"),
            &PortId::new("ethernet-1").expect("port"),
            &frame(1_500),
            0,
        )
        .expect("frame should transit");
    assert_eq!(
        transit.destination_component,
        ComponentId::new("router-01").expect("component")
    );
    assert_eq!(transit.serialization_us, 12_160);
    assert_eq!(transit.arrival_us, 14_160);
}

#[test]
fn connector_enforces_mtu_loss_and_operational_state() {
    let mut endpoint_a = endpoint("switch-01", "ethernet-1");
    endpoint_a.profile.settings.mtu = 100;
    let mut endpoint_b = endpoint("router-01", "ethernet-1");
    endpoint_b.profile.settings.mtu = 100;
    let mut connector = MediaLink::new(
        ComponentId::new("connection-01").expect("valid ID"),
        endpoint_a,
        endpoint_b,
        MediaLinkConfig {
            loss_every: Some(2),
            ..MediaLinkConfig::default()
        },
        virtual_medium(),
    )
    .expect("valid connection");
    let source = ComponentId::new("switch-01").expect("component");
    let port = PortId::new("ethernet-1").expect("port");
    assert_eq!(
        connector.transmit(&source, &port, &frame(123), 0),
        Err(MediaDropReason::MtuExceeded {
            wire_bytes: 123,
            maximum: 122,
        })
    );
    assert!(connector.transmit(&source, &port, &frame(100), 0).is_ok());
    assert_eq!(
        connector.transmit(&source, &port, &frame(100), 0),
        Err(MediaDropReason::ModeledLoss)
    );
    connector.set_operational(false);
    assert_eq!(
        connector.transmit(&source, &port, &frame(100), 0),
        Err(MediaDropReason::Down)
    );
}

#[test]
fn physical_media_exclusivity_matches_medium_type() {
    assert!(
        ConnectionMedium::Copper {
            config: CopperMedium {
                wiring: CopperWiring::StraightThrough,
                category: CopperCategory::Cat6a,
                length_m: 10.0,
            },
        }
        .requires_exclusive_endpoints()
    );
    assert!(!virtual_medium().requires_exclusive_endpoints());
    assert!(
        !ConnectionMedium::Radio {
            config: RadioMedium {
                standard: "IEEE 802.11ax".into(),
                ssid: "test".into(),
                security: "WPA3-Enterprise".into(),
                distance_m: 10.0,
            },
        }
        .requires_exclusive_endpoints()
    );
}

#[test]
fn transport_rejects_incompatible_medium() {
    let source = r#"
schema_version: "0.2.0"
id: "invalid-radio-link"
label: "Invalid radio link"
transport: "ethernet"
medium:
  type: "radio"
  standard: "IEEE 802.11ax"
  ssid: "test"
  security: "WPA3-Enterprise"
  distance_m: 10.0
endpoints:
  a:
    appliance: "endpoint-a"
    interface: "radio-0"
  b:
    appliance: "endpoint-b"
    interface: "radio-0"
properties:
  capacity_mbps: 300
"#;
    let error = ConnectionConfig::from_yaml(source).expect_err("must reject mismatch");
    assert!(error.to_string().contains("incompatible"));
}

#[test]
fn connector_rejects_down_ports_and_reverse_direction() {
    let mut source_endpoint = endpoint("switch-01", "span");
    source_endpoint.profile.state.initial_operational = PortState::Down;
    let mut down = MediaLink::new(
        ComponentId::new("connection-01").expect("valid ID"),
        source_endpoint,
        endpoint("sensor-01", "capture"),
        MediaLinkConfig::default(),
        virtual_medium(),
    )
    .expect("valid connection");
    let source = ComponentId::new("switch-01").expect("component");
    let source_port = PortId::new("span").expect("port");
    assert_eq!(
        down.transmit(&source, &source_port, &frame(64), 0),
        Err(MediaDropReason::SourcePortDown)
    );

    let mut directional = link(MediaLinkConfig {
        direction: LinkDirection::AToB,
        ..MediaLinkConfig::default()
    });
    assert!(
        directional
            .transmit(
                &ComponentId::new("switch-01").expect("component"),
                &PortId::new("ethernet-1").expect("port"),
                &frame(64),
                0,
            )
            .is_ok()
    );
    assert_eq!(
        directional.transmit(
            &ComponentId::new("router-01").expect("component"),
            &PortId::new("ethernet-1").expect("port"),
            &frame(64),
            0,
        ),
        Err(MediaDropReason::DirectionDenied)
    );
}

#[test]
fn repository_rejects_mismatched_aggregate_member_vlans() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config");
    let appliance_root = root.join("appliances");
    let access_switch =
        appliance_root.join("central-office/business-it/users/business-it-usr-sw-02.yaml");
    let source = fs::read_to_string(&access_switch).expect("access switch source");
    let invalid = source.replacen(
        "      - 70\n      - 999\n  - id: \"core-02\"",
        "  - id: \"core-02\"",
        1,
    );
    let appliances =
        ConfigRepository::load_with_override(&appliance_root, Some((&access_switch, &invalid)))
            .expect("locally valid asymmetric trunk");
    let error = ConnectionRepository::load(root.join("connections"), &appliances)
        .expect_err("LACP VLAN mismatch must fail");
    assert!(
        error
            .to_string()
            .contains("matching mode, speed, duplex, MTU, and VLANs")
    );
}

#[test]
fn repository_requires_an_operational_bidirectional_firewall_sync_link() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config");
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliances");
    let sync =
        root.join("connections/central-office/business-it/boundary/business-frw-03-ha-sync.yaml");
    let source = fs::read_to_string(&sync).expect("firewall sync source");
    let invalid = source.replace("direction: \"bidirectional\"", "direction: \"a-to-b\"");

    let error = ConnectionRepository::load_with_override(
        root.join("connections"),
        &appliances,
        Some((&sync, &invalid)),
    )
    .expect_err("one-way firewall sync must fail");
    assert!(
        error
            .to_string()
            .contains("operational bidirectional Ethernet")
    );
}
