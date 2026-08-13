use super::*;
fn mould<'a>(
    snapshot: &'a hearthline_config::HmiSnapshot,
    target: &str,
) -> &'a hearthline_config::HmiMouldProcessState {
    snapshot
        .moulds
        .iter()
        .find(|mould| mould.target == target)
        .expect("configured mould state")
}

#[test]
fn forming_scada_and_module_hmis_expose_their_configured_scopes() {
    let appliances = repository();
    let scada = HmiSession::from_repository(&appliances, "area-02-machine-pc-01")
        .expect("forming SCADA")
        .snapshot();

    assert_eq!(scada.interface_kind, "scada-workstation");
    assert_eq!(scada.signals.len(), 45);
    assert_eq!(scada.actuators.len(), 12);
    assert_eq!(scada.safety.len(), 6);
    assert_eq!(scada.remote_io_stations.len(), 6);
    assert_eq!(scada.parameters.len(), 28);
    assert_eq!(scada.station_status.len(), 6);
    assert_eq!(scada.moulds.len(), 4);
    assert_eq!(scada.remote_io, "area-02-rio-01");
    assert!(scada.safety.iter().all(|safety| !safety.trip_latched));
    let supervisory = scada.supervisory.as_ref().expect("supervisory model");
    assert_eq!(supervisory.assets.len(), 7);
    assert_eq!(supervisory.deployment_nodes.len(), 5);
    assert!(supervisory.repository.synchronized);
    assert_eq!(supervisory.identity.role, "process-engineer");
    for mould in &scada.moulds {
        assert!(mould.setpoints_bound, "{} setpoint binding", mould.target);
        assert!(
            mould.control_cabinet.is_some(),
            "{} control cabinet",
            mould.target
        );
        assert!(
            mould.utility_cabinet.is_some(),
            "{} mould-embedded utility section",
            mould.target
        );
        assert_eq!(
            mould
                .utility_cabinet
                .as_ref()
                .expect("mould-embedded utility section")
                .circuits
                .len(),
            5
        );
    }
    for (id, signal_count, actuator_count, remote_io) in [
        ("area-02-hmi-01", 6, 1, "area-02-m01-rio-01"),
        ("area-02-hmi-02", 6, 1, "area-02-m02-rio-01"),
        ("area-02-hmi-03", 6, 1, "area-02-m03-rio-01"),
        ("area-02-hmi-04", 6, 1, "area-02-m04-rio-01"),
        ("area-02-joystick-01", 2, 1, "area-02-rio-01"),
    ] {
        let module = HmiSession::from_repository(&appliances, id)
            .expect("forming module HMI")
            .snapshot();
        assert_eq!(module.interface_kind, "hmi", "{id} kind");
        assert_eq!(module.signals.len(), signal_count, "{id} signals");
        assert_eq!(module.actuators.len(), actuator_count, "{id} actuators");
        assert_eq!(module.controller, "area-02-vplc-01", "{id} controller");
        assert_eq!(module.remote_io, remote_io, "{id} remote I/O");
    }
}

#[test]
fn robot_controller_arbitrates_simultaneous_mould_requests() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    sessions
        .execute(&appliances, "area-02-hmi-01", HmiAction::StartMould)
        .expect("start Mould 1");
    sessions
        .execute(&appliances, "area-02-hmi-02", HmiAction::StartMould)
        .expect("start Mould 2");
    sessions.tick(9_000);
    sessions.tick(20);

    let pendant = sessions
        .profile(&appliances, "area-02-joystick-01")
        .expect("robot pendant");
    let robot = pendant.robot.expect("robot controller state");
    assert_eq!(robot.architecture.servo_axes, 6);
    assert_eq!(robot.handoffs.len(), 4);
    assert!(robot.cell.active_mould.is_some());
    assert!(robot.cell.queued_moulds.len() <= 1);
    assert_ne!(robot.cell.stage, "idle");
}

#[test]
fn supervisory_history_samples_live_tags_with_quality_and_time() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    sessions
        .profile(&appliances, "area-02-machine-pc-01")
        .expect("initialize machine PC");
    sessions.tick(2_000);
    let pc = sessions
        .profile(&appliances, "area-02-machine-pc-01")
        .expect("machine PC");
    let supervisory = pc.supervisory.expect("supervisory state");
    let level = supervisory
        .tags
        .iter()
        .find(|tag| tag.tag == "area-02-lt-01")
        .expect("level history tag");
    assert_eq!(level.quality, "good");
    assert_eq!(level.samples.len(), 1);
    assert_eq!(level.samples[0].timestamp_ms, 2_000);
}

#[test]
fn local_start_repeats_cycles_without_starting_other_moulds() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    let started = sessions
        .execute(&appliances, "area-02-hmi-01", HmiAction::StartMould)
        .expect("start Mould 1");
    assert!(matches!(started.status, HmiActionStatus::Applied));

    sessions.tick(750);
    let filling = sessions
        .profile(&appliances, "area-02-machine-pc-01")
        .expect("machine PC");
    assert_eq!(mould(&filling, "mould-01").phase, "mould-filling");
    assert_eq!(mould(&filling, "mould-02").phase, "idle");
    assert!(mould(&filling, "mould-01").production_enabled);
    assert!(!mould(&filling, "mould-02").production_enabled);
    assert_eq!(
        filling
            .signals
            .iter()
            .find(|signal| signal.tag == "area-02-ft-01")
            .expect("slip flow")
            .value,
        85.0
    );
    assert!(
        filling
            .signals
            .iter()
            .find(|signal| signal.tag == "area-02-pos-01")
            .expect("Mould 1 fill head")
            .value
            > 0.0
    );
    assert_eq!(
        filling
            .signals
            .iter()
            .find(|signal| signal.tag == "area-02-m02-pos-01")
            .expect("Mould 2 fill head")
            .value,
        0.0
    );

    for _ in 0..400 {
        sessions.tick(500);
        let state = sessions
            .profile(&appliances, "area-02-machine-pc-01")
            .expect("repeating process");
        if mould(&state, "mould-01").cycle_count >= 2 {
            break;
        }
    }
    let repeated = sessions
        .profile(&appliances, "area-02-machine-pc-01")
        .expect("repeating process");
    let mould_one = mould(&repeated, "mould-01");
    assert!(mould_one.running);
    assert!(mould_one.production_enabled);
    assert!(mould_one.cycle_count >= 2);
    assert_eq!(mould(&repeated, "mould-02").cycle_count, 0);

    let local_two = sessions
        .profile(&appliances, "area-02-hmi-02")
        .expect("Mould 2 local HMI");
    assert_eq!(local_two.process.expect("local process").phase, "idle");
}

#[test]
fn moulds_can_run_at_independent_phase_offsets() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    sessions
        .execute(&appliances, "area-02-hmi-01", HmiAction::StartMould)
        .expect("start Mould 1");
    sessions.tick(750);
    sessions
        .execute(&appliances, "area-02-hmi-02", HmiAction::StartMould)
        .expect("start Mould 2");
    sessions.tick(750);

    let pc = sessions
        .profile(&appliances, "area-02-machine-pc-01")
        .expect("machine PC");
    assert_eq!(mould(&pc, "mould-01").phase, "air-pressurizing");
    assert_eq!(mould(&pc, "mould-02").phase, "mould-filling");
    assert_eq!(mould(&pc, "mould-03").phase, "idle");
    assert_eq!(pc.process.expect("aggregate process").phase, "mixed");
}

#[test]
fn stop_pauses_after_the_phase_and_end_finishes_the_cycle() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    sessions
        .execute(&appliances, "area-02-hmi-01", HmiAction::StartMould)
        .expect("start Mould 1");
    sessions.tick(500);
    let requested = sessions
        .execute(
            &appliances,
            "area-02-hmi-01",
            HmiAction::StopMouldAfterPhase,
        )
        .expect("request phase stop");
    assert_eq!(
        mould(&requested.snapshot, "mould-01").stop_request,
        Some("after-phase")
    );

    sessions.tick(900);
    let before_boundary = sessions
        .profile(&appliances, "area-02-hmi-01")
        .expect("before phase boundary");
    assert!(mould(&before_boundary, "mould-01").running);
    sessions.tick(200);
    let paused = sessions
        .profile(&appliances, "area-02-hmi-01")
        .expect("phase-boundary pause");
    let paused_mould = mould(&paused, "mould-01");
    assert_eq!(paused_mould.phase, "air-pressurizing");
    assert!(paused_mould.paused);
    assert!(!paused_mould.running);
    let paused_scan = paused_mould.scan_count;

    sessions.tick(1_000);
    let held = sessions
        .profile(&appliances, "area-02-hmi-01")
        .expect("held phase boundary");
    assert_eq!(mould(&held, "mould-01").scan_count, paused_scan);

    let resumed = sessions
        .execute(&appliances, "area-02-hmi-01", HmiAction::StartMould)
        .expect("resume Mould 1");
    assert!(mould(&resumed.snapshot, "mould-01").running);
    assert!(!mould(&resumed.snapshot, "mould-01").paused);
    let ending = sessions
        .execute(&appliances, "area-02-hmi-01", HmiAction::EndMouldAfterCycle)
        .expect("request cycle end");
    assert_eq!(
        mould(&ending.snapshot, "mould-01").stop_request,
        Some("after-cycle")
    );

    for _ in 0..100 {
        sessions.tick(500);
        let state = sessions
            .profile(&appliances, "area-02-hmi-01")
            .expect("cycle progression");
        if !mould(&state, "mould-01").running {
            break;
        }
    }
    let stopped = sessions
        .profile(&appliances, "area-02-hmi-01")
        .expect("cycle-boundary stop");
    let stopped_mould = mould(&stopped, "mould-01");
    assert_eq!(stopped_mould.phase, "idle");
    assert_eq!(stopped_mould.cycle_count, 1);
    assert!(!stopped_mould.running);
    assert!(!stopped_mould.production_enabled);
    assert_eq!(stopped_mould.stop_request, None);
}

#[test]
fn forming_scada_publishes_identified_mould_telemetry_to_analytics() {
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
        .execute(&appliances, "area-02-hmi-01", HmiAction::StartMould)
        .expect("start Mould 1");
    sessions.tick(3_000);
    let snapshot = sessions
        .profile(&appliances, "area-02-machine-pc-01")
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
    let mould_one = mould(&snapshot, "mould-01");
    assert_eq!(*sequence, mould_one.scan_count);
    let sequence = *sequence;
    let payload: serde_json::Value = serde_json::from_str(payload).expect("telemetry JSON");
    assert_eq!(payload["cell"], "OT-AREA-02");
    assert_eq!(payload["mould"], "mould-01");
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
        .record_telemetry_publication(&appliances, "area-02-machine-pc-01", report.expectation_met)
        .expect("publication audit");
    let audited = sessions
        .profile(&appliances, "area-02-machine-pc-01")
        .expect("audited SCADA");
    let audit = audited.audit.last().expect("telemetry audit entry");
    assert_eq!(audit.action, "publish-telemetry");
    assert_eq!(audit.target, "operations-analytics-01");
    assert_eq!(audit.result, "delivered");
}

#[test]
fn forming_faults_trip_only_the_running_mould_and_require_clear_and_reset() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    sessions
        .execute(&appliances, "area-02-hmi-01", HmiAction::StartMould)
        .expect("start Mould 1");
    for _ in 0..40 {
        let state = sessions
            .profile(&appliances, "area-02-machine-pc-01")
            .expect("machine PC");
        if mould(&state, "mould-01").phase == "vacuum-dry" {
            break;
        }
        sessions.tick(1_000);
    }
    let vacuum = sessions
        .profile(&appliances, "area-02-machine-pc-01")
        .expect("vacuum phase");
    assert_eq!(mould(&vacuum, "mould-01").phase, "vacuum-dry");

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
            "area-02-machine-pc-01",
            HmiAction::SetProcessFault {
                fault: HmiProcessFault::VacuumLoss,
                active: true,
            },
        )
        .expect("inject vacuum fault");
    sessions.tick(800);
    let faulted = sessions
        .profile(&appliances, "area-02-machine-pc-01")
        .expect("machine PC");
    assert_eq!(mould(&faulted, "mould-01").phase, "faulted");
    assert_eq!(mould(&faulted, "mould-02").phase, "idle");
    assert!(
        faulted
            .alarms
            .iter()
            .any(|alarm| alarm.active && alarm.code == "FORMING-VACUUM-NOT-ESTABLISHED")
    );

    sessions
        .execute(
            &appliances,
            "area-02-machine-pc-01",
            HmiAction::SetProcessFault {
                fault: HmiProcessFault::VacuumLoss,
                active: false,
            },
        )
        .expect("clear vacuum fault");
    let reset = sessions
        .execute(
            &appliances,
            "area-02-machine-pc-01",
            HmiAction::ResetProcess,
        )
        .expect("reset process");
    assert!(matches!(reset.status, HmiActionStatus::Applied));
    assert_eq!(mould(&reset.snapshot, "mould-01").phase, "idle");
    assert!(reset.snapshot.alarms.iter().all(|alarm| !alarm.active));
}

#[test]
fn mould_overpressure_latches_only_the_owning_mould_safety() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    sessions
        .execute(&appliances, "area-02-hmi-01", HmiAction::StartMould)
        .expect("start Mould 1");
    sessions.tick(1_500);
    sessions
        .execute(
            &appliances,
            "area-02-machine-pc-01",
            HmiAction::SetProcessFault {
                fault: HmiProcessFault::MouldOverpressure,
                active: true,
            },
        )
        .expect("inject overpressure");
    sessions.tick(20);

    let tripped = sessions
        .profile(&appliances, "area-02-machine-pc-01")
        .expect("machine PC");
    assert!(
        tripped
            .safety
            .iter()
            .find(|safety| safety.component_id == "area-02-safe-01")
            .expect("Mould 1 safety")
            .trip_latched
    );
    assert_eq!(
        tripped
            .safety
            .iter()
            .filter(|safety| safety.trip_latched)
            .count(),
        1
    );
    assert_eq!(mould(&tripped, "mould-01").phase, "faulted");
    assert_eq!(mould(&tripped, "mould-02").phase, "idle");

    sessions
        .execute(
            &appliances,
            "area-02-machine-pc-01",
            HmiAction::SetProcessFault {
                fault: HmiProcessFault::MouldOverpressure,
                active: false,
            },
        )
        .expect("clear overpressure");
    let reset = sessions
        .execute(
            &appliances,
            "area-02-machine-pc-01",
            HmiAction::ResetSafety {
                safety_id: "area-02-safe-01".into(),
            },
        )
        .expect("Mould 1 safety reset");
    assert!(matches!(reset.status, HmiActionStatus::Applied));
    assert_eq!(mould(&reset.snapshot, "mould-01").phase, "idle");
}
