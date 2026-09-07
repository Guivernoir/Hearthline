use std::path::PathBuf;

use hearthline_config::{ConfigRepository, HmiAction};
use hearthline_sim::{PlantControlRuntime, PlantRuntimeStore};

fn repository() -> ConfigRepository {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config/appliances");
    ConfigRepository::load(root).expect("appliances")
}

#[test]
fn forming_control_source_is_loaded_and_exposed_by_the_hmi() {
    let appliances = repository();
    let session = PlantControlRuntime::from_repository(&appliances, "area-02-machine-pc-01")
        .expect("Forming SCADA");
    let snapshot = session.snapshot();
    let state = snapshot.control_program.expect("control program state");
    assert_eq!(state.language, "structured-text");
    assert_eq!(state.program, "FormingSequence");
    assert_eq!(state.task, "FormingSequenceTask");
    assert_eq!(state.current_step, 0);
    assert_eq!(state.scan_interval_ms, 20);
    assert_eq!(state.watchdog_ms, 100);
    assert!(state.source_path.ends_with("area-02-vplc-01.st"));
    assert!(state.binding_path.ends_with("area-02-vplc-01.yaml"));
    assert_eq!(state.revision.len(), 64);

    let document = session.control_program().expect("control source document");
    assert!(document.source.contains("SlipTemperature : REAL;"));
    for phase in [60, 70, 110, 120, 130] {
        assert!(document.source.contains(&format!("PhaseCode := {phase};")));
    }
    assert!(document.binding_yaml.contains("area-02-tt-01"));
    assert!(document.binding_yaml.contains("release-wet"));
    assert!(document.binding_yaml.contains("mould-wash"));
}

#[test]
fn body_preparation_control_source_is_the_live_slip_sequence_authority() {
    let appliances = repository();
    let mut sessions = PlantRuntimeStore::default();
    sessions
        .execute(
            &appliances,
            "area-01-hmi-01",
            HmiAction::ResetSafety {
                safety_id: "area-01-intlk-01".into(),
            },
        )
        .expect("reset slip safety");

    let idle = sessions
        .profile(&appliances, "area-01-hmi-01")
        .expect("idle slip state");
    let program = idle.control_program.expect("Body Preparation program");
    assert_eq!(program.program, "BodyPreparationBatch");
    assert_eq!(program.current_step, 0);
    assert!(program.source_path.ends_with("area-01-vplc-01.st"));

    let started = sessions
        .execute(&appliances, "area-01-hmi-01", HmiAction::StartProcess)
        .expect("start controlled batch");
    assert_eq!(
        started
            .snapshot
            .control_program
            .expect("live program state")
            .current_step,
        10
    );
    assert_eq!(
        started.snapshot.process.expect("slip process").phase,
        "water-charge"
    );

    sessions.tick(590);
    let before_scan = sessions
        .profile(&appliances, "area-01-hmi-01")
        .expect("pre-transition state");
    assert_eq!(
        before_scan
            .control_program
            .expect("program state")
            .current_step,
        10
    );
    assert_eq!(before_scan.process.expect("process").phase, "water-charge");

    sessions.tick(10);
    let after_scan = sessions
        .profile(&appliances, "area-01-hmi-01")
        .expect("post-transition state");
    assert_eq!(
        after_scan
            .control_program
            .expect("program state")
            .current_step,
        20
    );
    assert_eq!(
        after_scan.process.expect("process").phase,
        "deflocculant-charge"
    );
}

#[test]
fn body_preparation_hold_freezes_both_iec_step_and_physical_phase() {
    let appliances = repository();
    let mut sessions = PlantRuntimeStore::default();
    sessions
        .execute(
            &appliances,
            "area-01-hmi-01",
            HmiAction::ResetSafety {
                safety_id: "area-01-intlk-01".into(),
            },
        )
        .expect("reset slip safety");
    sessions
        .execute(&appliances, "area-01-hmi-01", HmiAction::StartProcess)
        .expect("start controlled batch");
    sessions.tick(200);
    sessions
        .execute(&appliances, "area-01-hmi-01", HmiAction::HoldProcess)
        .expect("hold controlled batch");
    let held = sessions
        .profile(&appliances, "area-01-hmi-01")
        .expect("held state");
    let held_step = held.control_program.expect("program state").current_step;
    let held_phase = held.process.expect("process state").phase;

    sessions.tick(5_000);
    let still_held = sessions
        .profile(&appliances, "area-01-hmi-01")
        .expect("stable held state");
    assert_eq!(
        still_held
            .control_program
            .expect("program state")
            .current_step,
        held_step
    );
    assert_eq!(still_held.process.expect("process state").phase, held_phase);
}

#[test]
fn structured_text_timer_transition_occurs_on_the_plc_scan_boundary() {
    let appliances = repository();
    let mut sessions = PlantRuntimeStore::default();
    sessions
        .execute(&appliances, "area-02-hmi-01", HmiAction::StartMould)
        .expect("start Mould 1");
    sessions.tick(1_500);
    let pressurizing = sessions
        .profile(&appliances, "area-02-hmi-01")
        .expect("pressurizing snapshot");
    assert_eq!(
        pressurizing.process.expect("process").phase,
        "air-pressurizing"
    );

    sessions.tick(750);
    let before_scan = sessions
        .profile(&appliances, "area-02-hmi-01")
        .expect("pre-scan snapshot");
    assert_eq!(
        before_scan.process.expect("process").phase,
        "air-pressurizing"
    );

    sessions.tick(10);
    let after_scan = sessions
        .profile(&appliances, "area-02-hmi-01")
        .expect("post-scan snapshot");
    assert_eq!(after_scan.process.expect("process").phase, "pressure-dwell");
    let controller = sessions
        .profile(&appliances, "area-02-machine-pc-01")
        .expect("controller source state");
    assert_eq!(
        controller
            .control_program
            .expect("control program")
            .current_step,
        30
    );
}
