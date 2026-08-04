use std::path::PathBuf;

use hearthline_config::{
    ConfigRepository, ConnectionRepository, ScenarioRepository, WorkstationAction,
    WorkstationActionStatus, run_workstation_action, workstation_profile,
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
fn business_workstation_profile_uses_internal_configuration() {
    let (appliances, _, scenarios) = repositories();

    let profile = workstation_profile(&appliances, &scenarios, "business-it-usr-pc-01")
        .expect("business workstation profile");

    assert_eq!(profile.site, "Central Office");
    assert_eq!(profile.environment, "Business IT");
    assert_eq!(profile.zone, "IT Users");
    assert_eq!(profile.default_gateway.as_deref(), Some("10.10.30.1"));
    assert_eq!(profile.dns_servers, ["10.10.20.10"]);
    assert_eq!(profile.interfaces[0].addresses, ["10.10.30.101/24"]);
    assert_eq!(
        profile.browser_home.as_deref(),
        Some("https://portal.hearthline.test/")
    );
}

#[test]
fn business_workstation_runs_internal_dns_and_portal_paths() {
    let (appliances, connections, scenarios) = repositories();

    let dns = run_workstation_action(
        &appliances,
        &connections,
        &scenarios,
        "business-it-usr-pc-01",
        WorkstationAction::Terminal {
            command: "nslookup portal.hearthline.test".into(),
        },
    )
    .expect("internal DNS action");
    assert!(matches!(dns.status, WorkstationActionStatus::Succeeded));
    assert_eq!(dns.simulations[0].scenario_id, "business-it-user-pc-01-dns");
    assert!(dns.output.iter().any(|line| line == "Address: 10.10.80.20"));

    let portal = run_workstation_action(
        &appliances,
        &connections,
        &scenarios,
        "business-it-usr-pc-01",
        WorkstationAction::Browser {
            url: "portal.hearthline.test/".into(),
        },
    )
    .expect("internal portal action");
    assert!(matches!(portal.status, WorkstationActionStatus::Succeeded));
    assert_eq!(portal.simulations.len(), 2);
    assert!(portal.simulations.iter().all(|run| run.expectation_met));
    let response = portal
        .browser
        .expect("browser result")
        .response
        .expect("portal response");
    assert_eq!(response.status, 200);
    assert_eq!(
        response.document.expect("portal document").title,
        "Hearthline Employee Portal"
    );
}

#[test]
fn second_business_workstation_uses_independent_scenarios() {
    let (appliances, connections, scenarios) = repositories();

    let profile = workstation_profile(&appliances, &scenarios, "business-it-usr-pc-02")
        .expect("second business workstation profile");
    assert_eq!(profile.interfaces[0].addresses, ["10.10.30.102/24"]);

    let report = run_workstation_action(
        &appliances,
        &connections,
        &scenarios,
        "business-it-usr-pc-02",
        WorkstationAction::Browser {
            url: "https://portal.hearthline.test/".into(),
        },
    )
    .expect("second portal action");

    assert!(matches!(report.status, WorkstationActionStatus::Succeeded));
    assert_eq!(
        report
            .simulations
            .iter()
            .map(|run| run.scenario_id.as_str())
            .collect::<Vec<_>>(),
        [
            "business-it-user-pc-02-dns",
            "business-it-user-pc-02-portal"
        ]
    );
    assert!(
        report
            .simulations
            .iter()
            .all(|run| run.packet.source_ip == "10.10.30.102")
    );
}

#[test]
fn second_access_switch_workstations_use_svi_routed_scenarios() {
    let (appliances, connections, scenarios) = repositories();

    for (workstation, address, suffix) in [
        ("business-it-usr-pc-03", "10.10.30.103/24", "03"),
        ("business-it-usr-pc-04", "10.10.30.104/24", "04"),
    ] {
        let profile =
            workstation_profile(&appliances, &scenarios, workstation).expect("workstation profile");
        assert_eq!(profile.interfaces[0].addresses, [address]);
        assert_eq!(
            profile.browser_home.as_deref(),
            Some("https://portal.hearthline.test/")
        );

        let report = run_workstation_action(
            &appliances,
            &connections,
            &scenarios,
            workstation,
            WorkstationAction::Browser {
                url: "https://portal.hearthline.test/".into(),
            },
        )
        .expect("portal action");

        assert!(matches!(report.status, WorkstationActionStatus::Succeeded));
        assert_eq!(
            report
                .simulations
                .iter()
                .map(|run| run.scenario_id.as_str())
                .collect::<Vec<_>>(),
            [
                format!("business-it-user-pc-{suffix}-dns"),
                format!("business-it-user-pc-{suffix}-portal"),
            ]
        );
        assert!(report.simulations.iter().all(|run| run.expectation_met));
    }
}
