use super::*;

fn robot_program_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../project/control/programs/forming/area-02-robot-01.g");
    std::fs::read_to_string(path).expect("canonical robot program")
}

fn enter_robot_setup(sessions: &mut HmiSessionStore, appliances: &ConfigRepository) {
    sessions
        .execute(
            appliances,
            "area-02-joystick-01",
            HmiAction::SetControlMode {
                mode: HmiControlMode::Setup,
                password: Some("VanillaIceCream1!".into()),
            },
        )
        .expect("authenticated robot setup");
    sessions
        .execute(
            appliances,
            "area-02-joystick-01",
            HmiAction::SetRobotMotionEnable { enabled: true },
        )
        .expect("robot motion enable");
}

#[test]
fn robot_program_requires_all_handoff_routines_and_exposes_execution() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    let initial = sessions
        .profile(&appliances, "area-02-joystick-01")
        .expect("robot profile")
        .robot
        .expect("robot state");
    assert_eq!(initial.program.name, "O0201");
    assert!(initial.program.source.contains("O0204"));
    assert!(
        initial
            .program
            .lines
            .iter()
            .any(|line| line.operation.is_some())
    );
    assert_eq!(initial.taught_positions.len(), 17);
    assert_eq!(
        initial
            .handoffs
            .iter()
            .map(|handoff| handoff.program.as_str())
            .collect::<Vec<_>>(),
        ["O0201", "O0202", "O0203", "O0204"]
    );

    enter_robot_setup(&mut sessions, &appliances);
    let invalid = sessions
        .execute(
            &appliances,
            "area-02-joystick-01",
            HmiAction::LoadRobotProgram {
                name: "INCOMPLETE".into(),
                source: "%\nO0201\nN10 M64\nN20 M65\nN30 M30\n%\n".into(),
            },
        )
        .expect("incomplete source report");
    assert!(matches!(invalid.status, HmiActionStatus::Denied));

    let loaded = sessions
        .execute(
            &appliances,
            "area-02-joystick-01",
            HmiAction::LoadRobotProgram {
                name: "SESSION_TEST".into(),
                source: robot_program_source(),
            },
        )
        .expect("valid source report");
    assert!(matches!(loaded.status, HmiActionStatus::Applied));
    assert_eq!(
        loaded.snapshot.robot.as_ref().expect("robot").program.name,
        "SESSION_TEST"
    );

    let stepped = sessions
        .execute(
            &appliances,
            "area-02-joystick-01",
            HmiAction::StepRobotProgram,
        )
        .expect("program step");
    assert!(matches!(stepped.status, HmiActionStatus::Applied));
    sessions.tick(250);
    let executing = sessions
        .profile(&appliances, "area-02-joystick-01")
        .expect("executing profile")
        .robot
        .expect("robot state");
    assert!(executing.program.active_line.is_some());
    assert!(executing.program.lines.iter().any(|line| line.active));
}

#[test]
fn running_manual_program_stays_stopped_after_motion_enable_is_released() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    enter_robot_setup(&mut sessions, &appliances);
    sessions
        .execute(
            &appliances,
            "area-02-joystick-01",
            HmiAction::RunRobotProgram,
        )
        .expect("start manual robot program");
    sessions.tick(500);
    let moving = sessions
        .profile(&appliances, "area-02-joystick-01")
        .expect("moving robot profile")
        .robot
        .expect("robot state");
    assert!(moving.program.running);
    assert!(moving.motion.active);

    let released = sessions
        .execute(
            &appliances,
            "area-02-joystick-01",
            HmiAction::SetRobotMotionEnable { enabled: false },
        )
        .expect("release pendant motion enable");
    let held = released.snapshot.robot.expect("held robot state");
    assert!(!held.motion_enabled);
    assert!(!held.motion.active);
    assert!(!held.program.running);
    assert!(held.program.paused);
    let held_pose = held.pose;

    sessions.tick(5_000);
    let after = sessions
        .profile(&appliances, "area-02-joystick-01")
        .expect("robot profile after hold")
        .robot
        .expect("robot state");
    assert_eq!(after.pose, held_pose);
    assert!(!after.motion.active);
    assert!(after.program.paused);
}

#[test]
fn wrong_pickup_coordinates_fault_the_robot_and_raise_an_alarm() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    enter_robot_setup(&mut sessions, &appliances);
    let source = robot_program_source().replace("N40 G1 X-930 Y-480 Z980", "N40 G1 X-930 Y80 Z980");
    let loaded = sessions
        .execute(
            &appliances,
            "area-02-joystick-01",
            HmiAction::LoadRobotProgram {
                name: "BAD_MOULD_01_PICKUP".into(),
                source,
            },
        )
        .expect("load deliberately incorrect program");
    assert!(matches!(loaded.status, HmiActionStatus::Applied));
    sessions
        .execute(
            &appliances,
            "area-02-joystick-01",
            HmiAction::SetControlMode {
                mode: HmiControlMode::Auto,
                password: None,
            },
        )
        .expect("robot automatic mode");
    sessions
        .execute(&appliances, "area-02-hmi-01", HmiAction::StartMould)
        .expect("start Mould 1");

    let mut faulted = None;
    for _ in 0..500 {
        sessions.tick(500);
        let snapshot = sessions
            .profile(&appliances, "area-02-joystick-01")
            .expect("robot profile");
        if snapshot
            .robot
            .as_ref()
            .is_some_and(|robot| robot.cell.fault_code.is_some())
        {
            faulted = Some(snapshot);
            break;
        }
    }
    let snapshot = faulted.expect("robot coordinate fault");
    let robot = snapshot.robot.expect("robot state");
    assert_eq!(robot.controller_state, "faulted");
    assert_eq!(
        robot.cell.fault_code.as_deref(),
        Some("ROBOT-PICKUP-POSITION-MISMATCH")
    );
    assert!(
        robot
            .cell
            .fault_message
            .as_deref()
            .is_some_and(|message| { message.contains("O0201") && message.contains("mould-01") })
    );
    assert!(
        snapshot
            .alarms
            .iter()
            .any(|alarm| { alarm.active && alarm.code == "ROBOT-PICKUP-POSITION-MISMATCH" })
    );
}
