use std::path::PathBuf;

mod control_program;

use hearthline_config::{
    ConfigRepository, ConnectionRepository, HmiAction, HmiActionStatus, HmiProcessFault,
    HmiSession, HmiSessionStore, ScenarioApplicationConfig, ScenarioRepository,
    build_forming_telemetry_packet, run_scenario_with_state_overrides,
};

fn repository() -> ConfigRepository {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config/appliances");
    ConfigRepository::load(root).expect("appliances")
}

#[test]
fn body_preparation_hmi_is_built_from_area_configuration() {
    let appliances = repository();
    let session = HmiSession::from_repository(&appliances, "area-01-hmi-01").expect("HMI session");
    let snapshot = session.snapshot();

    assert_eq!(snapshot.environment, "Body Preparation");
    assert_eq!(snapshot.zone, "OT-AREA-01");
    assert_eq!(snapshot.controller, "area-01-vplc-01");
    assert_eq!(snapshot.remote_io, "area-01-rio-01");
    assert_eq!(snapshot.signals.len(), 2);
    assert_eq!(snapshot.actuators.len(), 2);
    assert_eq!(snapshot.safety.len(), 1);
    assert!(snapshot.safety[0].trip_latched);
    assert!(
        snapshot.safety[0]
            .permissives
            .iter()
            .all(|permissive| permissive.satisfied)
    );
    assert_eq!(snapshot.alarms[0].code, "SAFETY-RESET-REQUIRED");
}

#[test]
fn safety_reset_then_command_traverses_the_configured_control_path() {
    let appliances = repository();
    let mut session =
        HmiSession::from_repository(&appliances, "area-01-hmi-01").expect("HMI session");

    let inhibited = session.execute(HmiAction::Command {
        tag: "area-01-pmp-01-command".into(),
        value: "running".into(),
    });
    assert!(matches!(inhibited.status, HmiActionStatus::Denied));
    assert_eq!(inhibited.snapshot.actuators[0].current_state, "stopped");

    let reset = session.execute(HmiAction::ResetSafety {
        safety_id: "area-01-intlk-01".into(),
    });
    assert!(matches!(reset.status, HmiActionStatus::Applied));
    assert!(!reset.snapshot.safety[0].trip_latched);
    assert_eq!(reset.trace.len(), 2);

    let command = session.execute(HmiAction::Command {
        tag: "area-01-pmp-01-command".into(),
        value: "running".into(),
    });
    assert!(matches!(command.status, HmiActionStatus::Applied));
    assert_eq!(command.trace.len(), 4);
    assert_eq!(command.trace[0].component, "area-01-hmi-01");
    assert_eq!(command.trace[1].component, "area-01-vplc-01");
    assert_eq!(command.trace[2].component, "area-01-rio-01");
    assert_eq!(command.trace[3].component, "area-01-pmp-01");
    assert_eq!(
        command
            .snapshot
            .actuators
            .iter()
            .find(|actuator| actuator.command_tag == "area-01-pmp-01-command")
            .expect("pump")
            .current_state,
        "running"
    );
}

#[test]
fn alarms_can_be_acknowledged_and_undeclared_commands_are_denied() {
    let appliances = repository();
    let mut session =
        HmiSession::from_repository(&appliances, "area-01-hmi-01").expect("HMI session");

    let acknowledgement = session.execute(HmiAction::AcknowledgeAlarm {
        alarm_id: "startup-area-01-intlk-01".into(),
    });
    assert!(matches!(acknowledgement.status, HmiActionStatus::Completed));
    assert!(acknowledgement.snapshot.alarms[0].acknowledged);

    session.execute(HmiAction::ResetSafety {
        safety_id: "area-01-intlk-01".into(),
    });
    let denied = session.execute(HmiAction::Command {
        tag: "area-01-unknown-command".into(),
        value: "running".into(),
    });
    assert!(matches!(denied.status, HmiActionStatus::Denied));
    assert!(
        denied
            .snapshot
            .alarms
            .iter()
            .any(|alarm| alarm.code == "HMI-COMMAND-DENIED")
    );
}

#[test]
fn every_process_hmi_executes_each_configured_field_state() {
    const HMI_IDS: [&str; 14] = [
        "area-01-hmi-01",
        "area-02-hmi-01",
        "area-02-hmi-02",
        "area-02-hmi-03",
        "area-02-hmi-04",
        "area-02-scada-01",
        "area-03-hmi-01",
        "area-04-hmi-01",
        "area-05-hmi-01",
        "area-06-hmi-01",
        "area-07-hmi-01",
        "area-08-hmi-01",
        "area-09-hmi-01",
        "area-10-hmi-01",
    ];

    let appliances = repository();
    for hmi_id in HMI_IDS {
        let mut session =
            HmiSession::from_repository(&appliances, hmi_id).expect("interactive HMI");
        let initial = session.snapshot();

        assert_eq!(initial.id, hmi_id);
        assert!(!initial.signals.is_empty(), "{hmi_id} signals");
        assert!(!initial.actuators.is_empty(), "{hmi_id} actuators");
        assert_eq!(initial.safety.len(), 1, "{hmi_id} safety interfaces");
        assert!(
            initial.safety[0]
                .permissives
                .iter()
                .all(|permissive| permissive.satisfied),
            "{hmi_id} startup permissives"
        );

        if initial.safety[0].trip_latched {
            let first_actuator = &initial.actuators[0];
            let denied = session.execute(HmiAction::Command {
                tag: first_actuator.command_tag.clone(),
                value: first_actuator.states[0].clone(),
            });
            assert!(
                matches!(denied.status, HmiActionStatus::Denied),
                "{hmi_id} must inhibit commands before reset"
            );

            let safety_id = initial.safety[0].component_id.clone();
            let reset = session.execute(HmiAction::ResetSafety { safety_id });
            assert!(
                matches!(reset.status, HmiActionStatus::Applied),
                "{hmi_id} safety reset"
            );
        }

        for actuator in &initial.actuators {
            assert!(
                actuator.states.len() >= 2,
                "{} state domain",
                actuator.component_id
            );
            for state in &actuator.states {
                let command = session.execute(HmiAction::Command {
                    tag: actuator.command_tag.clone(),
                    value: state.clone(),
                });
                assert!(
                    matches!(command.status, HmiActionStatus::Applied),
                    "{} state {state}",
                    actuator.component_id
                );
                assert_eq!(
                    command
                        .snapshot
                        .actuators
                        .iter()
                        .find(|candidate| candidate.component_id == actuator.component_id)
                        .expect("commanded actuator")
                        .current_state,
                    *state
                );
                assert_eq!(
                    command
                        .trace
                        .iter()
                        .map(|entry| entry.component.as_str())
                        .collect::<Vec<_>>(),
                    [
                        hmi_id,
                        initial.controller.as_str(),
                        initial.remote_io.as_str(),
                        actuator.component_id.as_str(),
                    ],
                    "{} command path",
                    actuator.component_id
                );
            }
        }
    }
}

#[test]
fn forming_scada_and_module_hmis_expose_their_configured_scopes() {
    let appliances = repository();
    let scada = HmiSession::from_repository(&appliances, "area-02-scada-01")
        .expect("forming SCADA")
        .snapshot();

    assert_eq!(scada.interface_kind, "scada-workstation");
    assert_eq!(scada.signals.len(), 17);
    assert_eq!(scada.actuators.len(), 6);
    assert_eq!(scada.remote_io, "area-02-rio-01");
    assert!(!scada.safety[0].trip_latched);

    for (id, signal_count, actuator_count) in [
        ("area-02-hmi-01", 6, 1),
        ("area-02-hmi-02", 4, 1),
        ("area-02-hmi-03", 5, 3),
        ("area-02-hmi-04", 2, 1),
    ] {
        let module = HmiSession::from_repository(&appliances, id)
            .expect("forming module HMI")
            .snapshot();
        assert_eq!(module.interface_kind, "hmi", "{id} kind");
        assert_eq!(module.signals.len(), signal_count, "{id} signals");
        assert_eq!(module.actuators.len(), actuator_count, "{id} actuators");
        assert_eq!(module.controller, "area-02-vplc-01", "{id} controller");
        assert_eq!(module.remote_io, "area-02-rio-01", "{id} remote I/O");
    }
}

#[test]
fn forming_scada_and_module_hmis_share_one_running_cell() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();

    let ready = sessions
        .profile(&appliances, "area-02-scada-01")
        .expect("ready SCADA");
    for (tag, state) in [
        ("area-02-slip-01-command", "recirculating"),
        ("area-02-mould-01-command", "closed"),
        ("area-02-robot-01-command", "home"),
    ] {
        let actuator = ready
            .actuators
            .iter()
            .find(|actuator| actuator.command_tag == tag)
            .expect("forming idle actuator");
        assert_eq!(actuator.current_state, state, "{tag} initial state");
    }

    let started = sessions
        .execute(&appliances, "area-02-scada-01", HmiAction::StartProcess)
        .expect("start action");
    assert!(matches!(started.status, HmiActionStatus::Applied));
    sessions.tick(750);

    let supply = sessions
        .profile(&appliances, "area-02-hmi-01")
        .expect("supply HMI");
    assert_eq!(
        supply.process.as_ref().expect("process").phase,
        "mould-filling"
    );
    assert_eq!(
        supply
            .signals
            .iter()
            .find(|signal| signal.tag == "area-02-ft-01")
            .expect("slip flow")
            .value,
        85.0
    );

    let denied = sessions
        .execute(
            &appliances,
            "area-02-hmi-01",
            HmiAction::Command {
                tag: "area-02-slip-01-command".into(),
                value: "draining".into(),
            },
        )
        .expect("manual command");
    assert!(matches!(denied.status, HmiActionStatus::Denied));

    sessions.tick(13_300);
    for id in ["area-02-scada-01", "area-02-hmi-02", "area-02-hmi-04"] {
        let snapshot = sessions.profile(&appliances, id).expect("shared profile");
        let process = snapshot.process.expect("forming process");
        assert_eq!(process.phase, "idle", "{id} phase");
        assert_eq!(process.cycle_count, 1, "{id} completed cycles");
    }
}

#[test]
fn forming_scada_publishes_the_current_process_snapshot_to_analytics() {
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config");
    let appliances = ConfigRepository::load(project.join("appliances")).expect("appliances");
    let connections =
        ConnectionRepository::load(project.join("connections"), &appliances).expect("connections");
    let scenarios = ScenarioRepository::load(project.join("scenarios"), &appliances, &connections)
        .expect("scenarios");
    let scenario = &scenarios
        .get("factory-operations-data")
        .expect("operations-data scenario")
        .config;
    let mut sessions = HmiSessionStore::default();
    sessions
        .execute(&appliances, "area-02-scada-01", HmiAction::StartProcess)
        .expect("start Forming");
    sessions.tick(3_000);
    let snapshot = sessions
        .profile(&appliances, "area-02-scada-01")
        .expect("Forming SCADA");
    let packet = build_forming_telemetry_packet(&snapshot, scenario.packet.clone())
        .expect("telemetry packet");

    let ScenarioApplicationConfig::Telemetry {
        source,
        sequence,
        payload,
        ..
    } = &packet.application
    else {
        panic!("expected telemetry application");
    };
    assert_eq!(source, "area-02-vplc-01");
    let sequence = *sequence;
    assert_eq!(
        sequence,
        snapshot.process.as_ref().expect("process").scan_count
    );
    let payload: serde_json::Value = serde_json::from_str(payload).expect("telemetry JSON");
    assert_eq!(payload["cell"], "OT-AREA-02");
    assert_eq!(payload["phase"], "pressure-dwell");
    assert_eq!(payload["slip_c"], 40.0);
    assert_eq!(payload["mould_bar"], 6.0);

    let report = run_scenario_with_state_overrides(
        &appliances,
        &connections,
        scenario,
        Some(packet),
        None,
        None,
        None,
    )
    .expect("telemetry scenario");
    assert!(report.expectation_met, "{:#?}", report.trace);
    assert!(report.trace.iter().any(|entry| {
        entry.component == "operations-analytics-01"
            && entry.summary.contains("accepted telemetry")
            && entry.summary.contains(&format!("sequence {sequence}"))
    }));
    sessions
        .record_telemetry_publication(&appliances, "area-02-scada-01", report.expectation_met)
        .expect("publication audit");
    let audited = sessions
        .profile(&appliances, "area-02-scada-01")
        .expect("audited SCADA");
    let audit = audited.audit.last().expect("telemetry audit entry");
    assert_eq!(audit.action, "publish-telemetry");
    assert_eq!(audit.target, "operations-analytics-01");
    assert_eq!(audit.result, "delivered");
}

#[test]
fn forming_faults_propagate_and_require_authorized_clear_and_reset() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    sessions
        .execute(&appliances, "area-02-scada-01", HmiAction::StartProcess)
        .expect("start action");
    sessions.tick(11_780);

    let unauthorized = sessions
        .execute(
            &appliances,
            "area-02-hmi-03",
            HmiAction::SetProcessFault {
                fault: HmiProcessFault::VacuumLoss,
                active: true,
            },
        )
        .expect("unauthorized fault action");
    assert!(matches!(unauthorized.status, HmiActionStatus::Denied));

    sessions
        .execute(
            &appliances,
            "area-02-scada-01",
            HmiAction::SetProcessFault {
                fault: HmiProcessFault::VacuumLoss,
                active: true,
            },
        )
        .expect("inject vacuum fault");
    sessions.tick(800);
    let utility = sessions
        .profile(&appliances, "area-02-hmi-03")
        .expect("utility HMI");
    assert_eq!(utility.process.as_ref().expect("process").phase, "faulted");
    assert!(
        utility
            .alarms
            .iter()
            .any(|alarm| alarm.active && alarm.code == "FORMING-VACUUM-NOT-ESTABLISHED")
    );

    sessions
        .execute(
            &appliances,
            "area-02-scada-01",
            HmiAction::SetProcessFault {
                fault: HmiProcessFault::VacuumLoss,
                active: false,
            },
        )
        .expect("clear vacuum fault");
    let reset = sessions
        .execute(&appliances, "area-02-scada-01", HmiAction::ResetProcess)
        .expect("reset process");
    assert!(matches!(reset.status, HmiActionStatus::Applied));
    assert_eq!(
        reset.snapshot.process.as_ref().expect("process").phase,
        "idle"
    );
    assert!(reset.snapshot.alarms.iter().all(|alarm| !alarm.active));
}

#[test]
fn mould_overpressure_latches_the_machine_safety_state() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    sessions
        .execute(&appliances, "area-02-scada-01", HmiAction::StartProcess)
        .expect("start action");
    sessions.tick(1_500);
    sessions
        .execute(
            &appliances,
            "area-02-scada-01",
            HmiAction::SetProcessFault {
                fault: HmiProcessFault::MouldOverpressure,
                active: true,
            },
        )
        .expect("inject overpressure");
    sessions.tick(20);

    let tripped = sessions
        .profile(&appliances, "area-02-hmi-02")
        .expect("casting HMI");
    assert!(tripped.safety[0].trip_latched);
    assert_eq!(tripped.process.as_ref().expect("process").phase, "faulted");

    sessions
        .execute(
            &appliances,
            "area-02-scada-01",
            HmiAction::SetProcessFault {
                fault: HmiProcessFault::MouldOverpressure,
                active: false,
            },
        )
        .expect("clear overpressure");
    let reset = sessions
        .execute(
            &appliances,
            "area-02-hmi-02",
            HmiAction::ResetSafety {
                safety_id: "area-02-safe-01".into(),
            },
        )
        .expect("safety reset");
    assert!(matches!(reset.status, HmiActionStatus::Applied));
    assert_eq!(
        reset.snapshot.process.as_ref().expect("process").phase,
        "idle"
    );
}
