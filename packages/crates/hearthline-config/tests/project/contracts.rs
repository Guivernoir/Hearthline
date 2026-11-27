use std::fs;
use std::path::Path;

use hearthline_config::{
    ApplianceConfig, ConfigRepository, ConnectionConfig, ConnectionRepository, HttpMethodConfig,
    parse_service_kind, service_name,
};
use hearthline_model::{HttpMethod, ServiceKind};
use serde_yaml_ng::Value;

const CONFIG_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../project/config");

fn fixture(path: &str) -> Value {
    let source = fs::read_to_string(Path::new(CONFIG_ROOT).join(path)).expect("fixture file");
    serde_yaml_ng::from_str(&source).expect("fixture YAML")
}

#[test]
fn service_policy_names_round_trip_and_aliases_normalize() {
    for (kind, name) in [
        (ServiceKind::IcmpEcho, "icmp-echo"),
        (ServiceKind::Dns, "dns"),
        (ServiceKind::Dhcp, "dhcp"),
        (ServiceKind::Http, "http"),
        (ServiceKind::Https, "https"),
        (ServiceKind::Ssh, "ssh"),
        (ServiceKind::Rdp, "rdp"),
        (ServiceKind::Snmp, "snmp"),
        (ServiceKind::Syslog, "syslog"),
        (ServiceKind::Ntp, "ntp"),
        (ServiceKind::Pki, "pki"),
        (ServiceKind::Identity, "identity"),
        (ServiceKind::PolicyDecision, "policy-decision"),
        (ServiceKind::FileTransfer, "file-transfer"),
        (ServiceKind::HistorianReplication, "historian-replication"),
        (ServiceKind::Monitoring, "monitoring"),
        (ServiceKind::Backup, "backup"),
        (ServiceKind::Analytics, "analytics"),
        (ServiceKind::VoiceSignaling, "voice-signaling"),
        (ServiceKind::Printing, "printing"),
        (ServiceKind::PlcEngineering, "plc-engineering"),
        (ServiceKind::IndustrialIo, "industrial-io"),
        (ServiceKind::Management, "management"),
        (ServiceKind::Generic, "generic"),
    ] {
        assert_eq!(service_name(kind), name);
        assert_eq!(parse_service_kind(name).expect("canonical name"), kind);
    }
    for (alias, canonical) in [
        ("dns-tcp", "dns"),
        ("dns-udp", "dns"),
        ("sftp", "ssh"),
        ("radius", "identity"),
        ("historian", "historian-replication"),
        ("scada", "monitoring"),
        ("engineering", "plc-engineering"),
    ] {
        assert_eq!(service_name(parse_service_kind(alias).unwrap()), canonical);
    }
    assert!(parse_service_kind("unconfigured-service").is_err());
}

#[test]
fn configured_http_methods_preserve_wire_identity() {
    for (name, runtime) in [
        ("get", HttpMethod::Get),
        ("head", HttpMethod::Head),
        ("post", HttpMethod::Post),
        ("put", HttpMethod::Put),
        ("patch", HttpMethod::Patch),
        ("delete", HttpMethod::Delete),
        ("options", HttpMethod::Options),
    ] {
        let method: HttpMethodConfig = serde_yaml_ng::from_str(name).unwrap();
        assert_eq!(method.runtime(), runtime);
        assert_eq!(method.to_string(), name.to_ascii_uppercase());
    }
}

#[test]
fn invalid_gateway_inspection_rules_are_rejected_before_runtime() {
    let gateway = fixture("appliances/central-office/it-dmz/services/business-web-gw-01.yaml");
    let check = |document: &Value, message: &str| {
        let error = ApplianceConfig::from_yaml(&serde_yaml_ng::to_string(document).unwrap())
            .expect_err("invalid gateway must not load");
        assert!(error.to_string().contains(message), "{error}");
    };

    for field in [
        "listeners",
        "allowed_hosts",
        "allowed_methods",
        "upstreams",
        "inspection_rules",
    ] {
        let mut invalid = gateway.clone();
        invalid["behavior"][field] = Value::Sequence(Vec::new());
        check(&invalid, "requires listeners, allowed hosts");
    }

    let rule = &gateway["behavior"]["inspection_rules"][0];
    let mut invalid = gateway.clone();
    invalid["behavior"]["inspection_rules"] = Value::Sequence(vec![rule.clone(); 17]);
    check(&invalid, "exceeds 16 inspection rules");

    for (field, message) in [
        ("contains", "pattern exceeds"),
        ("reason", "reason exceeds"),
    ] {
        let mut invalid = gateway.clone();
        invalid["behavior"]["inspection_rules"][0][field] = Value::String("x".repeat(97));
        check(&invalid, message);
    }

    let mut invalid = gateway.clone();
    let rules = invalid["behavior"]["inspection_rules"]
        .as_sequence_mut()
        .unwrap();
    rules.push(rule.clone());
    check(&invalid, "repeats inspection rule");
    invalid["behavior"]["inspection_rules"][2]["id"] = "alternate-rule".into();
    check(&invalid, "repeats an inspection signature");
}

#[test]
fn connection_repository_checks_actual_port_capabilities() {
    let root = Path::new(CONFIG_ROOT);
    let appliances = ConfigRepository::load(root.join("appliances")).unwrap();
    let connections = root.join("connections");
    let path = connections.join("customer/lan/customer-pc-01-to-sw-01.yaml");
    let model = fixture("connections/customer/lan/customer-pc-01-to-sw-01.yaml");
    let check = |document: &Value, message: &str| {
        let source = serde_yaml_ng::to_string(document).unwrap();
        ConnectionConfig::from_yaml(&source).expect("individual link document remains valid");
        let error = ConnectionRepository::load_with_override(
            &connections,
            &appliances,
            Some((&path, &source)),
        )
        .expect_err("cross-file endpoint validation must reject link");
        assert!(error.to_string().contains(message), "{error}");
    };

    let mut invalid = model.clone();
    invalid["endpoints"]["a"]["appliance"] = "unknown-customer-endpoint".into();
    check(&invalid, "references missing appliance");
    let mut invalid = model.clone();
    invalid["endpoints"]["a"]["interface"] = "unknown-interface".into();
    check(&invalid, "references missing interface");
    let mut invalid = model.clone();
    invalid["endpoints"]["a"]["appliance"] = "business-it-core-sw-01".into();
    invalid["endpoints"]["a"]["interface"] = "vlan-20".into();
    check(&invalid, "cannot terminate media on virtual SVI");
    let mut invalid = model.clone();
    invalid["medium"] = serde_yaml_ng::from_str(
        "{ type: fiber, mode: single-mode, connector: LC, length_m: 10.0 }",
    )
    .unwrap();
    check(&invalid, "medium is not supported");
    let mut invalid = model;
    invalid["properties"]["capacity_mbps"] = 1_001_u64.into();
    check(
        &invalid,
        "exceeds port customer-pc-01:fast-ethernet-0 configured speed",
    );
}
