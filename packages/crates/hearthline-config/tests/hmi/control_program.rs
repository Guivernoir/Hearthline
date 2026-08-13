use std::path::PathBuf;

use hearthline_config::{ConfigRepository, HmiAction, HmiSession, HmiSessionStore};

fn repository() -> ConfigRepository {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config/appliances");
    ConfigRepository::load(root).expect("appliances")
}

#[test]
fn forming_control_source_is_loaded_and_exposed_by_the_hmi() {
    let appliances = repository();
    let session =
        HmiSession::from_repository(&appliances, "area-02-machine-pc-01").expect("Forming SCADA");
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
fn structured_text_timer_transition_occurs_on_the_plc_scan_boundary() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
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
