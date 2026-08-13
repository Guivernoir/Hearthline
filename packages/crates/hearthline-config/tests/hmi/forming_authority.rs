use super::*;

#[test]
fn forming_local_selectors_enforce_mode_and_setup_safety_boundaries() {
    let appliances = repository();
    let mut session =
        HmiSession::from_repository(&appliances, "area-02-hmi-01").expect("Mould 1 HMI");

    let denied = session.execute(HmiAction::Command {
        tag: "area-02-mould-01-command".into(),
        value: "open".into(),
    });
    assert!(matches!(denied.status, HmiActionStatus::Denied));

    let manual = session.execute(HmiAction::SetControlMode {
        mode: HmiControlMode::Manual,
        password: None,
    });
    assert!(matches!(manual.status, HmiActionStatus::Applied));
    let movement = session.execute(HmiAction::Command {
        tag: "area-02-mould-01-command".into(),
        value: "open".into(),
    });
    assert!(matches!(movement.status, HmiActionStatus::Applied));

    let wrong_password = session.execute(HmiAction::SetControlMode {
        mode: HmiControlMode::Setup,
        password: Some("incorrect".into()),
    });
    assert!(matches!(wrong_password.status, HmiActionStatus::Denied));

    let setup = session.execute(HmiAction::SetControlMode {
        mode: HmiControlMode::Setup,
        password: Some("VanillaIceCream1!".into()),
    });
    assert!(matches!(setup.status, HmiActionStatus::Applied));
    let station = setup.snapshot.control_station.expect("control station");
    assert!(station.setup_authenticated);
    assert!(station.sensor_bypass_active);
    assert_eq!(station.bypassed_permissives, ["process-sensor-permissives"]);
    assert_eq!(
        station.retained_protections,
        ["emergency-stop-chain", "hardwired-travel-limits"]
    );
}

#[test]
fn forming_machine_pc_requires_local_manual_authority_and_cannot_drive_robot() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    let mould_one_valve = HmiAction::Command {
        tag: "area-02-water-01-command".into(),
        value: "release-wet".into(),
    };
    let denied_mould_one = sessions
        .execute(
            &appliances,
            "area-02-machine-pc-01",
            mould_one_valve.clone(),
        )
        .expect("denied Mould 1 valve command");
    assert!(matches!(denied_mould_one.status, HmiActionStatus::Denied));
    sessions
        .execute(
            &appliances,
            "area-02-hmi-01",
            HmiAction::SetControlMode {
                mode: HmiControlMode::Manual,
                password: None,
            },
        )
        .expect("Mould 1 manual selector");
    let applied_mould_one = sessions
        .execute(&appliances, "area-02-machine-pc-01", mould_one_valve)
        .expect("authorized Mould 1 valve command");
    assert!(matches!(applied_mould_one.status, HmiActionStatus::Applied));

    let command = HmiAction::Command {
        tag: "area-02-m02-manifold-01-command".into(),
        value: "release-water-left".into(),
    };

    let denied = sessions
        .execute(&appliances, "area-02-machine-pc-01", command.clone())
        .expect("denied PC valve command");
    assert!(matches!(denied.status, HmiActionStatus::Denied));

    let manual = sessions
        .execute(
            &appliances,
            "area-02-hmi-02",
            HmiAction::SetControlMode {
                mode: HmiControlMode::Manual,
                password: None,
            },
        )
        .expect("Mould 2 manual selector");
    assert!(matches!(manual.status, HmiActionStatus::Applied));

    let applied = sessions
        .execute(&appliances, "area-02-machine-pc-01", command)
        .expect("authorized PC valve command");
    assert!(matches!(applied.status, HmiActionStatus::Applied));

    let robot = sessions
        .execute(
            &appliances,
            "area-02-machine-pc-01",
            HmiAction::Command {
                tag: "area-02-robot-01-command".into(),
                value: "pickup".into(),
            },
        )
        .expect("PC robot command");
    assert!(matches!(robot.status, HmiActionStatus::Denied));
}

#[test]
fn forming_machine_pc_owns_parameters_recipes_and_control_source() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();

    let parameter = sessions
        .execute(
            &appliances,
            "area-02-machine-pc-01",
            HmiAction::SetParameter {
                parameter_id: "mould-03-pressure-bar".into(),
                value: 6.8,
            },
        )
        .expect("parameter update");
    assert!(matches!(parameter.status, HmiActionStatus::Applied));

    let recipe = sessions
        .execute(
            &appliances,
            "area-02-machine-pc-01",
            HmiAction::SelectRecipe {
                recipe_id: "thin-wall".into(),
            },
        )
        .expect("recipe selection");
    assert!(matches!(recipe.status, HmiActionStatus::Applied));
    assert_eq!(recipe.snapshot.active_recipe.as_deref(), Some("thin-wall"));
    assert_eq!(
        recipe
            .snapshot
            .parameters
            .iter()
            .find(|parameter| parameter.id == "mould-03-pressure-bar")
            .expect("updated parameter")
            .value,
        6.8
    );
    assert_eq!(
        recipe
            .snapshot
            .moulds
            .iter()
            .find(|mould| mould.target == "mould-03")
            .expect("Mould 3 runtime")
            .casting_pressure_bar,
        6.8
    );

    assert!(
        sessions
            .control_program(&appliances, "area-02-machine-pc-01")
            .expect("PC source")
            .is_some()
    );
    assert!(
        sessions
            .control_program(&appliances, "area-02-hmi-01")
            .expect("local panel source")
            .is_none()
    );
}

#[test]
fn each_mould_owns_its_start_and_machine_pc_start_is_denied() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    sessions
        .execute(
            &appliances,
            "area-02-joystick-01",
            HmiAction::SetControlMode {
                mode: HmiControlMode::Manual,
                password: None,
            },
        )
        .expect("robot manual selector");

    let pc_start = sessions
        .execute(
            &appliances,
            "area-02-machine-pc-01",
            HmiAction::StartProcess,
        )
        .expect("machine PC start");
    assert!(matches!(pc_start.status, HmiActionStatus::Denied));
    assert!(
        pc_start
            .message
            .contains("Start each mould from its local HMI")
    );

    let started = sessions
        .execute(&appliances, "area-02-hmi-01", HmiAction::StartMould)
        .expect("Mould 1 start");
    assert!(matches!(started.status, HmiActionStatus::Applied));
    assert!(started.snapshot.moulds[0].production_enabled);
    assert!(
        started
            .snapshot
            .moulds
            .iter()
            .skip(1)
            .all(|mould| !mould.production_enabled)
    );
}

#[test]
fn forming_manual_commands_remain_selected_until_replaced() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    sessions
        .execute(
            &appliances,
            "area-02-hmi-01",
            HmiAction::SetControlMode {
                mode: HmiControlMode::Manual,
                password: None,
            },
        )
        .expect("manual mode");

    for state in ["open", "closing"] {
        let command = sessions
            .execute(
                &appliances,
                "area-02-hmi-01",
                HmiAction::Command {
                    tag: "area-02-mould-01-command".into(),
                    value: state.into(),
                },
            )
            .expect("retained movement command");
        assert!(matches!(command.status, HmiActionStatus::Applied));
        sessions.tick(1_000);
        let retained = sessions
            .profile(&appliances, "area-02-hmi-01")
            .expect("retained command state");
        assert_eq!(retained.actuators[0].current_state, state);
    }
}

#[test]
fn forming_robot_manual_commands_remain_selected_until_replaced() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    sessions
        .execute(
            &appliances,
            "area-02-joystick-01",
            HmiAction::SetControlMode {
                mode: HmiControlMode::Manual,
                password: None,
            },
        )
        .expect("robot manual mode");
    sessions
        .execute(
            &appliances,
            "area-02-joystick-01",
            HmiAction::SetRobotMotionEnable { enabled: true },
        )
        .expect("robot motion enable");

    for state in ["approaching", "delivering"] {
        let command = sessions
            .execute(
                &appliances,
                "area-02-joystick-01",
                HmiAction::Command {
                    tag: "area-02-robot-01-command".into(),
                    value: state.into(),
                },
            )
            .expect("retained robot motion command");
        assert!(matches!(command.status, HmiActionStatus::Applied));
        sessions.tick(1_000);
        let retained = sessions
            .profile(&appliances, "area-02-joystick-01")
            .expect("retained robot command state");
        let actuator = retained
            .actuators
            .iter()
            .find(|actuator| actuator.command_tag == "area-02-robot-01-command")
            .expect("robot command actuator");
        assert_eq!(actuator.current_state, state);
    }
}

#[test]
fn robot_motion_interpolates_and_enforces_pendant_enable_and_workspace() {
    let appliances = repository();
    let mut sessions = HmiSessionStore::default();
    sessions
        .execute(
            &appliances,
            "area-02-joystick-01",
            HmiAction::SetControlMode {
                mode: HmiControlMode::Manual,
                password: None,
            },
        )
        .expect("robot manual mode");

    let target = HmiRobotPose {
        x: -620.0,
        y: 180.0,
        z: 1120.0,
        w: 0.0,
        p: 90.0,
        r: 0.0,
    };
    let denied = sessions
        .execute(
            &appliances,
            "area-02-joystick-01",
            HmiAction::MoveRobot {
                target,
                speed_percent: 20.0,
            },
        )
        .expect("motion without enable");
    assert!(matches!(denied.status, HmiActionStatus::Denied));

    sessions
        .execute(
            &appliances,
            "area-02-joystick-01",
            HmiAction::SetRobotMotionEnable { enabled: true },
        )
        .expect("motion enable");
    let moving = sessions
        .execute(
            &appliances,
            "area-02-joystick-01",
            HmiAction::MoveRobot {
                target,
                speed_percent: 20.0,
            },
        )
        .expect("Cartesian move");
    assert!(matches!(moving.status, HmiActionStatus::Applied));
    sessions.tick(500);
    let profile = sessions
        .profile(&appliances, "area-02-joystick-01")
        .expect("moving profile");
    let robot = profile.robot.expect("robot state");
    assert!(robot.motion.active);
    assert!(robot.motion.progress_percent > 0.0 && robot.motion.progress_percent < 100.0);
    assert!(robot.pose.x < 0.0 && robot.pose.x > target.x);

    sessions.tick(20_000);
    let complete = sessions
        .profile(&appliances, "area-02-joystick-01")
        .expect("completed profile")
        .robot
        .expect("robot state");
    assert!(!complete.motion.active);
    assert_eq!(complete.pose, target);

    let outside = sessions
        .execute(
            &appliances,
            "area-02-joystick-01",
            HmiAction::MoveRobot {
                target: HmiRobotPose {
                    x: 9_999.0,
                    ..target
                },
                speed_percent: 20.0,
            },
        )
        .expect("out-of-workspace motion");
    assert!(matches!(outside.status, HmiActionStatus::Denied));
}
