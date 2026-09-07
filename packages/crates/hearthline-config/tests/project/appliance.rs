use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use hearthline_config::{ApplianceConfig, ConfigRepository, ProcessViewConfig};
use hearthline_model::{BehaviorFamily, ComponentKind};
use serde_yaml_ng::Value;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

const SWITCH: &str = r#"
schema_version: 0.10.0
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
fn rust_generates_process_topology_from_validated_yaml_and_appliances() {
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project");
    let appliances =
        ConfigRepository::load(project.join("config/appliances")).expect("appliance catalog");
    let process = ProcessViewConfig::load(
        project.join("config/ot/process/architecture.yaml"),
        &appliances,
    )
    .expect("validated process topology");
    let value = serde_json::to_value(
        process
            .into_frontend(&appliances)
            .expect("generated process view"),
    )
    .expect("serializable process view");

    assert_eq!(value["schemaVersion"], "0.3.0");
    assert_eq!(value["generationStatus"], "generated");
    assert_eq!(value["areas"].as_array().map(Vec::len), Some(10));
    assert_eq!(value["networkEdges"].as_array().map(Vec::len), Some(14));
    assert_eq!(value["materialFlow"].as_array().map(Vec::len), Some(9));
    let controlled_drying = value["areas"]
        .as_array()
        .and_then(|areas| {
            areas
                .iter()
                .find(|area| area["routeKey"] == "controlled-drying")
        })
        .expect("controlled drying area");
    assert_eq!(
        controlled_drying["equipment"].as_array().map(Vec::len),
        Some(9)
    );
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

fn switch_mutation_error(mutate: impl FnOnce(&mut Value)) -> hearthline_config::ConfigError {
    let mut document: Value = serde_yaml_ng::from_str(SWITCH).expect("switch YAML");
    mutate(&mut document);
    ApplianceConfig::from_yaml(&serde_yaml_ng::to_string(&document).unwrap())
        .expect_err("invalid switch mutation")
}

#[test]
fn appliance_schema_rejects_interface_state_address_vlan_and_render_drift() {
    type ApplianceMutation = Box<dyn FnOnce(&mut Value)>;
    type MutationCase = (ApplianceMutation, &'static str);

    let cases: Vec<MutationCase> = vec![
        (
            Box::new(|document| document["schema_version"] = Value::String("9.9.9".into())),
            "uses schema",
        ),
        (
            Box::new(|document| {
                let duplicate = document["interfaces"][0].clone();
                document["interfaces"]
                    .as_sequence_mut()
                    .unwrap()
                    .push(duplicate);
            }),
            "repeats interface",
        ),
        (
            Box::new(|document| {
                document["interfaces"][0]["hardware"] = Value::String("telephone-rj11".into());
            }),
            "does not support",
        ),
        (
            Box::new(|document| {
                document["interfaces"][0]["state"]["administrative"] = Value::String("down".into());
            }),
            "administratively down",
        ),
        (
            Box::new(|document| {
                document["interfaces"][0]["settings"]["mtu"] = Value::Number(70_000.into());
            }),
            "MTU exceeds the runtime limit",
        ),
        (
            Box::new(|document| {
                document["interfaces"][0]["addresses"] =
                    serde_yaml_ng::from_str("[192.0.2.2/24, 192.0.2.2/24]").unwrap();
            }),
            "repeats IPv4 address",
        ),
        (
            Box::new(|document| {
                document["interfaces"][0]["vlans"] = serde_yaml_ng::from_str("[10, 10]").unwrap();
            }),
            "repeats VLAN",
        ),
        (
            Box::new(|document| {
                document["interfaces"][0]["vlans"] = serde_yaml_ng::from_str("[4095]").unwrap();
            }),
            "invalid VLAN",
        ),
        (
            Box::new(|document| {
                let duplicate = document["render"][0].clone();
                document["render"]
                    .as_sequence_mut()
                    .unwrap()
                    .push(duplicate);
            }),
            "repeats render binding",
        ),
        (
            Box::new(|document| {
                document["interfaces"][0]["addresses"] =
                    serde_yaml_ng::from_str("[192.0.2.2/24]").unwrap();
                document["default_gateway"] = Value::String("192.0.2.2".into());
            }),
            "cannot be a local address",
        ),
    ];
    for (mutate, expected) in cases {
        let error = switch_mutation_error(mutate);
        assert!(error.to_string().contains(expected), "{expected}: {error}");
    }

    let mut valid: Value = serde_yaml_ng::from_str(SWITCH).unwrap();
    valid["interfaces"][0]["addresses"] = serde_yaml_ng::from_str("[192.0.2.2/24]").unwrap();
    valid["default_gateway"] = Value::String("192.0.2.1".into());
    ApplianceConfig::from_yaml(&serde_yaml_ng::to_string(&valid).unwrap())
        .expect("on-link non-local gateway");
}

#[test]
fn appliance_schema_enforces_dns_record_ownership_and_http_text_capacity() {
    let dns_path = appliance_root().join("internet/provider-services/isp-dns-01.yaml");
    let dns_source = fs::read_to_string(&dns_path).expect("DNS appliance source");
    let mut dns: Value = serde_yaml_ng::from_str(&dns_source).unwrap();
    dns["behavior"]["dns_records"] = Value::Sequence(Vec::new());
    let error = ApplianceConfig::from_yaml(&serde_yaml_ng::to_string(&dns).unwrap())
        .expect_err("DNS server without records");
    assert!(
        error
            .to_string()
            .contains("requires at least one authoritative record")
    );

    let mut not_dns: Value = serde_yaml_ng::from_str(&dns_source).unwrap();
    not_dns["kind"] = Value::String("web-server".into());
    let error = ApplianceConfig::from_yaml(&serde_yaml_ng::to_string(&not_dns).unwrap())
        .expect_err("non-DNS service with authoritative records");
    assert!(error.to_string().contains("is not a DNS server"));

    let portal_path =
        appliance_root().join("central-office/business-it/servers/business-it-portal-01.yaml");
    let portal_source = fs::read_to_string(portal_path).expect("portal source");
    for (field, length) in [
        ("host", 129),
        ("title", 97),
        ("heading", 129),
        ("body", 257),
    ] {
        let mut portal: Value = serde_yaml_ng::from_str(&portal_source).unwrap();
        portal["behavior"]["http_site"][field] = Value::String("x".repeat(length));
        let error = ApplianceConfig::from_yaml(&serde_yaml_ng::to_string(&portal).unwrap())
            .expect_err("oversized HTTP site text");
        assert!(
            error.to_string().contains("runtime text capacity"),
            "{field}: {error}"
        );
    }
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

fn appliance_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config/appliances")
}

fn temporary_appliance_root(label: &str) -> PathBuf {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "hearthline-appliance-{label}-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("temporary appliance root");
    path
}

fn mutation_error(
    relative: &str,
    mutate: impl FnOnce(&mut Value),
) -> hearthline_config::ConfigError {
    let root = appliance_root();
    let path = root.join(relative);
    let mut document: Value =
        serde_yaml_ng::from_str(&fs::read_to_string(&path).expect("appliance source"))
            .expect("appliance YAML");
    mutate(&mut document);
    let source = serde_yaml_ng::to_string(&document).expect("mutated appliance YAML");
    match ConfigRepository::load_with_override(&root, Some((&path, &source))) {
        Err(error) => error,
        Ok(_) => panic!("invalid appliance mutation was accepted for {relative}"),
    }
}

fn remove_key(value: &mut Value, key: &str) {
    value
        .as_mapping_mut()
        .expect("YAML mapping")
        .remove(Value::String(key.into()));
}

#[test]
fn appliance_repository_enforces_nonempty_file_identity_and_unique_ids() {
    let empty = temporary_appliance_root("empty");
    assert!(
        ConfigRepository::load(&empty)
            .expect_err("empty appliance repository")
            .to_string()
            .contains("contains no appliance YAML files")
    );
    fs::remove_dir_all(&empty).expect("remove empty fixture");

    let wrong_name = temporary_appliance_root("wrong-name");
    fs::write(wrong_name.join("wrong.yaml"), SWITCH).expect("wrong-name fixture");
    assert!(
        ConfigRepository::load(&wrong_name)
            .expect_err("wrong appliance filename")
            .to_string()
            .contains("must be named test-switch-01.yaml")
    );
    fs::remove_dir_all(&wrong_name).expect("remove wrong-name fixture");

    let duplicate = temporary_appliance_root("duplicate");
    for directory in ["a", "b"] {
        let directory = duplicate.join(directory);
        fs::create_dir_all(&directory).expect("duplicate fixture directory");
        fs::write(directory.join("test-switch-01.yaml"), SWITCH)
            .expect("duplicate appliance fixture");
    }
    assert!(
        ConfigRepository::load(&duplicate)
            .expect_err("duplicate appliance ID")
            .to_string()
            .contains("duplicate appliance id test-switch-01")
    );
    fs::remove_dir_all(&duplicate).expect("remove duplicate fixture");

    let valid = temporary_appliance_root("valid");
    fs::write(valid.join("test-switch-01.yaml"), SWITCH).expect("valid appliance fixture");
    let repository = ConfigRepository::load(&valid).expect("single-appliance repository");
    assert_eq!(repository.len(), 1);
    assert!(!repository.is_empty());
    assert_eq!(repository.appliances().count(), 1);
    assert!(repository.get("missing").is_none());
    assert!(
        !repository
            .get("test-switch-01")
            .unwrap()
            .revision()
            .is_empty()
    );
    fs::remove_dir_all(&valid).expect("remove valid fixture");
}

#[test]
fn repository_rejects_multi_chassis_identity_and_reciprocity_drift() {
    const CORE_ONE: &str = "central-office/business-it/core/business-it-core-sw-01.yaml";
    const CORE_TWO: &str = "central-office/business-it/core/business-it-core-sw-02.yaml";

    let error = mutation_error(CORE_ONE, |document| {
        document["multi_chassis"]["peer"] = Value::String("missing-core".into());
        document["link_aggregation"]["system_mac"] = Value::String("02:00:00:10:ff:00".into());
    });
    assert!(
        error.to_string().contains("unknown multi-chassis peer"),
        "{error}"
    );

    let error = mutation_error(CORE_TWO, |document| remove_key(document, "multi_chassis"));
    assert!(error.to_string().contains("shared by standalone appliance"));

    let error = mutation_error(CORE_TWO, |document| {
        document["link_aggregation"]["system_mac"] = Value::String("02:00:00:10:ff:02".into());
    });
    assert!(
        error
            .to_string()
            .contains("requires one shared LACP system MAC")
    );

    let error = mutation_error(CORE_TWO, |document| {
        document["multi_chassis"]["domain"] = Value::String("other-core-pair".into());
    });
    assert!(
        error
            .to_string()
            .contains("one reciprocal multi-chassis pair")
    );
}

#[test]
fn repository_rejects_first_hop_and_firewall_ha_contract_drift() {
    const CORE_TWO: &str = "central-office/business-it/core/business-it-core-sw-02.yaml";
    const FIREWALL_A: &str = "central-office/security/business-frw-03a.yaml";
    const FIREWALL_B: &str = "central-office/security/business-frw-03b.yaml";

    let error = mutation_error(CORE_TWO, |document| {
        let interfaces = document["interfaces"]
            .as_sequence_mut()
            .expect("interfaces");
        let first_hop = interfaces
            .iter_mut()
            .find(|interface| interface["first_hop"]["group"].as_u64() == Some(20))
            .expect("VRRP group 20");
        first_hop["first_hop"]["group"] = Value::Number(21.into());
        first_hop["first_hop"]["virtual_mac"] = Value::String("00:00:5e:00:01:15".into());
    });
    assert!(
        error.to_string().contains("requires at least two members"),
        "{error}"
    );

    let error = mutation_error(CORE_TWO, |document| {
        let interfaces = document["interfaces"]
            .as_sequence_mut()
            .expect("interfaces");
        let first_hop = interfaces
            .iter_mut()
            .find(|interface| interface["first_hop"]["group"].as_u64() == Some(20))
            .expect("VRRP group 20");
        first_hop["first_hop"]["initial_role"] = Value::String("active".into());
    });
    assert!(
        error
            .to_string()
            .contains("requires exactly one initial active member"),
        "{error}"
    );

    let error = mutation_error(FIREWALL_A, |document| {
        document["firewall_ha"]["peer"] = Value::String("missing-firewall".into());
    });
    assert!(error.to_string().contains("references unknown HA peer"));

    let error = mutation_error(FIREWALL_B, |document| remove_key(document, "firewall_ha"));
    assert!(error.to_string().contains("does not declare firewall HA"));

    let error = mutation_error(FIREWALL_B, |document| {
        document["firewall_ha"]["failure_hold_ms"] = Value::Number(1_000.into());
    });
    assert!(error.to_string().contains("matching timers"));

    let error = mutation_error(FIREWALL_B, |document| {
        document["firewall_ha"]["monitored_interfaces"]
            .as_sequence_mut()
            .expect("monitored interfaces")
            .reverse();
    });
    assert!(
        error
            .to_string()
            .contains("matching monitored interface order")
    );
}
