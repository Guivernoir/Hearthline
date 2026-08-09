mod scenario {
    mod autonomy;
    mod availability;
    mod business_workstation;
    mod workstation_session;
}

use std::path::PathBuf;

use hearthline_config::{
    ConfigRepository, ConnectionRepository, ScenarioApplicationConfig, ScenarioConnectionOverride,
    ScenarioExpectationMode, ScenarioPacketConfig, ScenarioRepository, ScenarioStatus,
    ScenarioTransportConfig, SecurityDisposition, run_scenario, run_scenario_with_overrides,
};

fn project_config() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config")
}

#[test]
fn configured_customer_dns_scenario_completes_through_the_isp_path() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliance repository");
    let connections = ConnectionRepository::load(root.join("connections"), &appliances)
        .expect("connection repository");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenario repository");
    let scenario = &scenarios
        .get("customer-dns-lookup")
        .expect("customer DNS scenario")
        .config;

    let report =
        run_scenario(&appliances, &connections, scenario, None).expect("scenario execution");

    assert!(report.expectation_met, "{:#?}", report.trace);
    assert!(matches!(report.status, ScenarioStatus::Passed));
    assert_eq!(report.expectation_mode, ScenarioExpectationMode::Baseline);
    assert_eq!(report.appliance_count, 7);
    assert_eq!(report.link_count, 6);
    assert_eq!(report.statistics.drops, 0, "{:#?}", report.trace);
    assert!(
        report
            .trace
            .iter()
            .any(|entry| entry.summary.contains("PAT 192.168.0.2:53000"))
    );
    assert!(report.trace.iter().any(|entry| {
        entry.component == "customer-pc-01"
            && entry.summary.contains("shop.hearthline.test")
            && entry.summary.contains("192.0.2.10")
    }));
    serde_json::to_string(&report).expect("serializable scenario report");
}

#[test]
fn configured_customer_wan_outage_drops_at_the_down_carrier_connection() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliance repository");
    let connections = ConnectionRepository::load(root.join("connections"), &appliances)
        .expect("connection repository");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenario repository");
    let scenario = &scenarios
        .get("customer-wan-access-outage")
        .expect("customer WAN outage scenario")
        .config;

    let report =
        run_scenario(&appliances, &connections, scenario, None).expect("scenario execution");

    assert!(report.expectation_met, "{:#?}", report.trace);
    assert!(matches!(report.status, ScenarioStatus::Passed));
    assert!(scenario.recovery.is_some());
    assert_eq!(report.expectation_mode, ScenarioExpectationMode::Baseline);
    assert_eq!(report.statistics.drops, 1, "{:#?}", report.trace);
    assert_eq!(report.statistics.deliveries, 0, "{:#?}", report.trace);
    assert!(report.connection_states.iter().any(|connection| {
        connection.id == "customer-cpe-01-to-wan-01" && !connection.operational
    }));
    assert!(report.trace.iter().any(|entry| {
        entry.component == "customer-inet-cpe-01"
            && entry
                .summary
                .contains("media transit failed: connection is down")
    }));
}

#[test]
fn request_override_can_restore_the_canonical_outage_connection() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliance repository");
    let connections = ConnectionRepository::load(root.join("connections"), &appliances)
        .expect("connection repository");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenario repository");
    let scenario = &scenarios
        .get("customer-wan-access-outage")
        .expect("customer WAN outage scenario")
        .config;

    let report = run_scenario_with_overrides(
        &appliances,
        &connections,
        scenario,
        None,
        Some(vec![ScenarioConnectionOverride {
            connection: "customer-cpe-01-to-wan-01".into(),
            operational: true,
        }]),
    )
    .expect("restored scenario execution");

    assert!(report.expectation_met, "{:#?}", report.trace);
    assert!(matches!(report.status, ScenarioStatus::Passed));
    assert_eq!(report.expectation_mode, ScenarioExpectationMode::Recovery);
    assert_eq!(report.statistics.drops, 0, "{:#?}", report.trace);
    assert_eq!(report.statistics.deliveries, 1, "{:#?}", report.trace);
    assert!(
        report
            .connection_states
            .iter()
            .all(|state| state.operational)
    );
}

#[test]
fn request_connection_overrides_reject_invalid_topology_state() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliance repository");
    let connections = ConnectionRepository::load(root.join("connections"), &appliances)
        .expect("connection repository");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenario repository");
    let scenario = &scenarios
        .get("customer-dns-lookup")
        .expect("customer DNS scenario")
        .config;

    let outside_topology = run_scenario_with_overrides(
        &appliances,
        &connections,
        scenario,
        None,
        Some(vec![ScenarioConnectionOverride {
            connection: "business-cpe-01-to-edge-01".into(),
            operational: false,
        }]),
    )
    .expect_err("connection outside the selected topology");
    assert!(
        outside_topology
            .to_string()
            .contains("is not in its selected topology")
    );

    let duplicate = ScenarioConnectionOverride {
        connection: "customer-cpe-01-to-wan-01".into(),
        operational: false,
    };
    let repeated = run_scenario_with_overrides(
        &appliances,
        &connections,
        scenario,
        None,
        Some(vec![duplicate.clone(), duplicate]),
    )
    .expect_err("duplicate connection override");
    assert!(repeated.to_string().contains("repeats connection override"));
}

#[test]
fn configured_customer_https_request_returns_from_the_internal_application() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliance repository");
    let connections = ConnectionRepository::load(root.join("connections"), &appliances)
        .expect("connection repository");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenario repository");
    let scenario = &scenarios
        .get("customer-public-web-request")
        .expect("customer public web scenario")
        .config;

    let report =
        run_scenario(&appliances, &connections, scenario, None).expect("scenario execution");

    assert!(report.expectation_met, "{:#?}", report.trace);
    assert!(matches!(report.status, ScenarioStatus::Passed));
    assert_eq!(report.appliance_count, 16);
    assert_eq!(report.link_count, 15);
    assert_eq!(report.statistics.drops, 0, "{:#?}", report.trace);
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-edge-rtr-01"
            && entry
                .summary
                .contains("static destination NAT 192.0.2.10 -> 172.16.10.2")
    }));
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-frw-01a" && entry.summary.contains("permit-public-https")
    }));
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-web-gw-01"
            && entry.peer.as_deref() == Some("business-it-services-01")
            && entry.summary.contains("shop.hearthline.test/shop")
    }));
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-frw-02a"
            && entry
                .summary
                .contains("permit-web-gateway-to-application-https")
    }));
    assert!(report.trace.iter().any(|entry| {
        entry.component == "customer-pc-01" && entry.summary.contains("received HTTPS response 200")
    }));
    let response = report.http_response.expect("modeled HTTP response");
    assert_eq!(response.status, 200);
    assert_eq!(
        response.document.expect("configured HTTP document").title,
        "Hearthline Store"
    );
}

#[test]
fn configured_customer_management_attempt_is_denied_at_the_perimeter() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliance repository");
    let connections = ConnectionRepository::load(root.join("connections"), &appliances)
        .expect("connection repository");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenario repository");
    let scenario = &scenarios
        .get("customer-public-web-management-denied")
        .expect("customer public management denial scenario")
        .config;

    let report =
        run_scenario(&appliances, &connections, scenario, None).expect("scenario execution");

    assert!(report.expectation_met, "{:#?}", report.trace);
    assert!(matches!(report.status, ScenarioStatus::Passed));
    assert_eq!(report.appliance_count, 12);
    assert_eq!(report.link_count, 11);
    assert_eq!(report.statistics.drops, 1, "{:#?}", report.trace);
    assert_eq!(report.statistics.deliveries, 0, "{:#?}", report.trace);
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-edge-rtr-01"
            && entry.summary.contains("static destination NAT")
    }));
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-frw-01a" && entry.summary.contains("denied by default policy")
    }));
}

#[test]
fn configured_path_traversal_is_prevented_and_projects_security_evidence() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliance repository");
    let connections = ConnectionRepository::load(root.join("connections"), &appliances)
        .expect("connection repository");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenario repository");
    let scenario = &scenarios
        .get("customer-public-web-path-traversal-detected")
        .expect("path traversal exercise")
        .config;

    let report =
        run_scenario(&appliances, &connections, scenario, None).expect("scenario execution");

    assert!(report.expectation_met, "{:#?}", report.trace);
    assert!(matches!(report.status, ScenarioStatus::Passed));
    assert_eq!(report.statistics.drops, 1);
    assert_eq!(report.statistics.deliveries, 0);
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-web-gw-01" && entry.summary.contains("path traversal pattern")
    }));
    let event = report.security.expect("projected security event");
    assert_eq!(event.disposition, SecurityDisposition::Prevented);
    assert_eq!(event.detector, "business-web-gw-01");
    assert_eq!(event.defender, "operations-soc-console-01");
    assert_eq!(event.technique, "path-traversal");
    assert!(event.evidence.contains("path traversal pattern"));
}

#[test]
fn configured_disallowed_http_method_is_prevented_by_yaml_allowlist() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliance repository");
    let connections = ConnectionRepository::load(root.join("connections"), &appliances)
        .expect("connection repository");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenario repository");
    let scenario = &scenarios
        .get("customer-public-web-method-denied")
        .expect("disallowed HTTP method exercise")
        .config;

    let report =
        run_scenario(&appliances, &connections, scenario, None).expect("scenario execution");

    assert!(report.expectation_met, "{:#?}", report.trace);
    assert!(matches!(report.status, ScenarioStatus::Passed));
    assert_eq!(report.statistics.drops, 1);
    assert_eq!(report.statistics.deliveries, 0);
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-web-gw-01"
            && entry.summary.contains("HTTP method is not allowed")
    }));
    let event = report.security.expect("projected security event");
    assert_eq!(event.disposition, SecurityDisposition::Prevented);
    assert_eq!(event.detector, "business-web-gw-01");
    assert_eq!(event.defender, "operations-soc-console-01");
    assert_eq!(event.technique, "unsafe-http-method");
    assert_eq!(event.control, "configured-method-allowlist");
    assert!(event.evidence.contains("HTTP method is not allowed"));
}

#[test]
fn configured_sql_injection_body_is_prevented_by_yaml_inspection_rule() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliance repository");
    let connections = ConnectionRepository::load(root.join("connections"), &appliances)
        .expect("connection repository");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenario repository");
    let scenario = &scenarios
        .get("customer-public-web-sql-injection-detected")
        .expect("SQL injection exercise")
        .config;

    let report =
        run_scenario(&appliances, &connections, scenario, None).expect("scenario execution");

    assert!(report.expectation_met, "{:#?}", report.trace);
    assert!(matches!(report.status, ScenarioStatus::Passed));
    assert_eq!(report.statistics.drops, 1);
    assert_eq!(report.statistics.deliveries, 0);
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-web-gw-01"
            && entry.summary.contains("SQL injection tautology pattern")
    }));
    let event = report.security.expect("projected security event");
    assert_eq!(event.disposition, SecurityDisposition::Prevented);
    assert_eq!(event.technique, "sql-injection");
    assert_eq!(event.control, "configured-request-inspection");
    assert!(event.evidence.contains("sql-injection-tautology"));
}

#[test]
fn configured_factory_operations_data_reaches_central_analytics() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliance repository");
    let connections = ConnectionRepository::load(root.join("connections"), &appliances)
        .expect("connection repository");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenario repository");
    let scenario = &scenarios
        .get("factory-operations-data")
        .expect("factory operations-data scenario")
        .config;

    let report =
        run_scenario(&appliances, &connections, scenario, None).expect("scenario execution");

    assert!(report.expectation_met, "{:#?}", report.trace);
    assert!(matches!(report.status, ScenarioStatus::Passed));
    assert_eq!(report.appliance_count, 7);
    assert_eq!(report.link_count, 6);
    assert_eq!(report.statistics.drops, 0, "{:#?}", report.trace);
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-frw-03a"
            && entry.summary.contains("permit-historian-analytics-https")
    }));
    assert!(report.trace.iter().any(|entry| {
        entry.component == "operations-analytics-01"
            && entry.protocol.as_deref() == Some("analytics")
    }));
}

#[test]
fn configured_factory_ssh_attempt_is_denied_at_the_northbound_firewall() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliance repository");
    let connections = ConnectionRepository::load(root.join("connections"), &appliances)
        .expect("connection repository");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenario repository");
    let scenario = &scenarios
        .get("factory-operations-data-denied")
        .expect("factory operations-data denial scenario")
        .config;

    let report =
        run_scenario(&appliances, &connections, scenario, None).expect("scenario execution");

    assert!(report.expectation_met, "{:#?}", report.trace);
    assert!(matches!(report.status, ScenarioStatus::Passed));
    assert_eq!(report.appliance_count, 7);
    assert_eq!(report.link_count, 6);
    assert_eq!(report.statistics.drops, 1, "{:#?}", report.trace);
    assert_eq!(report.statistics.deliveries, 0, "{:#?}", report.trace);
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-frw-03a" && entry.summary.contains("denied by default policy")
    }));
}

#[test]
fn dns_packet_override_preserves_scenario_constraints() {
    let packet = ScenarioPacketConfig {
        source_ip: "192.168.0.2".into(),
        destination_ip: "198.51.100.50".into(),
        ttl: 64,
        wire_length_bytes: 96,
        transport: ScenarioTransportConfig::Udp {
            source_port: 53000,
            destination_port: 5353,
        },
        application: ScenarioApplicationConfig::DnsQuery {
            name: "shop.hearthline.test".into(),
        },
    };

    assert!(
        packet
            .validate()
            .expect_err("invalid DNS destination port")
            .to_string()
            .contains("destination port 53")
    );
}

#[test]
fn every_canonical_scenario_meets_its_baseline_expectation() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliance repository");
    let connections = ConnectionRepository::load(root.join("connections"), &appliances)
        .expect("connection repository");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenario repository");

    for loaded in scenarios.scenarios() {
        let report = run_scenario(&appliances, &connections, &loaded.config, None)
            .unwrap_or_else(|error| panic!("scenario {} failed: {error}", loaded.config.id));
        assert!(
            report.expectation_met && matches!(report.status, ScenarioStatus::Passed),
            "scenario {} did not meet its expectation: {:#?}",
            loaded.config.id,
            report.trace
        );
    }
}
