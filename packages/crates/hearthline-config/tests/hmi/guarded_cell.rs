use super::*;

const GUARD_SAFETY: &str = "area-02-cell-guard-safe-01";

fn guard(snapshot: &hearthline_config::HmiSnapshot) -> &hearthline_config::HmiCellGuardState {
    &snapshot
        .guarded_cell
        .as_ref()
        .expect("guarded forming cell")
        .guard
}

#[test]
fn open_gate_inhibits_motion_and_requires_close_then_reset() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();

    let opened = sessions
        .execute(
            &appliances,
            "area-02-machine-pc-01",
            HmiAction::SetGuardDoor { open: true },
        )
        .expect("open access gate");
    assert!(matches!(opened.status, HmiActionStatus::Applied));
    assert_eq!(guard(&opened.snapshot).position, "open");
    assert!(!guard(&opened.snapshot).reset_required);

    let denied = sessions
        .execute(&appliances, "area-02-hmi-01", HmiAction::StartMould)
        .expect("inhibited mould start");
    assert!(matches!(denied.status, HmiActionStatus::Denied));
    assert!(guard(&denied.snapshot).reset_required);
    assert!(denied.snapshot.alarms.iter().any(|alarm| {
        alarm.active && alarm.code == "CELL-GUARD-MOTION-INHIBITED" && alarm.source == GUARD_SAFETY
    }));

    let reset_open = sessions
        .execute(
            &appliances,
            "area-02-machine-pc-01",
            HmiAction::ResetSafety {
                safety_id: GUARD_SAFETY.into(),
            },
        )
        .expect("reset with gate open");
    assert!(matches!(reset_open.status, HmiActionStatus::Denied));

    sessions
        .execute(
            &appliances,
            "area-02-machine-pc-01",
            HmiAction::SetGuardDoor { open: false },
        )
        .expect("close access gate");
    let reset = sessions
        .execute(
            &appliances,
            "area-02-machine-pc-01",
            HmiAction::ResetSafety {
                safety_id: GUARD_SAFETY.into(),
            },
        )
        .expect("reset closed guard");
    assert!(matches!(reset.status, HmiActionStatus::Applied));
    assert!(!guard(&reset.snapshot).reset_required);
    assert!(
        reset
            .snapshot
            .alarms
            .iter()
            .all(|alarm| { alarm.code != "CELL-GUARD-MOTION-INHIBITED" || !alarm.active })
    );

    let started = sessions
        .execute(&appliances, "area-02-hmi-01", HmiAction::StartMould)
        .expect("start after guard reset");
    assert!(matches!(started.status, HmiActionStatus::Applied));
}

#[test]
fn opening_gate_during_a_cycle_faults_and_freezes_the_mould() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    sessions
        .execute(&appliances, "area-02-hmi-01", HmiAction::StartMould)
        .expect("start Mould 1");
    sessions.tick(700);

    let opened = sessions
        .execute(
            &appliances,
            "area-02-machine-pc-01",
            HmiAction::SetGuardDoor { open: true },
        )
        .expect("open gate during cycle");
    let before = opened
        .snapshot
        .moulds
        .iter()
        .find(|mould| mould.target == "mould-01")
        .expect("Mould 1");
    assert_eq!(before.phase, "faulted");
    assert!(!before.running);
    let scan_count = before.scan_count;

    sessions.tick(5_000);
    let held = sessions
        .profile(&appliances, "area-02-machine-pc-01")
        .expect("held cell state");
    let mould = held
        .moulds
        .iter()
        .find(|mould| mould.target == "mould-01")
        .expect("Mould 1");
    assert_eq!(mould.phase, "faulted");
    assert_eq!(mould.scan_count, scan_count);
    assert!(guard(&held).reset_required);
}

#[test]
fn open_gate_denies_manual_mould_and_robot_motion() {
    let appliances = repository();
    let mut mould_sessions = HmiSessionStore::default();
    mould_sessions
        .execute(
            &appliances,
            "area-02-machine-pc-01",
            HmiAction::SetGuardDoor { open: true },
        )
        .expect("open gate for mould test");
    mould_sessions
        .execute(
            &appliances,
            "area-02-hmi-01",
            HmiAction::SetControlMode {
                mode: HmiControlMode::Manual,
                password: None,
            },
        )
        .expect("select mould manual mode");
    let mould_move = mould_sessions
        .execute(
            &appliances,
            "area-02-hmi-01",
            HmiAction::Command {
                tag: "area-02-mould-01-command".into(),
                value: "opening".into(),
            },
        )
        .expect("inhibited mould movement");
    assert!(matches!(mould_move.status, HmiActionStatus::Denied));
    assert_eq!(
        mould_move
            .snapshot
            .actuators
            .iter()
            .find(|actuator| actuator.command_tag == "area-02-mould-01-command")
            .expect("Mould 1 actuator")
            .current_state,
        "closed"
    );

    let mut robot_sessions = HmiSessionStore::default();
    robot_sessions
        .execute(
            &appliances,
            "area-02-machine-pc-01",
            HmiAction::SetGuardDoor { open: true },
        )
        .expect("open gate for robot test");
    robot_sessions
        .execute(
            &appliances,
            "area-02-joystick-01",
            HmiAction::SetControlMode {
                mode: HmiControlMode::Manual,
                password: None,
            },
        )
        .expect("select robot manual mode");
    robot_sessions
        .execute(
            &appliances,
            "area-02-joystick-01",
            HmiAction::SetRobotMotionEnable { enabled: true },
        )
        .expect("enable pendant motion");
    let robot_move = robot_sessions
        .execute(
            &appliances,
            "area-02-joystick-01",
            HmiAction::MoveRobot {
                target: HmiRobotPose {
                    x: -620.0,
                    y: 180.0,
                    z: 1120.0,
                    w: 0.0,
                    p: 90.0,
                    r: 0.0,
                },
                speed_percent: 20.0,
            },
        )
        .expect("inhibited robot movement");
    assert!(matches!(robot_move.status, HmiActionStatus::Denied));
    assert!(
        !robot_move
            .snapshot
            .robot
            .expect("robot state")
            .motion
            .active
    );
}

#[test]
fn configured_handoffs_begin_inside_the_guarded_cell() {
    let appliances = repository();
    let snapshot = HmiSession::from_repository(&appliances, "area-02-machine-pc-01")
        .expect("forming machine PC")
        .snapshot();
    let cell = snapshot.guarded_cell.expect("guarded forming cell");
    assert_eq!(cell.handoff_stations.len(), 4);
    assert!(cell.handoff_stations.iter().all(|station| {
        station.state == "in-cell"
            && station.in_cell_confirmed
            && !station.operator_side_confirmed
            && !station.piece_present
    }));
}
