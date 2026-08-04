use std::path::PathBuf;

use hearthline_config::{
    ConfigRepository, ConnectionRepository, ScenarioApplicationConfig, ScenarioHttpMethod,
    ScenarioRepository, WorkstationAction, WorkstationActionStatus, run_workstation_action,
    workstation_profile,
};

fn repositories() -> (ConfigRepository, ConnectionRepository, ScenarioRepository) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config");
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliances");
    let connections =
        ConnectionRepository::load(root.join("connections"), &appliances).expect("connections");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenarios");
    (appliances, connections, scenarios)
}

#[test]
fn customer_workstation_profile_comes_from_appliance_configuration() {
    let (appliances, _, scenarios) = repositories();

    let profile = workstation_profile(&appliances, &scenarios, "customer-pc-01")
        .expect("workstation profile");

    assert_eq!(profile.hostname, "customer-pc-01");
    assert_eq!(profile.default_gateway.as_deref(), Some("192.168.0.1"));
    assert_eq!(profile.dns_servers, ["198.51.100.50"]);
    assert_eq!(profile.interfaces.len(), 1);
    assert_eq!(profile.interfaces[0].addresses, ["192.168.0.2/24"]);
    assert_eq!(profile.interfaces[0].operational_state, "up");
    assert_eq!(
        profile.browser_home.as_deref(),
        Some("https://shop.hearthline.test/shop")
    );
}

#[test]
fn second_customer_workstation_has_an_independent_profile() {
    let (appliances, _, scenarios) = repositories();

    let profile = workstation_profile(&appliances, &scenarios, "customer-pc-02")
        .expect("workstation profile");

    assert_eq!(profile.hostname, "customer-pc-02");
    assert_eq!(profile.default_gateway.as_deref(), Some("192.168.0.1"));
    assert_eq!(profile.dns_servers, ["198.51.100.50"]);
    assert_eq!(profile.interfaces[0].addresses, ["192.168.0.3/24"]);
    assert_eq!(
        profile.interfaces[0].mac_address.as_deref(),
        Some("02:00:00:00:01:03")
    );
}

#[test]
fn nslookup_runs_the_configured_dns_path() {
    let (appliances, connections, scenarios) = repositories();

    let report = run_workstation_action(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-01",
        WorkstationAction::Terminal {
            command: "nslookup shop.hearthline.test".into(),
        },
    )
    .expect("DNS action");

    assert!(matches!(report.status, WorkstationActionStatus::Succeeded));
    assert_eq!(report.simulations.len(), 1);
    assert!(report.simulations[0].expectation_met);
    assert!(
        report
            .output
            .iter()
            .any(|line| line == "Address: 192.0.2.10")
    );
}

#[test]
fn browser_runs_dns_then_https_with_an_interactive_path() {
    let (appliances, connections, scenarios) = repositories();

    let report = run_workstation_action(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-01",
        WorkstationAction::Browser {
            url: "shop.hearthline.test/shop/catalog?line=kiln".into(),
        },
    )
    .expect("browser action");

    assert!(matches!(report.status, WorkstationActionStatus::Succeeded));
    assert_eq!(report.simulations.len(), 2);
    assert!(report.simulations.iter().all(|run| run.expectation_met));
    let browser = report.browser.expect("browser result");
    assert_eq!(browser.resolved_address.as_deref(), Some("192.0.2.10"));
    assert_eq!(
        browser.forwarded_to.as_deref(),
        Some("business-it-services-01")
    );
    assert_eq!(browser.gateway.as_deref(), Some("business-web-gw-01"));
    let response = browser.response.expect("browser HTTP response");
    assert_eq!(response.status, 200);
    assert_eq!(
        response.document.expect("browser document").heading,
        "Ceramic process equipment"
    );
    assert_eq!(browser.path, "/shop/catalog?line=kiln");
}

#[test]
fn curl_path_traversal_runs_the_configured_security_exercise() {
    let (appliances, connections, scenarios) = repositories();

    let report = run_workstation_action(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-01",
        WorkstationAction::Terminal {
            command: "curl https://shop.hearthline.test/shop?file=../../etc/passwd".into(),
        },
    )
    .expect("security exercise action");

    assert!(matches!(report.status, WorkstationActionStatus::Denied));
    assert_eq!(report.simulations.len(), 2);
    assert_eq!(
        report.simulations[1].scenario_id,
        "customer-public-web-path-traversal-detected"
    );
    let event = report.simulations[1]
        .security
        .as_ref()
        .expect("security event");
    assert_eq!(event.detector, "business-web-gw-01");
    assert_eq!(event.defender, "operations-soc-console-01");
    assert!(
        report
            .output
            .iter()
            .any(|line| line.contains("path-traversal"))
    );
}

#[test]
fn curl_delete_runs_the_configured_method_security_exercise() {
    let (appliances, connections, scenarios) = repositories();

    let report = run_workstation_action(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-01",
        WorkstationAction::Terminal {
            command: "curl -X DELETE https://shop.hearthline.test/shop/admin".into(),
        },
    )
    .expect("method security exercise action");

    assert!(matches!(report.status, WorkstationActionStatus::Denied));
    assert_eq!(report.simulations.len(), 2);
    assert_eq!(
        report.simulations[1].scenario_id,
        "customer-public-web-method-denied"
    );
    assert!(matches!(
        report.simulations[1].packet.application,
        ScenarioApplicationConfig::HttpRequest {
            method: ScenarioHttpMethod::Delete,
            ..
        }
    ));
    let event = report.simulations[1]
        .security
        .as_ref()
        .expect("security event");
    assert_eq!(event.technique, "unsafe-http-method");
    assert_eq!(event.control, "configured-method-allowlist");
    assert!(event.evidence.contains("HTTP method is not allowed"));
}

#[test]
fn curl_head_uses_the_normal_configured_https_path() {
    let (appliances, connections, scenarios) = repositories();

    let report = run_workstation_action(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-01",
        WorkstationAction::Terminal {
            command: "curl -I https://shop.hearthline.test/shop".into(),
        },
    )
    .expect("HEAD action");

    assert!(matches!(report.status, WorkstationActionStatus::Succeeded));
    assert_eq!(report.simulations.len(), 2);
    assert_eq!(
        report.simulations[1].scenario_id,
        "customer-public-web-request"
    );
    assert!(matches!(
        report.simulations[1].packet.application,
        ScenarioApplicationConfig::HttpRequest {
            method: ScenarioHttpMethod::Head,
            ..
        }
    ));
    let browser = report.browser.expect("HEAD navigation result");
    assert_eq!(browser.method, "HEAD");
    assert_eq!(browser.response.expect("HEAD response").status, 200);
}

#[test]
fn curl_rejects_unmodeled_http_methods_before_simulation() {
    let (appliances, connections, scenarios) = repositories();

    let report = run_workstation_action(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-01",
        WorkstationAction::Terminal {
            command: "curl -X TRACE https://shop.hearthline.test/shop".into(),
        },
    )
    .expect("unsupported method report");

    assert!(matches!(
        report.status,
        WorkstationActionStatus::Unsupported
    ));
    assert!(report.simulations.is_empty());
    assert!(
        report
            .output
            .iter()
            .any(|line| line.contains("unsupported HTTP method TRACE"))
    );
}

#[test]
fn quoted_curl_data_runs_the_configured_sql_injection_exercise() {
    let (appliances, connections, scenarios) = repositories();

    let report = run_workstation_action(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-01",
        WorkstationAction::Terminal {
            command:
                "curl --data \"username=admin' OR '1'='1\" https://shop.hearthline.test/shop/login"
                    .into(),
        },
    )
    .expect("SQL injection action");

    assert!(matches!(report.status, WorkstationActionStatus::Denied));
    assert_eq!(
        report.simulations[1].scenario_id,
        "customer-public-web-sql-injection-detected"
    );
    assert!(matches!(
        &report.simulations[1].packet.application,
        ScenarioApplicationConfig::HttpRequest {
            method: ScenarioHttpMethod::Post,
            body: Some(body),
            body_bytes: 25,
            ..
        } if body == "username=admin' OR '1'='1"
    ));
    let browser = report.browser.expect("POST navigation result");
    assert_eq!(browser.method, "POST");
    assert_eq!(browser.request_body_bytes, 25);
    assert_eq!(
        report.simulations[1]
            .security
            .as_ref()
            .expect("security event")
            .technique,
        "sql-injection"
    );
}

#[test]
fn benign_quoted_post_body_uses_the_normal_https_path() {
    let (appliances, connections, scenarios) = repositories();

    let report = run_workstation_action(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-01",
        WorkstationAction::Terminal {
            command: "curl -d \"username=operator&password=valid\" https://shop.hearthline.test/shop/login"
                .into(),
        },
    )
    .expect("benign POST action");

    assert!(matches!(report.status, WorkstationActionStatus::Succeeded));
    assert_eq!(
        report.simulations[1].scenario_id,
        "customer-public-web-request"
    );
    assert!(report.simulations[1].security.is_none());
    let browser = report.browser.expect("benign POST navigation result");
    assert_eq!(browser.method, "POST");
    assert_eq!(browser.request_body_bytes, 32);
    assert_eq!(browser.response.expect("HTTP response").status, 200);
}

#[test]
fn terminal_rejects_unterminated_quoted_arguments_before_simulation() {
    let (appliances, connections, scenarios) = repositories();

    let report = run_workstation_action(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-01",
        WorkstationAction::Terminal {
            command: "curl -d \"incomplete https://shop.hearthline.test/shop".into(),
        },
    )
    .expect("quoted argument report");

    assert!(matches!(
        report.status,
        WorkstationActionStatus::Unsupported
    ));
    assert!(report.simulations.is_empty());
    assert!(
        report
            .output
            .iter()
            .any(|line| line.contains("quoted argument is not terminated"))
    );
}

#[test]
fn second_customer_workstation_runs_its_own_dns_and_https_paths() {
    let (appliances, connections, scenarios) = repositories();

    let report = run_workstation_action(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-02",
        WorkstationAction::Browser {
            url: "shop.hearthline.test/shop".into(),
        },
    )
    .expect("browser action");

    assert!(matches!(report.status, WorkstationActionStatus::Succeeded));
    assert_eq!(report.simulations.len(), 2);
    assert_eq!(
        report.simulations[0].scenario_id,
        "customer-pc-02-dns-lookup"
    );
    assert_eq!(
        report.simulations[1].scenario_id,
        "customer-pc-02-public-web-request"
    );
    assert_eq!(report.simulations[0].packet.source_ip, "192.168.0.3");
    assert_eq!(report.simulations[1].packet.source_ip, "192.168.0.3");
    assert!(report.simulations.iter().all(|run| run.expectation_met));
    assert_eq!(
        report
            .browser
            .expect("browser result")
            .response
            .expect("HTTP response")
            .status,
        200
    );
}

#[test]
fn ssh_reports_the_configured_perimeter_denial() {
    let (appliances, connections, scenarios) = repositories();

    let report = run_workstation_action(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-01",
        WorkstationAction::Terminal {
            command: "ssh shop.hearthline.test".into(),
        },
    )
    .expect("SSH action");

    assert!(matches!(report.status, WorkstationActionStatus::Denied));
    assert_eq!(report.simulations.len(), 2);
    assert_eq!(report.simulations[1].packet.destination_ip, "192.0.2.10");
    assert_eq!(report.simulations[1].statistics.drops, 1);
    assert!(
        report
            .output
            .iter()
            .any(|line| line.contains("business-frw-01a"))
    );
}

#[test]
fn second_customer_workstation_ssh_uses_its_own_denial_path() {
    let (appliances, connections, scenarios) = repositories();

    let report = run_workstation_action(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-02",
        WorkstationAction::Terminal {
            command: "ssh shop.hearthline.test".into(),
        },
    )
    .expect("SSH action");

    assert!(matches!(report.status, WorkstationActionStatus::Denied));
    assert_eq!(
        report.simulations[1].scenario_id,
        "customer-pc-02-public-web-management-denied"
    );
    assert_eq!(report.simulations[1].packet.source_ip, "192.168.0.3");
    assert_eq!(report.simulations[1].statistics.drops, 1);
}

#[test]
fn unknown_terminal_commands_do_not_bypass_the_rust_command_contract() {
    let (appliances, connections, scenarios) = repositories();

    let report = run_workstation_action(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-01",
        WorkstationAction::Terminal {
            command: "nmap 192.0.2.10".into(),
        },
    )
    .expect("unsupported action report");

    assert!(matches!(
        report.status,
        WorkstationActionStatus::Unsupported
    ));
    assert!(report.simulations.is_empty());
}
