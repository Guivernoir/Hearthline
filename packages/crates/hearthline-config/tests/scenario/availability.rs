use std::path::PathBuf;

use hearthline_config::{
    ConfigRepository, ConnectionRepository, FirewallHaRole, FirstHopRole, ScenarioContinuityFault,
    ScenarioExpectationMode, ScenarioFirstHopOverride, ScenarioRepository, ScenarioStatus,
    SpanningTreePortRole, SpanningTreePortState, run_scenario, run_scenario_with_state_overrides,
};

fn project_config() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config")
}

#[test]
fn business_core_recovery_moves_forwarding_to_core_two() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliances");
    let connections =
        ConnectionRepository::load(root.join("connections"), &appliances).expect("connections");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenarios");
    let scenario = &scenarios
        .get("business-it-core-failover-dns")
        .expect("core failover scenario")
        .config;

    let baseline = run_scenario(&appliances, &connections, scenario, None).expect("baseline");
    assert!(baseline.expectation_met, "{:#?}", baseline.trace);
    assert_eq!(baseline.expectation_mode, ScenarioExpectationMode::Baseline);
    assert!(baseline.trace.iter().any(|entry| {
        entry.component == "business-it-core-sw-01" && entry.summary.contains("UDP")
    }));
    for (appliance, vlan) in [("business-it-usr-sw-02", 30), ("business-it-srv-sw-02", 20)] {
        let state = baseline
            .spanning_tree_states
            .iter()
            .find(|state| {
                state.appliance == appliance && state.interface == "core-02" && state.vlan == vlan
            })
            .expect("baseline aggregate member state");
        assert_eq!(state.role, SpanningTreePortRole::Root);
        assert_eq!(state.state, SpanningTreePortState::Forwarding);
        assert_eq!(state.root_bridge, "business-it-core-sw-01");
    }
    let baseline_members = baseline
        .link_aggregation_states
        .iter()
        .filter(|state| {
            matches!(
                state.logical_id.as_str(),
                "business-it-usr-sw-02-uplink" | "business-it-srv-sw-02-uplink"
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(baseline_members.len(), 8);
    assert!(baseline_members.iter().all(|state| {
        state.selected
            && state.collecting
            && state.distributing
            && state.bundle_operational
            && state.active_members == 2
    }));

    let recovery = scenario.recovery.as_ref().expect("recovery preset");
    let report = run_scenario_with_state_overrides(
        &appliances,
        &connections,
        scenario,
        None,
        Some(recovery.connection_overrides.clone()),
        Some(recovery.first_hop_overrides.clone()),
        Some(recovery.firewall_ha_overrides.clone()),
    )
    .expect("recovered execution");

    assert!(report.expectation_met, "{:#?}", report.trace);
    assert!(matches!(report.status, ScenarioStatus::Passed));
    assert_eq!(report.expectation_mode, ScenarioExpectationMode::Recovery);
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-it-core-sw-02" && entry.summary.contains("UDP")
    }));
    assert!(!report.trace.iter().any(|entry| {
        entry.component == "business-it-core-sw-01" && entry.summary.contains("UDP")
    }));
    assert!(recovery.first_hop_overrides.iter().all(|expected| {
        report.first_hop_states.iter().any(|state| {
            state.appliance == expected.appliance
                && state.interface == expected.interface
                && state.role == expected.role
        })
    }));
    assert!(report.link_aggregation_states.iter().all(|state| {
        if !matches!(
            state.logical_id.as_str(),
            "business-it-usr-sw-02-uplink" | "business-it-srv-sw-02-uplink"
        ) {
            return true;
        }
        if state.connection.ends_with("core-01") {
            !state.selected && !state.distributing
        } else {
            state.selected
                && state.distributing
                && state.bundle_operational
                && state.active_members == 1
        }
    }));
    assert!(report.link_aggregation_states.iter().any(|state| {
        state.appliance == "business-it-core-sw-02" && state.distributing && !state.peer_forwarding
    }));
    for (appliance, vlan) in [("business-it-usr-sw-02", 30), ("business-it-srv-sw-02", 20)] {
        let state = report
            .spanning_tree_states
            .iter()
            .find(|state| {
                state.appliance == appliance && state.interface == "core-02" && state.vlan == vlan
            })
            .expect("recovered root port state");
        assert_eq!(state.role, SpanningTreePortRole::Root);
        assert_eq!(state.state, SpanningTreePortState::Forwarding);
        assert_eq!(state.root_bridge, "business-it-core-sw-01");
    }
}

#[test]
fn scenario_rejects_two_active_members_of_one_gateway_group() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliances");
    let connections =
        ConnectionRepository::load(root.join("connections"), &appliances).expect("connections");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenarios");
    let scenario = &scenarios
        .get("business-it-core-failover-dns")
        .expect("core failover scenario")
        .config;

    let error = run_scenario_with_state_overrides(
        &appliances,
        &connections,
        scenario,
        None,
        None,
        Some(vec![ScenarioFirstHopOverride {
            appliance: "business-it-core-sw-02".into(),
            interface: "vlan-30".into(),
            role: FirstHopRole::Active,
        }]),
        None,
    )
    .expect_err("split-brain state must fail closed");

    assert!(error.to_string().contains("activates more than one member"));
}

#[test]
fn northbound_firewall_recovery_transfers_virtual_ownership_to_member_b() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliances");
    let connections =
        ConnectionRepository::load(root.join("connections"), &appliances).expect("connections");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenarios");
    let scenario = &scenarios
        .get("business-northbound-firewall-failover")
        .expect("northbound firewall failover scenario")
        .config;

    let baseline = run_scenario(&appliances, &connections, scenario, None).expect("baseline");
    assert!(baseline.expectation_met, "{:#?}", baseline.trace);
    assert_eq!(baseline.expectation_mode, ScenarioExpectationMode::Baseline);
    assert!(baseline.trace.iter().any(|entry| {
        entry.component == "business-frw-03a"
            && entry.summary.contains("permit-historian-analytics-https")
    }));
    assert_eq!(baseline.firewall_ha_states.len(), 2);
    assert!(baseline.firewall_ha_states.iter().all(|state| {
        state.sync_operational
            && state.session_sync
            && ((state.appliance == "business-frw-03a" && state.role == FirewallHaRole::Active)
                || (state.appliance == "business-frw-03b" && state.role == FirewallHaRole::Standby))
    }));

    let recovery = scenario.recovery.as_ref().expect("recovery preset");
    let report = run_scenario_with_state_overrides(
        &appliances,
        &connections,
        scenario,
        None,
        Some(recovery.connection_overrides.clone()),
        Some(recovery.first_hop_overrides.clone()),
        Some(recovery.firewall_ha_overrides.clone()),
    )
    .expect("recovered execution");

    assert!(report.expectation_met, "{:#?}", report.trace);
    assert_eq!(report.expectation_mode, ScenarioExpectationMode::Recovery);
    assert!(matches!(report.status, ScenarioStatus::Passed));
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-frw-03b"
            && entry.summary.contains("permit-historian-analytics-https")
    }));
    assert!(!report.trace.iter().any(|entry| {
        entry.component == "business-frw-03a"
            && entry.summary.contains("permit-historian-analytics-https")
    }));
    assert!(report.firewall_ha_states.iter().all(|state| {
        (state.appliance == "business-frw-03a" && state.role == FirewallHaRole::Standby)
            || (state.appliance == "business-frw-03b" && state.role == FirewallHaRole::Active)
    }));
    assert!(
        report
            .first_hop_states
            .iter()
            .filter(|state| {
                matches!(
                    state.appliance.as_str(),
                    "business-frw-03a" | "business-frw-03b"
                )
            })
            .all(|state| {
                (state.appliance == "business-frw-03a" && state.role == FirstHopRole::Standby)
                    || (state.appliance == "business-frw-03b" && state.role == FirstHopRole::Active)
            })
    );
}

#[test]
fn northbound_firewall_continuity_uses_media_synchronized_session_state() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliances");
    let connections =
        ConnectionRepository::load(root.join("connections"), &appliances).expect("connections");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenarios");
    let scenario = &scenarios
        .get("business-northbound-firewall-session-continuity")
        .expect("firewall continuity scenario")
        .config;

    let report = run_scenario(&appliances, &connections, scenario, None).expect("continuity run");

    assert!(report.expectation_met, "{:#?}", report.trace);
    assert!(matches!(report.status, ScenarioStatus::Passed));
    assert_eq!(report.expectation_mode, ScenarioExpectationMode::Continuity);
    let continuity = report.continuity.as_ref().expect("continuity evidence");
    assert!(continuity.faults.is_empty());
    assert!(continuity.sync_operational_at_failure);
    assert_eq!(continuity.failed_appliance, "business-frw-03a");
    assert_eq!(continuity.promoted_appliance, "business-frw-03b");
    assert_eq!(continuity.last_heartbeat_us, 750_002);
    assert_eq!(continuity.promotion_at_us, 1_500_002);
    assert_eq!(continuity.interruption_us, 500_002);
    assert_eq!(continuity.synchronized_sessions, 1);
    assert_eq!(continuity.sessions_after_continuation, 1);
    assert_eq!(continuity.replicated_updates, 1);
    assert!(continuity.continuation_expectation_met);
    assert!(report.trace.iter().any(|entry| {
        entry.protocol.as_deref() == Some("firewall-ha") && entry.summary.contains("session update")
    }));
    assert!(
        report
            .trace
            .iter()
            .any(|entry| { entry.connection.as_deref() == Some("business-frw-03-ha-sync") })
    );
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-frw-03b"
            && entry
                .summary
                .contains("allowed by existing stateful session")
    }));
    assert!(
        !report
            .trace
            .iter()
            .any(|entry| entry.summary.contains("invalid TCP state"))
    );
    assert!(report.firewall_ha_states.iter().all(|state| {
        (state.appliance == "business-frw-03a" && state.role == FirewallHaRole::Standby)
            || (state.appliance == "business-frw-03b" && state.role == FirewallHaRole::Active)
    }));
}

#[test]
fn northbound_firewall_continues_with_state_synchronized_before_sync_loss() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliances");
    let connections =
        ConnectionRepository::load(root.join("connections"), &appliances).expect("connections");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenarios");
    let scenario = &scenarios
        .get("business-northbound-firewall-ha-sync-loss")
        .expect("HA sync-loss scenario")
        .config;

    let report = run_scenario(&appliances, &connections, scenario, None).expect("sync-loss run");

    assert!(report.expectation_met, "{:#?}", report.trace);
    assert!(matches!(report.status, ScenarioStatus::Passed));
    let continuity = report.continuity.as_ref().expect("continuity evidence");
    assert_eq!(
        continuity.faults,
        vec![ScenarioContinuityFault::SyncLinkLoss { at_us: 600_000 }]
    );
    assert!(!continuity.sync_operational_at_failure);
    assert_eq!(continuity.last_heartbeat_us, 500_002);
    assert_eq!(continuity.promotion_at_us, 1_250_002);
    assert_eq!(continuity.interruption_us, 250_002);
    assert_eq!(continuity.synchronized_sessions, 1);
    assert_eq!(continuity.sessions_after_continuation, 1);
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-frw-03a"
            && entry.time_us == 750_000
            && entry.summary.contains("connection is down")
    }));
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-frw-03b"
            && entry
                .summary
                .contains("allowed by existing stateful session")
    }));
}

#[test]
fn northbound_firewall_fails_closed_after_standby_session_state_loss() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliances");
    let connections =
        ConnectionRepository::load(root.join("connections"), &appliances).expect("connections");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenarios");
    let scenario = &scenarios
        .get("business-northbound-firewall-session-state-loss")
        .expect("session-state-loss scenario")
        .config;

    let report = run_scenario(&appliances, &connections, scenario, None).expect("state-loss run");

    assert!(report.expectation_met, "{:#?}", report.trace);
    assert!(matches!(report.status, ScenarioStatus::Passed));
    let continuity = report.continuity.as_ref().expect("continuity evidence");
    assert_eq!(
        continuity.faults,
        vec![ScenarioContinuityFault::StandbySessionLoss { at_us: 800_000 }]
    );
    assert!(continuity.sync_operational_at_failure);
    assert_eq!(continuity.synchronized_sessions, 0);
    assert_eq!(continuity.sessions_after_continuation, 0);
    assert_eq!(continuity.replicated_updates, 1);
    assert!(continuity.continuation_expectation_met);
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-frw-03b"
            && entry.summary.contains("cleared 1 replicated session")
    }));
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-frw-03b" && entry.summary.contains("denied by default policy")
    }));
    assert!(!report.trace.iter().any(|entry| {
        entry.component == "business-frw-03b"
            && entry
                .summary
                .contains("allowed by existing stateful session")
    }));
}

#[test]
fn northbound_firewall_expires_retained_session_after_long_idle_period() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliances");
    let connections =
        ConnectionRepository::load(root.join("connections"), &appliances).expect("connections");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenarios");
    let scenario = &scenarios
        .get("business-northbound-firewall-stale-session-expiry")
        .expect("stale-session-expiry scenario")
        .config;

    let report = run_scenario(&appliances, &connections, scenario, None).expect("stale-state run");

    assert!(report.expectation_met, "{:#?}", report.trace);
    assert!(matches!(report.status, ScenarioStatus::Passed));
    let continuity = report.continuity.as_ref().expect("continuity evidence");
    assert_eq!(
        continuity.faults,
        vec![ScenarioContinuityFault::SyncLinkLoss { at_us: 600_000 }]
    );
    assert!(!continuity.sync_operational_at_failure);
    assert_eq!(continuity.synchronized_sessions, 1);
    assert_eq!(continuity.sessions_after_continuation, 0);
    assert_eq!(continuity.replicated_updates, 1);
    assert!(continuity.continuation_expectation_met);
    assert!(report.duration_us >= 301_000_000);
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-frw-03b"
            && entry.summary.contains("expired 1 stale stateful session")
    }));
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-frw-03b" && entry.summary.contains("denied by default policy")
    }));
    assert!(!report.trace.iter().any(|entry| {
        entry.component == "business-frw-03b"
            && entry
                .summary
                .contains("allowed by existing stateful session")
    }));
}

#[test]
fn northbound_firewall_fences_standby_during_unconfirmed_peer_isolation() {
    let root = project_config();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliances");
    let connections =
        ConnectionRepository::load(root.join("connections"), &appliances).expect("connections");
    let scenarios = ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)
        .expect("scenarios");
    let scenario = &scenarios
        .get("business-northbound-firewall-isolation-fenced")
        .expect("HA isolation scenario")
        .config;

    let report = run_scenario(&appliances, &connections, scenario, None).expect("isolation run");

    assert!(report.expectation_met, "{:#?}", report.trace);
    assert!(matches!(report.status, ScenarioStatus::Passed));
    assert_eq!(report.expectation_mode, ScenarioExpectationMode::Isolation);
    let isolation = report.ha_isolation.as_ref().expect("isolation evidence");
    assert_eq!(isolation.active_appliance, "business-frw-03a");
    assert_eq!(isolation.standby_appliance, "business-frw-03b");
    assert_eq!(isolation.isolation_at_us, 600_000);
    assert_eq!(isolation.last_heartbeat_us, 500_002);
    assert_eq!(isolation.evaluation_at_us, 1_250_002);
    assert_eq!(isolation.promotion_inhibited_at_us, 1_250_002);
    assert_eq!(isolation.active_members, 1);
    assert_eq!(isolation.standby_sessions, 1);
    assert!(!isolation.sync_operational);
    assert!(!isolation.peer_failure_confirmed);
    assert!(isolation.continuation_expectation_met);
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-frw-03b"
            && entry.summary.contains("promotion inhibited")
            && entry.summary.contains("peer failure is unconfirmed")
    }));
    assert!(!report.trace.iter().any(|entry| {
        entry.component == "business-frw-03b" && entry.summary.contains("firewall HA promoted")
    }));
    assert!(report.trace.iter().any(|entry| {
        entry.component == "business-frw-03a"
            && entry
                .summary
                .contains("allowed by existing stateful session")
    }));
    assert!(report.firewall_ha_states.iter().all(|state| {
        (state.appliance == "business-frw-03a" && state.role == FirewallHaRole::Active)
            || (state.appliance == "business-frw-03b" && state.role == FirewallHaRole::Standby)
    }));
}
