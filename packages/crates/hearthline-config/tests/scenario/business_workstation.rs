use std::path::PathBuf;

use hearthline_config::{
    ConfigRepository, ConnectionRepository, ScenarioRepository, WORKSTATION_DNS_TTL_MS,
    WorkstationAction, WorkstationActionStatus, WorkstationSession, run_workstation_action,
    run_workstation_action_with_session, workstation_profile,
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

#[test]
fn workstation_session_caches_expires_and_flushes_dns_answers() {
    let (appliances, connections, scenarios) = repositories();
    let workstation = "business-it-usr-pc-01";
    let mut session = WorkstationSession::default();
    let browse = || WorkstationAction::Browser {
        url: "https://portal.hearthline.test/".into(),
    };

    let first = run_workstation_action_with_session(
        &appliances,
        &connections,
        &scenarios,
        workstation,
        browse(),
        &mut session,
    )
    .expect("first portal action");
    assert_eq!(first.simulations.len(), 2);
    assert_eq!(
        first
            .browser
            .as_ref()
            .expect("browser result")
            .resolution_source,
        "dns-query"
    );

    let cached = run_workstation_action_with_session(
        &appliances,
        &connections,
        &scenarios,
        workstation,
        browse(),
        &mut session,
    )
    .expect("cached portal action");
    assert_eq!(cached.simulations.len(), 1);
    assert_eq!(
        cached
            .browser
            .as_ref()
            .expect("cached browser result")
            .resolution_source,
        "client-cache"
    );
    assert!(cached.output.iter().any(|line| line.contains("DNS cache")));

    let display = run_workstation_action_with_session(
        &appliances,
        &connections,
        &scenarios,
        workstation,
        WorkstationAction::Terminal {
            command: "ipconfig /displaydns".into(),
        },
        &mut session,
    )
    .expect("display DNS cache");
    assert!(
        display
            .output
            .iter()
            .any(|line| line.contains("portal.hearthline.test"))
    );

    session.tick(WORKSTATION_DNS_TTL_MS);
    let expired = run_workstation_action_with_session(
        &appliances,
        &connections,
        &scenarios,
        workstation,
        browse(),
        &mut session,
    )
    .expect("expired portal action");
    assert_eq!(expired.simulations.len(), 2);

    let flushed = run_workstation_action_with_session(
        &appliances,
        &connections,
        &scenarios,
        workstation,
        WorkstationAction::Terminal {
            command: "ipconfig /flushdns".into(),
        },
        &mut session,
    )
    .expect("flush DNS cache");
    assert!(
        flushed
            .output
            .iter()
            .any(|line| line.contains("1 cached DNS record"))
    );

    let after_flush = run_workstation_action_with_session(
        &appliances,
        &connections,
        &scenarios,
        workstation,
        browse(),
        &mut session,
    )
    .expect("post-flush portal action");
    assert_eq!(after_flush.simulations.len(), 2);
}

#[test]
fn nslookup_queries_the_server_even_when_client_cache_is_populated() {
    let (appliances, connections, scenarios) = repositories();
    let workstation = "business-it-usr-pc-01";
    let mut session = WorkstationSession::default();
    session.remember_dns("portal.hearthline.test", "10.10.80.20");

    let report = run_workstation_action_with_session(
        &appliances,
        &connections,
        &scenarios,
        workstation,
        WorkstationAction::Terminal {
            command: "nslookup portal.hearthline.test".into(),
        },
        &mut session,
    )
    .expect("nslookup action");

    assert_eq!(report.simulations.len(), 1);
    assert_eq!(report.simulations[0].packet.destination_ip, "10.10.20.10");
}
