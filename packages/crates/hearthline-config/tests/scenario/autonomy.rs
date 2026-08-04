use std::path::PathBuf;

use hearthline_config::{
    ConfigRepository, ConnectionRepository, ScenarioExpectationMode, ScenarioRepository,
    ScenarioStatus, run_scenario, run_scenario_with_state_overrides,
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
fn factory_control_remains_local_during_complete_conduit_handoff_loss() {
    let (appliances, connections, scenarios) = repositories();
    let scenario = &scenarios
        .get("factory-local-autonomy-conduit-outage")
        .expect("local-autonomy scenario")
        .config;

    let report =
        run_scenario(&appliances, &connections, scenario, None).expect("local-autonomy execution");

    assert!(matches!(report.status, ScenarioStatus::Passed));
    assert!(matches!(
        report.expectation_mode,
        ScenarioExpectationMode::Autonomy
    ));
    assert!(report.expectation_met, "{:#?}", report.trace);
    assert_eq!(report.statistics.drops, 2, "{:#?}", report.trace);
    assert_eq!(report.statistics.deliveries, 0, "{:#?}", report.trace);
    assert!(
        report
            .connection_states
            .iter()
            .any(|state| { state.id == "central-conduit-1-to-ot-dmz-sw-01" && !state.operational })
    );
    assert!(
        report
            .connection_states
            .iter()
            .any(|state| { state.id == "central-conduit-2-to-ot-dmz-sw-02" && !state.operational })
    );

    let autonomy = report.local_autonomy.expect("local-autonomy evidence");
    assert_eq!(autonomy.hmi, "area-01-hmi-01");
    assert_eq!(autonomy.controller, "area-01-vplc-01");
    assert_eq!(autonomy.remote_io, "area-01-rio-01");
    assert_eq!(autonomy.safety_interface, "area-01-intlk-01");
    assert_eq!(autonomy.actuator, "area-01-pmp-01");
    assert_eq!(autonomy.actuator_state, "running");
    assert_eq!(autonomy.outage_connections.len(), 2);
    assert_eq!(autonomy.local_path_connections.len(), 7);
    assert!(autonomy.local_path_operational);
    assert!(autonomy.safety_reset_applied);
    assert!(autonomy.command_applied);
    assert!(autonomy.northbound_expectation_met);
    assert!(autonomy.autonomy_expectation_met);
    assert_eq!(autonomy.control_trace.len(), 6);
    assert_eq!(
        autonomy
            .control_trace
            .iter()
            .map(|entry| entry.component.as_str())
            .collect::<Vec<_>>(),
        [
            "area-01-intlk-01",
            "area-01-intlk-01",
            "area-01-hmi-01",
            "area-01-vplc-01",
            "area-01-rio-01",
            "area-01-pmp-01",
        ]
    );
}

#[test]
fn local_autonomy_contract_rejects_runtime_topology_overrides() {
    let (appliances, connections, scenarios) = repositories();
    let scenario = &scenarios
        .get("factory-local-autonomy-conduit-outage")
        .expect("local-autonomy scenario")
        .config;

    let error = run_scenario_with_state_overrides(
        &appliances,
        &connections,
        scenario,
        None,
        Some(Vec::new()),
        None,
        None,
    )
    .expect_err("controlled local-autonomy scenario must reject request overrides");
    assert!(
        error
            .to_string()
            .contains("does not accept runtime overrides")
    );
}
