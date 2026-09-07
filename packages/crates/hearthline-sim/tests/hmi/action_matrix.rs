use super::*;

const JOYSTICK: &str = "area-02-joystick-01";
const MACHINE_PC: &str = "area-02-machine-pc-01";
const MOULD_HMI: &str = "area-02-hmi-01";
const SETUP_PASSWORD: &str = "VanillaIceCream1!";

fn set_mode(
    sessions: &mut PlantRuntimeStore,
    appliances: &ConfigRepository,
    mode: HmiControlMode,
) -> hearthline_config::HmiActionReport {
    sessions
        .execute(
            appliances,
            JOYSTICK,
            HmiAction::SetControlMode {
                mode,
                password: (matches!(mode, HmiControlMode::Setup))
                    .then(|| SETUP_PASSWORD.to_owned()),
            },
        )
        .expect("robot selector action")
}

fn enable(sessions: &mut PlantRuntimeStore, appliances: &ConfigRepository) {
    let report = sessions
        .execute(
            appliances,
            JOYSTICK,
            HmiAction::SetRobotMotionEnable { enabled: true },
        )
        .expect("robot motion enable report");
    assert!(matches!(report.status, HmiActionStatus::Applied));
}

fn canonical_robot_program() -> String {
    std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../project/control/programs/forming/area-02-robot-01.g"),
    )
    .expect("canonical robot program")
}

#[test]
fn robot_actions_reject_wrong_station_mode_enable_and_input_boundaries() {
    let appliances = repository();
    let mut sessions = PlantRuntimeStore::default();
    let pose = HmiRobotPose {
        x: -620.0,
        y: 180.0,
        z: 1120.0,
        w: 0.0,
        p: 90.0,
        r: 0.0,
    };

    for (id, enabled) in [(MACHINE_PC, true), (MACHINE_PC, false), (MOULD_HMI, true)] {
        let report = sessions
            .execute(&appliances, id, HmiAction::SetRobotMotionEnable { enabled })
            .expect("unauthorized pendant report");
        assert!(matches!(report.status, HmiActionStatus::Denied));
    }
    let auto_enable = sessions
        .execute(
            &appliances,
            JOYSTICK,
            HmiAction::SetRobotMotionEnable { enabled: true },
        )
        .expect("automatic-mode enable report");
    assert!(matches!(auto_enable.status, HmiActionStatus::Denied));

    assert!(matches!(
        set_mode(&mut sessions, &appliances, HmiControlMode::Manual).status,
        HmiActionStatus::Applied
    ));
    enable(&mut sessions, &appliances);
    for speed_percent in [0.0, 100.1, f64::NAN] {
        let denied = sessions
            .execute(
                &appliances,
                JOYSTICK,
                HmiAction::MoveRobot {
                    target: pose,
                    speed_percent,
                },
            )
            .expect("invalid speed report");
        assert!(matches!(denied.status, HmiActionStatus::Denied));
    }
    let invalid_pose = sessions
        .execute(
            &appliances,
            JOYSTICK,
            HmiAction::MoveRobot {
                target: HmiRobotPose {
                    x: f64::INFINITY,
                    ..pose
                },
                speed_percent: 20.0,
            },
        )
        .expect("invalid pose report");
    assert!(matches!(invalid_pose.status, HmiActionStatus::Denied));

    let moving = sessions
        .execute(
            &appliances,
            JOYSTICK,
            HmiAction::MoveRobot {
                target: pose,
                speed_percent: 20.0,
            },
        )
        .expect("valid motion report");
    assert!(matches!(moving.status, HmiActionStatus::Applied));
    let already_active = sessions
        .execute(
            &appliances,
            JOYSTICK,
            HmiAction::MoveRobot {
                target: HmiRobotPose { z: 1000.0, ..pose },
                speed_percent: 20.0,
            },
        )
        .expect("active motion report");
    assert!(matches!(already_active.status, HmiActionStatus::Denied));
    assert!(already_active.message.contains("already active"));

    let released = sessions
        .execute(
            &appliances,
            JOYSTICK,
            HmiAction::SetRobotMotionEnable { enabled: false },
        )
        .expect("release motion enable");
    assert!(matches!(released.status, HmiActionStatus::Applied));
    let no_enable = sessions
        .execute(
            &appliances,
            JOYSTICK,
            HmiAction::MoveRobotToPosition {
                position_id: "robot-home".into(),
                speed_percent: 10.0,
            },
        )
        .expect("disabled taught motion report");
    assert!(matches!(no_enable.status, HmiActionStatus::Denied));
}

#[test]
fn robot_jog_and_taught_position_actions_cover_every_axis_and_validation_path() {
    let appliances = repository();
    let mut sessions = PlantRuntimeStore::default();
    set_mode(&mut sessions, &appliances, HmiControlMode::Manual);
    enable(&mut sessions, &appliances);

    for increment in [0.0, f64::NAN] {
        let report = sessions
            .execute(
                &appliances,
                JOYSTICK,
                HmiAction::JogRobot {
                    coordinate_system: HmiRobotCoordinateSystem::World,
                    axis: HmiRobotAxis::X,
                    increment,
                    speed_percent: 10.0,
                },
            )
            .expect("invalid increment report");
        assert!(matches!(report.status, HmiActionStatus::Denied));
    }
    for (coordinate_system, axis) in [
        (HmiRobotCoordinateSystem::World, HmiRobotAxis::J1),
        (HmiRobotCoordinateSystem::Joint, HmiRobotAxis::X),
    ] {
        let report = sessions
            .execute(
                &appliances,
                JOYSTICK,
                HmiAction::JogRobot {
                    coordinate_system,
                    axis,
                    increment: 1.0,
                    speed_percent: 10.0,
                },
            )
            .expect("axis mismatch report");
        assert!(matches!(report.status, HmiActionStatus::Denied));
    }

    for axis in [
        HmiRobotAxis::X,
        HmiRobotAxis::Y,
        HmiRobotAxis::Z,
        HmiRobotAxis::W,
        HmiRobotAxis::P,
        HmiRobotAxis::R,
    ] {
        let report = sessions
            .execute(
                &appliances,
                JOYSTICK,
                HmiAction::JogRobot {
                    coordinate_system: HmiRobotCoordinateSystem::World,
                    axis,
                    increment: 0.1,
                    speed_percent: 10.0,
                },
            )
            .expect("Cartesian jog report");
        assert!(matches!(report.status, HmiActionStatus::Applied));
        sessions.tick(100_000);
    }
    for axis in [
        HmiRobotAxis::J1,
        HmiRobotAxis::J2,
        HmiRobotAxis::J3,
        HmiRobotAxis::J4,
        HmiRobotAxis::J5,
        HmiRobotAxis::J6,
    ] {
        let report = sessions
            .execute(
                &appliances,
                JOYSTICK,
                HmiAction::JogRobot {
                    coordinate_system: HmiRobotCoordinateSystem::Joint,
                    axis,
                    increment: 0.1,
                    speed_percent: 10.0,
                },
            )
            .expect("joint jog report");
        assert!(matches!(report.status, HmiActionStatus::Applied));
        sessions.tick(100_000);
    }

    let unknown = sessions
        .execute(
            &appliances,
            JOYSTICK,
            HmiAction::MoveRobotToPosition {
                position_id: "missing-position".into(),
                speed_percent: 10.0,
            },
        )
        .expect("unknown position report");
    assert!(matches!(unknown.status, HmiActionStatus::Denied));
    let taught_id = sessions
        .profile(&appliances, JOYSTICK)
        .expect("robot profile")
        .robot
        .expect("robot state")
        .taught_positions[0]
        .id
        .clone();
    let known = sessions
        .execute(
            &appliances,
            JOYSTICK,
            HmiAction::MoveRobotToPosition {
                position_id: taught_id,
                speed_percent: 10.0,
            },
        )
        .expect("known position report");
    assert!(matches!(known.status, HmiActionStatus::Applied));
}

#[test]
fn robot_setup_program_and_guard_actions_enforce_transaction_boundaries() {
    let appliances = repository();
    let mut sessions = PlantRuntimeStore::default();
    set_mode(&mut sessions, &appliances, HmiControlMode::Manual);
    let denied_teach = sessions
        .execute(
            &appliances,
            JOYSTICK,
            HmiAction::TeachRobotPosition {
                position_id: "session-point".into(),
                label: "Session point".into(),
            },
        )
        .expect("manual teach report");
    assert!(matches!(denied_teach.status, HmiActionStatus::Denied));

    set_mode(&mut sessions, &appliances, HmiControlMode::Setup);
    for (position_id, label) in [
        ("INVALID ID".to_owned(), "label".to_owned()),
        ("session-point".to_owned(), " ".to_owned()),
        ("session-point".to_owned(), "x".repeat(65)),
    ] {
        let report = sessions
            .execute(
                &appliances,
                JOYSTICK,
                HmiAction::TeachRobotPosition { position_id, label },
            )
            .expect("invalid teach report");
        assert!(matches!(report.status, HmiActionStatus::Denied));
    }
    let taught = sessions
        .execute(
            &appliances,
            JOYSTICK,
            HmiAction::TeachRobotPosition {
                position_id: "session-point".into(),
                label: "Session point".into(),
            },
        )
        .expect("valid teach report");
    assert!(matches!(taught.status, HmiActionStatus::Applied));

    let long_name = sessions
        .execute(
            &appliances,
            JOYSTICK,
            HmiAction::LoadRobotProgram {
                name: "x".repeat(65),
                source: canonical_robot_program(),
            },
        )
        .expect("long program name report");
    assert!(matches!(long_name.status, HmiActionStatus::Denied));
    let invalid_source = sessions
        .execute(
            &appliances,
            JOYSTICK,
            HmiAction::LoadRobotProgram {
                name: "INVALID".into(),
                source: "not a robot program".into(),
            },
        )
        .expect("invalid program source report");
    assert!(matches!(invalid_source.status, HmiActionStatus::Denied));
    let loaded = sessions
        .execute(
            &appliances,
            JOYSTICK,
            HmiAction::LoadRobotProgram {
                name: "ACTION_MATRIX".into(),
                source: canonical_robot_program(),
            },
        )
        .expect("valid program load report");
    assert!(matches!(loaded.status, HmiActionStatus::Applied));

    for action in [HmiAction::RunRobotProgram, HmiAction::StepRobotProgram] {
        let report = sessions
            .execute(&appliances, JOYSTICK, action)
            .expect("program without enable report");
        assert!(matches!(report.status, HmiActionStatus::Denied));
    }
    enable(&mut sessions, &appliances);
    let running = sessions
        .execute(&appliances, JOYSTICK, HmiAction::RunRobotProgram)
        .expect("run program report");
    assert!(matches!(running.status, HmiActionStatus::Applied));
    let paused = sessions
        .execute(&appliances, JOYSTICK, HmiAction::PauseRobotProgram)
        .expect("pause program report");
    assert!(matches!(paused.status, HmiActionStatus::Applied));
    let reset = sessions
        .execute(&appliances, JOYSTICK, HmiAction::ResetRobotProgram)
        .expect("reset program report");
    assert!(matches!(reset.status, HmiActionStatus::Applied));
    let stepped = sessions
        .execute(&appliances, JOYSTICK, HmiAction::StepRobotProgram)
        .expect("step program report");
    assert!(matches!(stepped.status, HmiActionStatus::Applied));

    let denied_door = sessions
        .execute(
            &appliances,
            JOYSTICK,
            HmiAction::SetGuardDoor { open: true },
        )
        .expect("unauthorized door report");
    assert!(matches!(denied_door.status, HmiActionStatus::Denied));

    let mut guard_sessions = PlantRuntimeStore::default();
    let opened = guard_sessions
        .execute(
            &appliances,
            MACHINE_PC,
            HmiAction::SetGuardDoor { open: true },
        )
        .expect("open idle guard");
    assert!(matches!(opened.status, HmiActionStatus::Applied));
    let closed = guard_sessions
        .execute(
            &appliances,
            MACHINE_PC,
            HmiAction::SetGuardDoor { open: false },
        )
        .expect("close healthy guard");
    assert!(matches!(closed.status, HmiActionStatus::Applied));
}

#[test]
fn open_guard_checks_every_motion_shape_before_latching_a_trip() {
    let appliances = repository();
    let mut sessions = PlantRuntimeStore::default();
    let pose = HmiRobotPose {
        x: -620.0,
        y: 180.0,
        z: 1120.0,
        w: 0.0,
        p: 90.0,
        r: 0.0,
    };
    sessions
        .execute(
            &appliances,
            MACHINE_PC,
            HmiAction::SetGuardDoor { open: true },
        )
        .expect("open idle guard");

    for action in [
        HmiAction::Command {
            tag: "area-02-water-01-command".into(),
            value: "release-wet".into(),
        },
        HmiAction::Command {
            tag: "area-02-mould-01-command".into(),
            value: "stopped".into(),
        },
        HmiAction::Command {
            tag: "area-02-mould-01-command".into(),
            value: "isolated".into(),
        },
        HmiAction::Command {
            tag: "missing-mould-command".into(),
            value: "open".into(),
        },
        HmiAction::Command {
            tag: "area-02-mould-01-command".into(),
            value: "open".into(),
        },
        HmiAction::MoveRobot {
            target: HmiRobotPose {
                x: f64::NAN,
                ..pose
            },
            speed_percent: 10.0,
        },
        HmiAction::MoveRobot {
            target: pose,
            speed_percent: 0.0,
        },
        HmiAction::MoveRobot {
            target: pose,
            speed_percent: 10.0,
        },
        HmiAction::MoveRobotToPosition {
            position_id: "robot-home".into(),
            speed_percent: 0.0,
        },
        HmiAction::MoveRobotToPosition {
            position_id: "robot-home".into(),
            speed_percent: 10.0,
        },
        HmiAction::JogRobot {
            coordinate_system: HmiRobotCoordinateSystem::World,
            axis: HmiRobotAxis::X,
            increment: 0.0,
            speed_percent: 10.0,
        },
        HmiAction::JogRobot {
            coordinate_system: HmiRobotCoordinateSystem::World,
            axis: HmiRobotAxis::X,
            increment: 1.0,
            speed_percent: 0.0,
        },
        HmiAction::JogRobot {
            coordinate_system: HmiRobotCoordinateSystem::World,
            axis: HmiRobotAxis::J1,
            increment: 1.0,
            speed_percent: 10.0,
        },
        HmiAction::JogRobot {
            coordinate_system: HmiRobotCoordinateSystem::World,
            axis: HmiRobotAxis::X,
            increment: 1.0,
            speed_percent: 10.0,
        },
    ] {
        let report = sessions
            .execute(&appliances, JOYSTICK, action)
            .expect("invalid open-guard motion report");
        assert!(matches!(report.status, HmiActionStatus::Denied));
    }

    let mut sessions = PlantRuntimeStore::default();
    set_mode(&mut sessions, &appliances, HmiControlMode::Manual);
    enable(&mut sessions, &appliances);
    sessions
        .execute(
            &appliances,
            MACHINE_PC,
            HmiAction::SetGuardDoor { open: true },
        )
        .expect("open guard after enabling the pendant");
    for action in [
        HmiAction::MoveRobotToPosition {
            position_id: "missing-position".into(),
            speed_percent: 10.0,
        },
        HmiAction::JogRobot {
            coordinate_system: HmiRobotCoordinateSystem::World,
            axis: HmiRobotAxis::J1,
            increment: 1.0,
            speed_percent: 10.0,
        },
    ] {
        let report = sessions
            .execute(&appliances, JOYSTICK, action)
            .expect("authorized invalid open-guard motion report");
        assert!(matches!(report.status, HmiActionStatus::Denied));
    }

    let trip = sessions
        .execute(
            &appliances,
            JOYSTICK,
            HmiAction::MoveRobot {
                target: pose,
                speed_percent: 10.0,
            },
        )
        .expect("valid open-guard motion demand");
    assert!(matches!(trip.status, HmiActionStatus::Denied));
    assert!(
        trip.snapshot
            .alarms
            .iter()
            .any(|alarm| { alarm.code == "CELL-GUARD-MOTION-INHIBITED" && alarm.active })
    );
}

#[test]
fn process_actions_reject_invalid_authority_scope_and_lifecycle_transitions() {
    let appliances = repository();

    let mut body = PlantRuntimeStore::default();
    let unsafe_start = body
        .execute(&appliances, "area-01-hmi-01", HmiAction::StartProcess)
        .expect("unsafe batch start report");
    assert!(matches!(unsafe_start.status, HmiActionStatus::Denied));
    let idle_hold = body
        .execute(&appliances, "area-01-hmi-01", HmiAction::HoldProcess)
        .expect("idle batch hold report");
    assert!(matches!(idle_hold.status, HmiActionStatus::Denied));
    let idle_reset = body
        .execute(&appliances, "area-01-hmi-01", HmiAction::ResetProcess)
        .expect("idle batch reset report");
    assert!(matches!(idle_reset.status, HmiActionStatus::Denied));

    body.execute(
        &appliances,
        "area-01-hmi-01",
        HmiAction::ResetSafety {
            safety_id: "area-01-intlk-01".into(),
        },
    )
    .expect("reset batch safety");
    assert!(matches!(
        body.execute(&appliances, "area-01-hmi-01", HmiAction::StartProcess)
            .expect("start batch")
            .status,
        HmiActionStatus::Applied
    ));
    let duplicate_start = body
        .execute(&appliances, "area-01-hmi-01", HmiAction::StartProcess)
        .expect("duplicate batch start report");
    assert!(matches!(duplicate_start.status, HmiActionStatus::Denied));

    for action in [HmiAction::StartProcess, HmiAction::HoldProcess] {
        let report = body
            .execute(&appliances, "area-01-wt-hmi-01", action)
            .expect("wrong-train cell-wide action report");
        assert!(matches!(report.status, HmiActionStatus::Denied));
    }
    let wrong_fault_scope = body
        .execute(
            &appliances,
            "area-01-hmi-01",
            HmiAction::SetProcessFault {
                fault: HmiProcessFault::GlazeMillOverload,
                active: true,
            },
        )
        .expect("wrong-scope disturbance report");
    assert!(matches!(wrong_fault_scope.status, HmiActionStatus::Denied));

    let mut forming = PlantRuntimeStore::default();
    for (interface, action) in [
        ("area-02-machine-pc-01", HmiAction::HoldProcess),
        ("area-02-hmi-01", HmiAction::StopMouldAfterPhase),
        ("area-02-machine-pc-01", HmiAction::StopMouldAfterPhase),
        ("area-02-machine-pc-01", HmiAction::EndMouldAfterCycle),
        ("area-02-machine-pc-01", HmiAction::ResetProcess),
        ("area-02-joystick-01", HmiAction::ResetProcess),
    ] {
        let report = forming
            .execute(&appliances, interface, action)
            .expect("invalid Forming lifecycle report");
        assert!(matches!(report.status, HmiActionStatus::Denied));
    }
    let unauthorized_fault = forming
        .execute(
            &appliances,
            "area-02-joystick-01",
            HmiAction::SetProcessFault {
                fault: HmiProcessFault::RobotPickupFailure,
                active: true,
            },
        )
        .expect("unauthorized fault injection report");
    assert!(matches!(unauthorized_fault.status, HmiActionStatus::Denied));

    forming
        .execute(&appliances, "area-02-hmi-01", HmiAction::StartMould)
        .expect("start Mould 1");
    let duplicate_mould_start = forming
        .execute(&appliances, "area-02-hmi-01", HmiAction::StartMould)
        .expect("duplicate mould start report");
    assert!(matches!(
        duplicate_mould_start.status,
        HmiActionStatus::Denied
    ));
}
