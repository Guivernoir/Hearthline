use std::path::PathBuf;

use hearthline_config::{
    ConfigRepository, ConnectionRepository, InteractiveScenarioSession, ScenarioRepository,
    WorkstationAction, WorkstationActionKind, WorkstationActionStatus, WorkstationSession,
    run_workstation_action_with_session,
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

fn browse() -> WorkstationAction {
    WorkstationAction::Browser {
        url: "https://shop.hearthline.test/shop".into(),
    }
}

#[test]
fn customer_session_retains_and_expires_arp_and_pat_state() {
    let (appliances, connections, scenarios) = repositories();
    let mut session = WorkstationSession::default();

    let first = run_workstation_action_with_session(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-01",
        browse(),
        &mut session,
    )
    .expect("first browser action");
    assert_eq!(first.simulations.len(), 2);
    assert!(first.network_state.active);
    assert_eq!(first.network_state.pat_translations, 2);
    assert_eq!(first.network_state.arp_entries.len(), 1);
    assert_eq!(first.network_state.arp_entries[0].address, "192.168.0.1");
    let customer_switch = first
        .network_state
        .devices
        .iter()
        .find(|device| device.id == "customer-sw-01")
        .expect("customer switch runtime");
    assert_eq!(customer_switch.mac_table.len(), 2);
    let customer_router = first
        .network_state
        .devices
        .iter()
        .find(|device| device.id == "customer-rtr-01")
        .expect("customer router runtime");
    assert_eq!(customer_router.neighbors.len(), 2);
    assert_eq!(customer_router.pat_translations.len(), 2);
    let business_firewall = first
        .network_state
        .devices
        .iter()
        .find(|device| device.id == "business-frw-01a")
        .expect("business firewall runtime");
    assert_eq!(business_firewall.firewall_sessions.len(), 1);
    assert!(first.simulations[0].trace.iter().any(|entry| {
        entry.component == "customer-pc-01" && entry.protocol.as_deref() == Some("arp")
    }));
    assert!(!first.simulations[1].trace.iter().any(|entry| {
        entry.component == "customer-pc-01" && entry.protocol.as_deref() == Some("arp")
    }));

    let inspection = run_workstation_action_with_session(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-01",
        WorkstationAction::Inspect {
            appliance: "customer-rtr-01".into(),
            command: "show ip nat translations".into(),
        },
        &mut session,
    )
    .expect("PAT inspection");
    assert!(matches!(inspection.action, WorkstationActionKind::Inspect));
    assert!(matches!(
        inspection.status,
        WorkstationActionStatus::Completed
    ));
    assert!(
        inspection.output.iter().any(|line| {
            line.contains("192.168.0.2:55000") && line.contains("203.0.113.2:49153")
        })
    );

    let unknown = run_workstation_action_with_session(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-01",
        WorkstationAction::Inspect {
            appliance: "unknown-rtr-01".into(),
            command: "show status".into(),
        },
        &mut session,
    )
    .expect("unknown runtime appliance");
    assert!(matches!(
        unknown.status,
        WorkstationActionStatus::Unsupported
    ));

    let second = run_workstation_action_with_session(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-01",
        browse(),
        &mut session,
    )
    .expect("cached browser action");
    assert_eq!(second.simulations.len(), 1);
    assert_eq!(second.network_state.pat_translations, 2);
    assert_eq!(second.network_state.arp_entries.len(), 1);
    assert!(
        second.simulations[0]
            .trace
            .iter()
            .all(|entry| entry.protocol.as_deref() != Some("arp"))
    );

    let arp = run_workstation_action_with_session(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-01",
        WorkstationAction::Terminal {
            command: "arp -a".into(),
        },
        &mut session,
    )
    .expect("ARP table");
    assert!(arp.simulations.is_empty());
    assert!(arp.output.iter().any(|line| line.contains("192.168.0.1")));

    session.tick(1_200_001);
    let expired = session.network_state().expect("expired network state");
    assert!(expired.arp_entries.is_empty());
    assert_eq!(expired.pat_translations, 0);
    assert!(expired.devices.iter().all(|device| {
        device.mac_table.is_empty()
            && device.neighbors.is_empty()
            && device.pat_translations.is_empty()
            && device.firewall_sessions.is_empty()
    }));

    let probe = run_workstation_action_with_session(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-01",
        WorkstationAction::Terminal {
            command: "ping -n 1 198.51.100.50".into(),
        },
        &mut session,
    )
    .expect("post-expiry probe");
    assert!(probe.simulations[0].trace.iter().any(|entry| {
        entry.component == "customer-pc-01" && entry.protocol.as_deref() == Some("arp")
    }));
    assert!(
        probe.simulations[0].duration_us < 1_000_000,
        "trace time was not normalized: {} us",
        probe.simulations[0].duration_us
    );
    assert_eq!(probe.network_state.pat_translations, 1);
}

#[test]
fn interactive_session_rejects_controlled_resilience_scenarios() {
    let (appliances, connections, scenarios) = repositories();
    let mut session = InteractiveScenarioSession::from_source(
        &appliances,
        &connections,
        &scenarios,
        "customer-pc-01",
    )
    .expect("interactive session");
    let outage = &scenarios
        .get("customer-wan-access-outage")
        .expect("outage scenario")
        .config;

    let error = session
        .run(&appliances, &connections, outage, None)
        .expect_err("controlled outage must not enter an interactive session");

    assert!(error.to_string().contains("is not compatible"));
}
