use super::*;

fn reset_safety(sessions: &mut HmiSessionStore, appliances: &ConfigRepository) {
    reset_local_safety(sessions, appliances, "area-01-hmi-01", "area-01-intlk-01");
}

fn reset_local_safety(
    sessions: &mut HmiSessionStore,
    appliances: &ConfigRepository,
    hmi: &str,
    safety: &str,
) {
    let reset = sessions
        .execute(
            appliances,
            hmi,
            HmiAction::ResetSafety {
                safety_id: safety.into(),
            },
        )
        .expect("reset Body Preparation safety");
    assert!(matches!(reset.status, HmiActionStatus::Applied));
}

#[test]
fn body_preparation_hmi_runs_holds_and_completes_a_batch() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    reset_safety(&mut sessions, &appliances);

    let started = sessions
        .execute(&appliances, "area-01-hmi-01", HmiAction::StartProcess)
        .expect("start Body Preparation");
    assert!(matches!(started.status, HmiActionStatus::Applied));
    assert_eq!(
        started
            .snapshot
            .body_preparation
            .as_ref()
            .expect("batch state")
            .slip
            .train
            .phase,
        "water-charge"
    );

    sessions.tick(200);
    let charging = sessions
        .profile(&appliances, "area-01-hmi-01")
        .expect("charging state");
    assert!(
        charging
            .signals
            .iter()
            .find(|signal| signal.tag == "area-01-ft-02")
            .expect("water flow")
            .value
            > 0.0
    );
    let held = sessions
        .execute(&appliances, "area-01-hmi-01", HmiAction::HoldProcess)
        .expect("hold batch");
    assert!(
        held.snapshot
            .body_preparation
            .expect("batch")
            .slip
            .train
            .held
    );
    assert!(
        held.snapshot
            .actuators
            .iter()
            .all(|actuator| { actuator.current_state == actuator.safe_state })
    );

    sessions
        .execute(&appliances, "area-01-hmi-01", HmiAction::StartProcess)
        .expect("resume batch");
    for _ in 0..100 {
        sessions.tick(500);
        let state = sessions
            .profile(&appliances, "area-01-hmi-01")
            .expect("batch state");
        if state
            .body_preparation
            .as_ref()
            .expect("batch")
            .slip
            .train
            .cycle_count
            == 1
        {
            break;
        }
    }
    let complete = sessions
        .profile(&appliances, "area-01-hmi-01")
        .expect("completed batch");
    let batch = complete.body_preparation.expect("batch");
    assert_eq!(batch.slip.train.cycle_count, 1);
    assert!((batch.slip.batch_mass_kg - 1_335.3).abs() < 0.1);
    assert!((batch.slip.solids_percent - 74.9).abs() < 0.2);
}

#[test]
fn body_preparation_qc_fault_requires_clear_and_reset() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    reset_safety(&mut sessions, &appliances);
    sessions
        .execute(&appliances, "area-01-hmi-01", HmiAction::StartProcess)
        .expect("start batch");
    sessions
        .execute(
            &appliances,
            "area-01-hmi-01",
            HmiAction::SetProcessFault {
                fault: HmiProcessFault::QualityOutOfSpec,
                active: true,
            },
        )
        .expect("inject quality failure");

    for _ in 0..100 {
        sessions.tick(500);
        let state = sessions
            .profile(&appliances, "area-01-hmi-01")
            .expect("batch state");
        if state
            .process
            .as_ref()
            .is_some_and(|process| process.phase == "faulted")
        {
            break;
        }
    }
    let faulted = sessions
        .profile(&appliances, "area-01-hmi-01")
        .expect("faulted batch");
    assert_eq!(faulted.process.expect("process").phase, "faulted");
    assert!(
        faulted
            .alarms
            .iter()
            .any(|alarm| alarm.code == "BODY-SLIP-QUALITY-RELEASE-DENIED" && alarm.active)
    );

    sessions
        .execute(
            &appliances,
            "area-01-hmi-01",
            HmiAction::SetProcessFault {
                fault: HmiProcessFault::QualityOutOfSpec,
                active: false,
            },
        )
        .expect("clear quality failure");
    let reset = sessions
        .execute(&appliances, "area-01-hmi-01", HmiAction::ResetProcess)
        .expect("reset process");
    assert!(matches!(reset.status, HmiActionStatus::Applied));
    assert_eq!(reset.snapshot.process.expect("process").phase, "idle");
}

#[test]
fn body_preparation_utility_trains_have_independent_hmi_control() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    for (hmi, safety, train, phase) in [
        (
            "area-01-wt-hmi-01",
            "area-01-wt-intlk-01",
            HmiPreparationTrain::Water,
            "raw-water-intake",
        ),
        (
            "area-01-rw-hmi-01",
            "area-01-rw-intlk-01",
            HmiPreparationTrain::ReturnWater,
            "segregated-return-collection",
        ),
        (
            "area-01-gl-hmi-01",
            "area-01-gl-intlk-01",
            HmiPreparationTrain::Glaze,
            "glaze-water-charge",
        ),
    ] {
        reset_local_safety(&mut sessions, &appliances, hmi, safety);
        let started = sessions
            .execute(&appliances, hmi, HmiAction::StartPreparationTrain { train })
            .expect("start preparation train");
        assert!(matches!(started.status, HmiActionStatus::Applied));

        let preparation = started
            .snapshot
            .body_preparation
            .expect("preparation state");
        let actual = match train {
            HmiPreparationTrain::Water => preparation.water.train.phase,
            HmiPreparationTrain::ReturnWater => preparation.return_water.train.phase,
            HmiPreparationTrain::Glaze => preparation.glaze.train.phase,
            HmiPreparationTrain::Slip => unreachable!(),
        };
        assert_eq!(actual, phase);

        let held = sessions
            .execute(&appliances, hmi, HmiAction::HoldPreparationTrain { train })
            .expect("hold preparation train");
        assert!(matches!(held.status, HmiActionStatus::Applied));
    }

    let denied = sessions
        .execute(
            &appliances,
            "area-01-hmi-01",
            HmiAction::StartPreparationTrain {
                train: HmiPreparationTrain::Water,
            },
        )
        .expect("reject remote train start");
    assert!(matches!(denied.status, HmiActionStatus::Denied));
}

#[test]
fn water_to_slip_handoff_is_shared_between_its_local_hmis() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    reset_local_safety(
        &mut sessions,
        &appliances,
        "area-01-hmi-01",
        "area-01-intlk-01",
    );
    reset_local_safety(
        &mut sessions,
        &appliances,
        "area-01-wd-hmi-01",
        "area-01-wd-intlk-01",
    );

    let denied = sessions
        .execute(
            &appliances,
            "area-01-hmi-01",
            HmiAction::SetProcessFault {
                fault: HmiProcessFault::WaterToSlipLeak,
                active: true,
            },
        )
        .expect("reject receiver-side fault injection");
    assert!(matches!(denied.status, HmiActionStatus::Denied));

    sessions
        .execute(
            &appliances,
            "area-01-wd-hmi-01",
            HmiAction::SetProcessFault {
                fault: HmiProcessFault::WaterToSlipLeak,
                active: true,
            },
        )
        .expect("inject the water-cell branch leak");
    let started = sessions
        .execute(&appliances, "area-01-hmi-01", HmiAction::StartProcess)
        .expect("start slip water charge");
    assert!(matches!(started.status, HmiActionStatus::Applied));
    sessions.tick(200);

    for hmi in ["area-01-hmi-01", "area-01-wd-hmi-01"] {
        let snapshot = sessions.profile(&appliances, hmi).expect("handoff state");
        let line = snapshot
            .body_preparation
            .expect("Body Preparation state")
            .pipelines
            .water_to_slip;
        assert!(line.leak_detected);
        assert!(line.outlet_flow_l_min < line.inlet_flow_l_min);
        assert!(
            snapshot
                .alarms
                .iter()
                .any(|alarm| { alarm.code == "BODY-WATER-SLIP-BRANCH-LEAK" && alarm.active })
        );
    }
}

#[test]
fn water_pipeline_hmi_reports_failover_and_dispatches_pump_maintenance() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    reset_local_safety(
        &mut sessions,
        &appliances,
        "area-01-wd-hmi-01",
        "area-01-wd-intlk-01",
    );

    let denied = sessions
        .execute(
            &appliances,
            "area-01-wt-hmi-01",
            HmiAction::SetWaterPumpFailure {
                pump_id: "area-01-wd-pmp-01a".into(),
                failed: true,
            },
        )
        .expect("treatment HMI rejects pipeline fault injection");
    assert!(matches!(denied.status, HmiActionStatus::Denied));

    sessions
        .execute(
            &appliances,
            "area-01-wd-hmi-01",
            HmiAction::SetWaterPumpFailure {
                pump_id: "area-01-wd-pmp-01a".into(),
                failed: true,
            },
        )
        .expect("inject duty-pump heartbeat loss");
    sessions.tick(PUMP_HEARTBEAT_TIMEOUT_MS);

    let failed = sessions
        .profile(&appliances, "area-01-wd-hmi-01")
        .expect("pipeline state after failover");
    let water = failed
        .body_preparation
        .expect("water networks")
        .water_networks;
    let duty = water
        .pumps
        .iter()
        .find(|pump| pump.id == "area-01-wd-pmp-01a")
        .expect("preferred duty pump");
    let standby = water
        .pumps
        .iter()
        .find(|pump| pump.id == "area-01-wd-pmp-01b")
        .expect("standby pump");
    assert!(!duty.heartbeat_ok);
    assert_eq!(duty.maintenance, "required");
    assert!(standby.running_feedback);
    assert!(failed.alarms.iter().any(|alarm| {
        alarm.active && alarm.code == "BODY-WATER-PUMP-HEARTBEAT-LOST" && alarm.source == duty.id
    }));

    let dispatched = sessions
        .execute(
            &appliances,
            "area-01-wd-hmi-01",
            HmiAction::DispatchWaterPumpMaintenance {
                pump_id: duty.id.into(),
            },
        )
        .expect("dispatch maintenance");
    assert!(matches!(dispatched.status, HmiActionStatus::Applied));
    assert_eq!(
        dispatched
            .snapshot
            .body_preparation
            .expect("water networks")
            .water_networks
            .pumps
            .iter()
            .find(|pump| pump.id == duty.id)
            .expect("dispatched pump")
            .maintenance,
        "dispatched"
    );
}
