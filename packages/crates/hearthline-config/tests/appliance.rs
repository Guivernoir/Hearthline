use std::fs;
use std::path::PathBuf;

use hearthline_config::{ApplianceConfig, ConfigRepository};
use hearthline_model::{BehaviorFamily, ComponentKind};

const SWITCH: &str = r#"
schema_version: 0.9.0
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
fn spanning_tree_requires_a_standard_priority_and_unicast_bridge_mac() {
    let configured = SWITCH.replace(
        "render:",
        "spanning_tree:\n  protocol: rapid-pvst\n  bridge_priority: 24576\n  bridge_mac: 02:00:00:00:00:01\nrender:",
    );
    ApplianceConfig::from_yaml(&configured).expect("valid Rapid-PVST bridge");

    let invalid_priority = configured.replace("bridge_priority: 24576", "bridge_priority: 25000");
    assert!(
        ApplianceConfig::from_yaml(&invalid_priority)
            .expect_err("nonstandard bridge priority")
            .to_string()
            .contains("multiple of 4096")
    );
    let invalid_mac = configured.replace(
        "bridge_mac: 02:00:00:00:00:01",
        "bridge_mac: ff:ff:ff:ff:ff:ff",
    );
    assert!(
        ApplianceConfig::from_yaml(&invalid_mac)
            .expect_err("multicast bridge MAC")
            .to_string()
            .contains("must be unicast")
    );
}

#[test]
fn link_aggregation_requires_valid_switch_members_and_minimum_links() {
    let configured = SWITCH.replace(
        "render:",
        "link_aggregation:\n  system_mac: 02:00:00:00:00:01\n  groups:\n    - id: po-uplink\n      logical_id: test-uplink\n      protocol: lacp\n      mode: active\n      minimum_active_members: 1\n      members: [ethernet-1]\nrender:",
    );
    ApplianceConfig::from_yaml(&configured).expect("valid LACP aggregate");

    let invalid_minimum =
        configured.replace("minimum_active_members: 1", "minimum_active_members: 2");
    assert!(
        ApplianceConfig::from_yaml(&invalid_minimum)
            .expect_err("minimum exceeds member count")
            .to_string()
            .contains("minimum active members")
    );
    let invalid_member = configured.replace("members: [ethernet-1]", "members: [missing]");
    assert!(
        ApplianceConfig::from_yaml(&invalid_member)
            .expect_err("unknown aggregate member")
            .to_string()
            .contains("unknown interface")
    );
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

#[test]
fn interface_mac_and_default_gateway_are_structurally_validated() {
    let invalid_mac = SWITCH.replace(
        "hardware: ethernet-rj45",
        "hardware: ethernet-rj45\n    mac_address: ff:ff:ff:ff:ff:ff",
    );
    let error = ApplianceConfig::from_yaml(&invalid_mac).expect_err("broadcast MAC must fail");
    assert!(error.to_string().contains("unicast MAC"));

    let invalid_gateway = SWITCH.replace(
        "summary: Valid parser fixture",
        "summary: Valid parser fixture\ndefault_gateway: 192.0.2.1",
    );
    let error =
        ApplianceConfig::from_yaml(&invalid_gateway).expect_err("off-link gateway must fail");
    assert!(error.to_string().contains("not on-link"));
}

#[test]
fn impaired_link_rejects_a_zero_loss_interval() {
    let invalid = SWITCH
        .replace("kind: layer-2-switch", "kind: wan-circuit")
        .replace("hardware: ethernet-rj45", "hardware: carrier-demarc")
        .replace("mode: access", "mode: transparent")
        .replace(
            "family: ethernet-switch\n  vlans: [10]\n  management_vlan: 10\n  spanning_tree: true",
            "family: impaired-link\n  operational: true\n  delay_ms: 10\n  loss_every: 0",
        );
    let error = ApplianceConfig::from_yaml(&invalid).expect_err("zero interval must fail");
    assert!(error.to_string().contains("must be non-zero"));
}

#[test]
fn svi_requires_a_virtual_layer_three_interface() {
    let invalid = SWITCH
        .replace("kind: layer-2-switch", "kind: layer-3-switch")
        .replace("hardware: ethernet-rj45", "hardware: virtual-nic")
        .replace("mode: access", "mode: svi")
        .replace(
            "family: ethernet-switch\n  vlans: [10]\n  management_vlan: 10\n  spanning_tree: true",
            "family: router\n  routes: []\n  forwarding: true",
        );
    let error = ApplianceConfig::from_yaml(&invalid).expect_err("incomplete SVI must fail");
    assert!(error.to_string().contains("SVI"));
}

#[test]
fn repository_rejects_duplicate_vrrp_member_priorities() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config/appliances");
    let core_two = root.join("central-office/business-it/core/business-it-core-sw-02.yaml");
    let source = fs::read_to_string(&core_two).expect("Core-02 source");
    let invalid = source.replace("priority: 100", "priority: 110");

    let error = ConfigRepository::load_with_override(&root, Some((&core_two, &invalid)))
        .expect_err("duplicate priorities must fail");
    assert!(error.to_string().contains("repeats priority 110"));
}

#[test]
fn repository_rejects_duplicate_spanning_tree_bridge_macs() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config/appliances");
    let core_two = root.join("central-office/business-it/core/business-it-core-sw-02.yaml");
    let source = fs::read_to_string(&core_two).expect("Core-02 source");
    let invalid = source.replace(
        "bridge_mac: \"02:00:00:10:00:02\"",
        "bridge_mac: \"02:00:00:10:00:01\"",
    );

    let error = ConfigRepository::load_with_override(&root, Some((&core_two, &invalid)))
        .expect_err("duplicate bridge MAC must fail");
    assert!(error.to_string().contains("bridge MAC"));
    assert!(error.to_string().contains("is shared by"));
}

#[test]
fn repository_rejects_multi_chassis_peers_with_the_same_role() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config/appliances");
    let core_two = root.join("central-office/business-it/core/business-it-core-sw-02.yaml");
    let source = fs::read_to_string(&core_two).expect("Core-02 source");
    let invalid = source.replace("role: \"secondary\"", "role: \"primary\"");

    let error = ConfigRepository::load_with_override(&root, Some((&core_two, &invalid)))
        .expect_err("multi-chassis roles must differ");
    assert!(error.to_string().contains("opposite roles"));
}

#[test]
fn repository_rejects_firewall_ha_role_and_policy_drift() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config/appliances");
    let member_b = root.join("central-office/security/business-frw-03b.yaml");
    let source = fs::read_to_string(&member_b).expect("firewall B source");

    let split_brain = source.replacen("role: \"standby\"", "role: \"active\"", 1);
    let error = ConfigRepository::load_with_override(&root, Some((&member_b, &split_brain)))
        .expect_err("firewall HA and virtual roles must align");
    assert!(error.to_string().contains("HA role must match"));

    let policy_drift = source.replace("service: \"https\"", "service: \"ssh\"");
    let error = ConfigRepository::load_with_override(&root, Some((&member_b, &policy_drift)))
        .expect_err("firewall HA policy drift must fail");
    assert!(error.to_string().contains("synchronized stateful policy"));
}
