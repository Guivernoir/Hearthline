use super::*;

#[test]
fn forming_faults_trip_only_the_running_mould_and_require_clear_and_reset() {
    let appliances = repository();
    let mut sessions = PlantRuntimeStore::default();
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
    let mut sessions = PlantRuntimeStore::default();
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
