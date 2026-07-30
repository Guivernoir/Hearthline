use std::net::Ipv4Addr;

use hearthline_config::{
    ConnectionConfig, ConnectionDirection, ConnectionEndpoint, ConnectionEndpoints,
    ConnectionProperties, ConnectorDropReason, ConnectorPortProfile, SimulatedConnector,
};
use hearthline_engine::{
    ConnectionMedium, CopperCategory, CopperMedium, CopperWiring, PortState, PortStateConfig,
    RadioMedium, VirtualMedium,
};
use hearthline_model::{
    ArpOperation, ArpPacket, ComponentId, EthernetFrame, MacAddress, NetworkPayload, VlanId,
};

fn endpoint(appliance: &str, interface: &str) -> ConnectionEndpoint {
    ConnectionEndpoint {
        appliance: appliance.into(),
        interface: interface.into(),
    }
}

fn frame() -> EthernetFrame {
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
    }
}

#[test]
fn connector_applies_serialization_and_fixed_delay() {
    let a = endpoint("switch-01", "ethernet-1");
    let b = endpoint("router-01", "ethernet-1");
    let mut connector = SimulatedConnector::new(
        ComponentId::new("connection-01").expect("valid ID"),
        ConnectionEndpoints {
            a: a.clone(),
            b: b.clone(),
        },
        ConnectionProperties {
            capacity_mbps: 1,
            latency_ms: 2,
            ..ConnectionProperties::default()
        },
    )
    .expect("valid connector");
    let (destination, transit) = connector
        .transmit(&a, &frame(), 1_500)
        .expect("frame should transit");
    assert_eq!(destination, &b);
    assert_eq!(transit.delay_ms, 14);
}

#[test]
fn connector_enforces_mtu_loss_and_operational_state() {
    let a = endpoint("switch-01", "ethernet-1");
    let b = endpoint("router-01", "ethernet-1");
    let mut connector = SimulatedConnector::new_configured(
        ComponentId::new("connection-01").expect("valid ID"),
        ConnectionEndpoints { a: a.clone(), b },
        ConnectionProperties {
            loss_every: Some(2),
            ..ConnectionProperties::default()
        },
        ConnectionMedium::Virtual {
            config: VirtualMedium {
                technology: "test bridge".into(),
            },
        },
        ConnectorPortProfile {
            mtu: 100,
            ..ConnectorPortProfile::default()
        },
        ConnectorPortProfile {
            mtu: 100,
            ..ConnectorPortProfile::default()
        },
    )
    .expect("valid connector");
    assert_eq!(
        connector.transmit(&a, &frame(), 101),
        Err(ConnectorDropReason::MtuExceeded {
            frame_bytes: 101,
            mtu: 100
        })
    );
    assert!(connector.transmit(&a, &frame(), 100).is_ok());
    assert_eq!(
        connector.transmit(&a, &frame(), 100),
        Err(ConnectorDropReason::ModeledLoss)
    );
    connector.set_operational(false);
    assert_eq!(
        connector.transmit(&a, &frame(), 100),
        Err(ConnectorDropReason::Down)
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
    assert!(
        !ConnectionMedium::Virtual {
            config: VirtualMedium {
                technology: "test bridge".into(),
            },
        }
        .requires_exclusive_endpoints()
    );
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
    let a = endpoint("switch-01", "span");
    let b = endpoint("sensor-01", "capture");
    let mut down = SimulatedConnector::new_configured(
        ComponentId::new("connection-01").expect("valid ID"),
        ConnectionEndpoints {
            a: a.clone(),
            b: b.clone(),
        },
        ConnectionProperties::default(),
        ConnectionMedium::Virtual {
            config: VirtualMedium {
                technology: "test bridge".into(),
            },
        },
        ConnectorPortProfile {
            state: PortStateConfig {
                administrative: PortState::Down,
                initial_operational: PortState::Down,
            },
            ..ConnectorPortProfile::default()
        },
        ConnectorPortProfile::default(),
    )
    .expect("valid connector");
    assert_eq!(
        down.transmit(&a, &frame(), 64),
        Err(ConnectorDropReason::SourcePortDown)
    );

    let mut directional = SimulatedConnector::new(
        ComponentId::new("mirror-connection-01").expect("valid ID"),
        ConnectionEndpoints {
            a: a.clone(),
            b: b.clone(),
        },
        ConnectionProperties {
            direction: ConnectionDirection::AToB,
            ..ConnectionProperties::default()
        },
    )
    .expect("valid connector");
    assert!(directional.transmit(&a, &frame(), 64).is_ok());
    assert_eq!(
        directional.transmit(&b, &frame(), 64),
        Err(ConnectorDropReason::InvalidEndpoint)
    );
}
