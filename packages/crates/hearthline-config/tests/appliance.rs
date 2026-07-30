use hearthline_config::ApplianceConfig;
use hearthline_model::{BehaviorFamily, ComponentKind};

const SWITCH: &str = r#"
schema_version: 0.3.0
id: test-switch-01
label: Test SW-01
kind: layer-2-switch
site: test
environment: test-lan
zone: access
role: Test access switch
summary: Valid parser fixture
render:
  - view: test/test-lan
    node: switch
interfaces:
  - id: ethernet-1
    hardware: ethernet-rj45
    state:
      administrative: up
      initial_operational: up
    settings:
      speed_mbps: 1000
      duplex: full
      mtu: 1500
    mode: access
    addresses: []
    vlans: [10]
behavior:
  family: ethernet-switch
  vlans: [10]
  management_vlan: 10
  spanning_tree: true
"#;

#[test]
fn appliance_dispatches_to_typed_behavior() {
    let config = ApplianceConfig::from_yaml(SWITCH).expect("valid switch");
    assert_eq!(config.kind, ComponentKind::Layer2Switch);
    assert_eq!(config.behavior_family(), BehaviorFamily::EthernetSwitch);
}

#[test]
fn kind_and_behavior_must_match() {
    let invalid = SWITCH.replace(
        "family: ethernet-switch\n  vlans: [10]\n  management_vlan: 10\n  spanning_tree: true",
        "family: endpoint\n  accepted_services: []\n  respond_to_icmp: true",
    );
    let error = ApplianceConfig::from_yaml(&invalid).expect_err("must reject mismatch");
    assert!(error.to_string().contains("requires behavior family"));
}

#[test]
fn unknown_fields_are_rejected() {
    let invalid = SWITCH.replace(
        "summary: Valid parser fixture",
        "summary: Valid parser fixture\nmystery: true",
    );
    assert!(ApplianceConfig::from_yaml(&invalid).is_err());
}

#[test]
fn firewall_must_default_deny() {
    let firewall = SWITCH
        .replace("kind: layer-2-switch", "kind: firewall")
        .replace(
            "family: ethernet-switch\n  vlans: [10]\n  management_vlan: 10\n  spanning_tree: true",
            "family: stateful-firewall\n  stateful: true\n  default_action: permit\n  rules: []",
        );
    let error = ApplianceConfig::from_yaml(&firewall).expect_err("must reject permit default");
    assert!(error.to_string().contains("default deny"));
}
