use std::fs;
use std::path::PathBuf;

use hearthline_config::{
    ApplianceConfig, BehaviorConfig, OperatorControlMode, RobotMotionProfileConfig,
    SupervisoryProfileConfig,
};

fn appliance_source(relative: &str) -> String {
    fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../project/config/appliances")
            .join(relative),
    )
    .expect("canonical appliance source")
}

fn robot_controller() -> ApplianceConfig {
    ApplianceConfig::from_yaml(&appliance_source(
        "factory/process/material/forming/field/robotic-demould/area-02-robot-controller-01.yaml",
    ))
    .expect("canonical robot controller")
}

fn invalid_robot(change: impl FnOnce(&mut RobotMotionProfileConfig)) {
    let mut config = robot_controller();
    let BehaviorConfig::FieldActuator {
        motion_profile: Some(profile),
        ..
    } = &mut config.behavior
    else {
        panic!("robot controller motion profile");
    };
    change(profile);
    config
        .validate()
        .expect_err("mutated robot profile must fail");
}

fn canonical(relative: &str) -> ApplianceConfig {
    ApplianceConfig::from_yaml(&appliance_source(relative)).expect("canonical appliance")
}

fn invalid_behavior(relative: &str, change: impl FnOnce(&mut BehaviorConfig)) {
    let mut config = canonical(relative);
    change(&mut config.behavior);
    config.validate().expect_err("mutated behavior must fail");
}

fn invalid_machine(change: impl FnOnce(&mut BehaviorConfig)) {
    invalid_behavior(
        "factory/process/material/forming/control/operator/area-02-machine-pc-01.yaml",
        change,
    );
}

fn invalid_selector(change: impl FnOnce(&mut BehaviorConfig)) {
    invalid_behavior(
        "factory/process/material/forming/control/operator/area-02-joystick-01.yaml",
        change,
    );
}

fn invalid_supervisory(change: impl FnOnce(&mut SupervisoryProfileConfig)) {
    invalid_machine(|behavior| {
        let BehaviorConfig::OperatorInterface {
            supervisory_profile: Some(profile),
            ..
        } = behavior
        else {
            panic!("supervisory profile");
        };
        change(profile);
    });
}

#[test]
fn robot_profile_validation_rejects_every_structural_boundary() {
    robot_controller().validate().expect("reference profile");

    invalid_robot(|p| p.architecture.manipulator = "bad id".into());
    invalid_robot(|p| p.architecture.servo_axes = 5);
    invalid_robot(|p| p.architecture.interpolation_cycle_ms = 0);
    invalid_robot(|p| p.program_ref = "program.st".into());

    invalid_robot(|p| p.max_linear_speed_mm_s = f64::NAN);
    invalid_robot(|p| p.max_linear_speed_mm_s = 0.0);
    invalid_robot(|p| p.max_joint_speed_deg_s = f64::NAN);
    invalid_robot(|p| p.max_joint_speed_deg_s = 0.0);
    invalid_robot(|p| p.default_speed_percent = f64::NAN);
    invalid_robot(|p| p.default_speed_percent = 101.0);
    invalid_robot(|p| p.default_speed_percent = 0.0);

    invalid_robot(|p| p.workspace.minimum.x = f64::NAN);
    invalid_robot(|p| p.workspace.maximum.x = f64::NAN);
    invalid_robot(|p| p.workspace.minimum.x = p.workspace.maximum.x);
    invalid_robot(|p| p.workspace.joint_minimum[0] = f64::NAN);
    invalid_robot(|p| p.workspace.joint_maximum[0] = f64::NAN);
    invalid_robot(|p| p.workspace.joint_minimum[0] = p.workspace.joint_maximum[0]);
    invalid_robot(|p| p.home.x = p.workspace.maximum.x + 1.0);
    invalid_robot(|p| p.home.x = f64::NAN);

    invalid_robot(|p| p.taught_positions[0].id = "bad id".into());
    invalid_robot(|p| p.taught_positions[1].id = p.taught_positions[0].id.clone());
    invalid_robot(|p| p.taught_positions[0].label.clear());
    invalid_robot(|p| p.taught_positions[0].pose.x = p.workspace.maximum.x + 1.0);
    invalid_robot(|p| p.taught_positions.retain(|position| position.id != "home"));

    invalid_robot(|p| p.frames.clear());
    invalid_robot(|p| p.frames[0].id = "bad id".into());
    invalid_robot(|p| p.frames[1].id = p.frames[0].id.clone());
    invalid_robot(|p| p.frames[0].label.clear());
    invalid_robot(|p| p.frames[0].pose.x = f64::NAN);
    invalid_robot(|p| p.frames[0].parent = Some("missing-frame".into()));
    invalid_robot(|p| p.active_user_frame = "missing-frame".into());

    invalid_robot(|p| p.payloads.clear());
    invalid_robot(|p| p.payloads[0].id = "bad id".into());
    invalid_robot(|p| p.payloads.push(p.payloads[0].clone()));
    invalid_robot(|p| p.payloads[0].label.clear());
    invalid_robot(|p| p.payloads[0].mass_kg = f64::NAN);
    invalid_robot(|p| p.payloads[0].mass_kg = 0.0);
    invalid_robot(|p| p.payloads[0].center_of_mass_mm[0] = f64::NAN);
    invalid_robot(|p| p.active_payload = "missing-payload".into());

    invalid_robot(|p| p.tools.clear());
    invalid_robot(|p| p.tools[0].id = "bad id".into());
    invalid_robot(|p| p.tools.push(p.tools[0].clone()));
    invalid_robot(|p| p.tools[0].label.clear());
    invalid_robot(|p| p.tools[0].tcp.x = f64::NAN);
    invalid_robot(|p| p.tools[0].payload = "missing-payload".into());
    invalid_robot(|p| p.active_tool = "missing-tool".into());

    invalid_robot(|p| p.handoffs.clear());
    invalid_robot(|p| p.handoffs[0].mould = "bad id".into());
    invalid_robot(|p| p.handoffs[1].mould = p.handoffs[0].mould.clone());
    invalid_robot(|p| p.handoffs[0].program.clear());
    invalid_robot(|p| p.handoffs[1].program = p.handoffs[0].program.clone());
    invalid_robot(|p| p.handoffs[0].program = "P0201".into());
    invalid_robot(|p| p.handoffs[0].program = "O2A01".into());
    invalid_robot(|p| p.handoffs[0].pickup_tolerance_mm = f64::NAN);
    invalid_robot(|p| p.handoffs[0].pickup_tolerance_mm = 0.0);
    invalid_robot(|p| p.handoffs[0].handoff_tolerance_mm = f64::NAN);
    invalid_robot(|p| p.handoffs[0].handoff_tolerance_mm = 0.0);
    invalid_robot(|p| p.handoffs[0].orientation_tolerance_deg = f64::NAN);
    invalid_robot(|p| p.handoffs[0].orientation_tolerance_deg = 0.0);
    invalid_robot(|p| p.handoffs[0].user_frame = "missing-frame".into());
    invalid_robot(|p| p.handoffs[0].approach_position = "missing-position".into());
    invalid_robot(|p| p.handoffs[0].pickup_position = "missing-position".into());
    invalid_robot(|p| p.handoffs[0].handoff_position = "missing-position".into());
    invalid_robot(|p| p.handoffs[0].retreat_position = "missing-position".into());
}

#[test]
fn operator_validation_rejects_authority_parameter_and_recipe_boundary_errors() {
    invalid_machine(|behavior| {
        let BehaviorConfig::OperatorInterface { controller, .. } = behavior else {
            unreachable!()
        };
        *controller = "bad id".into();
    });
    invalid_machine(|behavior| {
        let BehaviorConfig::OperatorInterface { permissions, .. } = behavior else {
            unreachable!()
        };
        permissions.clear();
    });
    invalid_machine(|behavior| {
        let BehaviorConfig::OperatorInterface { permissions, .. } = behavior else {
            unreachable!()
        };
        permissions[0].clear();
    });
    invalid_machine(|behavior| {
        let BehaviorConfig::OperatorInterface { permissions, .. } = behavior else {
            unreachable!()
        };
        permissions.push(permissions[0].clone());
    });
    for field in ["signal", "command", "safety"] {
        invalid_machine(|behavior| {
            let BehaviorConfig::OperatorInterface {
                signal_tags,
                command_tags,
                safety_components,
                ..
            } = behavior
            else {
                unreachable!()
            };
            let values = match field {
                "signal" => signal_tags,
                "command" => command_tags,
                _ => safety_components,
            };
            values.push("bad id".into());
        });
    }

    invalid_selector(|behavior| {
        let BehaviorConfig::OperatorInterface {
            control_station: Some(station),
            ..
        } = behavior
        else {
            unreachable!()
        };
        station.target = "bad id".into();
    });
    invalid_selector(|behavior| {
        let BehaviorConfig::OperatorInterface {
            control_station: Some(station),
            ..
        } = behavior
        else {
            unreachable!()
        };
        station.mode_selector.as_mut().unwrap().positions.clear();
    });
    invalid_selector(|behavior| {
        let BehaviorConfig::OperatorInterface {
            control_station: Some(station),
            ..
        } = behavior
        else {
            unreachable!()
        };
        station.mode_selector.as_mut().unwrap().initial_position = OperatorControlMode::Manual;
        station
            .mode_selector
            .as_mut()
            .unwrap()
            .positions
            .retain(|mode| *mode != OperatorControlMode::Manual);
    });
    invalid_selector(|behavior| {
        let BehaviorConfig::OperatorInterface {
            control_station: Some(station),
            ..
        } = behavior
        else {
            unreachable!()
        };
        station
            .mode_selector
            .as_mut()
            .unwrap()
            .positions
            .push(OperatorControlMode::Auto);
    });
    invalid_selector(|behavior| {
        let BehaviorConfig::OperatorInterface {
            control_station: Some(station),
            ..
        } = behavior
        else {
            unreachable!()
        };
        station
            .mode_selector
            .as_mut()
            .unwrap()
            .setup_password_sha256 = None;
    });
    invalid_selector(|behavior| {
        let BehaviorConfig::OperatorInterface {
            control_station: Some(station),
            ..
        } = behavior
        else {
            unreachable!()
        };
        station
            .mode_selector
            .as_mut()
            .unwrap()
            .setup_password_sha256 = Some("short".into());
    });
    invalid_selector(|behavior| {
        let BehaviorConfig::OperatorInterface {
            control_station: Some(station),
            ..
        } = behavior
        else {
            unreachable!()
        };
        station
            .mode_selector
            .as_mut()
            .unwrap()
            .setup_password_sha256 = Some("z".repeat(64));
    });
    invalid_selector(|behavior| {
        let BehaviorConfig::OperatorInterface {
            control_station: Some(station),
            ..
        } = behavior
        else {
            unreachable!()
        };
        station
            .mode_selector
            .as_mut()
            .unwrap()
            .bypassed_permissives
            .push(String::new());
    });
    invalid_selector(|behavior| {
        let BehaviorConfig::OperatorInterface {
            control_station: Some(station),
            ..
        } = behavior
        else {
            unreachable!()
        };
        station
            .mode_selector
            .as_mut()
            .unwrap()
            .retained_protections
            .push(String::new());
    });

    invalid_machine(|behavior| {
        let BehaviorConfig::OperatorInterface { parameters, .. } = behavior else {
            unreachable!()
        };
        parameters[0].id = "bad id".into();
    });
    invalid_machine(|behavior| {
        let BehaviorConfig::OperatorInterface { parameters, .. } = behavior else {
            unreachable!()
        };
        parameters[1].id = parameters[0].id.clone();
    });
    for case in 0..8 {
        invalid_machine(|behavior| {
            let BehaviorConfig::OperatorInterface { parameters, .. } = behavior else {
                unreachable!()
            };
            let parameter = &mut parameters[0];
            match case {
                0 => parameter.minimum = f64::NAN,
                1 => parameter.maximum = f64::NAN,
                2 => parameter.step = f64::NAN,
                3 => parameter.initial_value = f64::NAN,
                4 => parameter.minimum = parameter.maximum,
                5 => parameter.step = 0.0,
                6 => parameter.initial_value = parameter.minimum - 1.0,
                _ => parameter.initial_value = parameter.maximum + 1.0,
            }
        });
    }
    invalid_machine(|behavior| {
        let BehaviorConfig::OperatorInterface { recipes, .. } = behavior else {
            unreachable!()
        };
        recipes[0].id = "bad id".into();
    });
    invalid_machine(|behavior| {
        let BehaviorConfig::OperatorInterface { recipes, .. } = behavior else {
            unreachable!()
        };
        recipes[1].id = recipes[0].id.clone();
    });
    invalid_machine(|behavior| {
        let BehaviorConfig::OperatorInterface { active_recipe, .. } = behavior else {
            unreachable!()
        };
        *active_recipe = Some("missing-recipe".into());
    });
}

#[test]
fn supervisory_validation_rejects_each_metadata_graph_and_history_boundary() {
    invalid_supervisory(|p| p.namespace.clear());
    invalid_supervisory(|p| p.model_id.clear());
    invalid_supervisory(|p| p.repository.revision.clear());
    invalid_supervisory(|p| p.repository.deployed_revision.clear());
    invalid_supervisory(|p| p.repository.engineering_node = "bad id".into());

    invalid_supervisory(|p| p.templates.clear());
    invalid_supervisory(|p| p.templates[0].id = "bad id".into());
    invalid_supervisory(|p| p.templates[1].id = p.templates[0].id.clone());
    invalid_supervisory(|p| p.templates[0].label.clear());
    invalid_supervisory(|p| p.templates[0].attributes.clear());
    invalid_supervisory(|p| p.templates[0].parent = Some("missing-template".into()));

    invalid_supervisory(|p| p.assets.clear());
    invalid_supervisory(|p| p.assets[0].id = "bad id".into());
    invalid_supervisory(|p| p.assets[1].id = p.assets[0].id.clone());
    invalid_supervisory(|p| p.assets[0].label.clear());
    invalid_supervisory(|p| p.assets[0].template = "missing-template".into());
    invalid_supervisory(|p| p.assets[0].parent = Some("missing-asset".into()));

    invalid_supervisory(|p| p.deployment_nodes.clear());
    invalid_supervisory(|p| p.deployment_nodes[0].id = "bad id".into());
    invalid_supervisory(|p| p.deployment_nodes[1].id = p.deployment_nodes[0].id.clone());
    invalid_supervisory(|p| p.deployment_nodes[0].label.clear());
    invalid_supervisory(|p| p.deployment_nodes[0].host = "bad id".into());
    invalid_supervisory(|p| p.repository.engineering_node = "missing-node".into());

    invalid_supervisory(|p| p.roles.clear());
    invalid_supervisory(|p| p.roles[0].id = "bad id".into());
    invalid_supervisory(|p| p.roles[1].id = p.roles[0].id.clone());
    invalid_supervisory(|p| p.roles[0].label.clear());
    invalid_supervisory(|p| p.roles[0].permissions.clear());
    invalid_supervisory(|p| p.identity.role = "missing-role".into());
    invalid_supervisory(|p| p.identity.user.clear());
    invalid_supervisory(|p| p.identity.authentication.clear());
    invalid_supervisory(|p| p.history.sample_interval_ms = 0);
    invalid_supervisory(|p| p.history.capacity = 7);
    invalid_supervisory(|p| p.history.capacity = 121);
    invalid_supervisory(|p| p.history.tags.clear());
    invalid_supervisory(|p| p.history.tags.push(String::new()));
    invalid_supervisory(|p| p.history.tags.push(p.history.tags[0].clone()));
}

#[test]
fn cabinet_sensor_actuator_and_safety_validation_cover_off_nominal_shapes() {
    const REMOTE: &str =
        "factory/process/material/forming/control/automation/area-02-m03-rio-01.yaml";
    for case in 0..6 {
        invalid_behavior(REMOTE, |behavior| {
            let BehaviorConfig::RemoteIo {
                control_cabinet: Some(cabinet),
                ..
            } = behavior
            else {
                unreachable!()
            };
            match case {
                0 => cabinet.target = "bad id".into(),
                1 => cabinet.utility_cabinet = "bad id".into(),
                2 => cabinet.enclosure_rating.clear(),
                3 => cabinet.safety_relay.clear(),
                4 => cabinet.control_voltage_vdc = 0,
                _ => cabinet.modules.clear(),
            }
        });
    }
    invalid_behavior(REMOTE, |behavior| {
        let BehaviorConfig::RemoteIo {
            control_cabinet: Some(cabinet),
            ..
        } = behavior
        else {
            unreachable!()
        };
        cabinet.modules.push(cabinet.modules[0].clone());
    });

    const UTILITY: &str = "factory/process/material/forming/field/pressure-casting/mould-03/outputs/area-02-m03-manifold-01.yaml";
    for case in 0..10 {
        invalid_behavior(UTILITY, |behavior| {
            let BehaviorConfig::FieldActuator {
                utility_cabinet: Some(cabinet),
                ..
            } = behavior
            else {
                unreachable!()
            };
            match case {
                0 => cabinet.target = "bad id".into(),
                1 => cabinet.remote_io = "bad id".into(),
                2 => cabinet.circuits[0].id = "bad id".into(),
                3 => cabinet.enclosure_rating.clear(),
                4 => cabinet.isolation_state.clear(),
                5 => cabinet.control_voltage_vdc = 0,
                6 => cabinet.circuits.clear(),
                7 => cabinet.circuits[0].label.clear(),
                8 => cabinet.circuits[0].source.clear(),
                _ => cabinet.circuits[0].command_states.clear(),
            }
        });
    }
    invalid_behavior(UTILITY, |behavior| {
        let BehaviorConfig::FieldActuator {
            utility_cabinet: Some(cabinet),
            ..
        } = behavior
        else {
            unreachable!()
        };
        cabinet.circuits[0].nominal_pressure = Some(f64::NAN);
    });
    invalid_behavior(UTILITY, |behavior| {
        let BehaviorConfig::FieldActuator {
            utility_cabinet: Some(cabinet),
            ..
        } = behavior
        else {
            unreachable!()
        };
        cabinet.circuits.push(cabinet.circuits[0].clone());
    });
    invalid_behavior(UTILITY, |behavior| {
        let BehaviorConfig::FieldActuator {
            utility_cabinet: Some(cabinet),
            ..
        } = behavior
        else {
            unreachable!()
        };
        let repeated = cabinet.circuits[0].command_states[0].clone();
        cabinet.circuits[0].command_states.push(repeated);
    });

    const SENSOR: &str = "factory/process/material/forming/field/pressure-casting/mould-02/sensors/area-02-m02-pt-01.yaml";
    for value in [f64::NAN, -1.0, 1000.0] {
        invalid_behavior(SENSOR, |behavior| {
            let BehaviorConfig::FieldSensor { initial_value, .. } = behavior else {
                unreachable!()
            };
            *initial_value = Some(value);
        });
    }

    const SAFETY: &str =
        "factory/process/material/forming/field/pressure-casting/mould-01/area-02-safe-01.yaml";
    invalid_behavior(SAFETY, |behavior| {
        let BehaviorConfig::Safety { permissives, .. } = behavior else {
            unreachable!()
        };
        permissives.push(permissives[0].clone());
    });
    invalid_behavior(SAFETY, |behavior| {
        let BehaviorConfig::Safety {
            initially_permissive,
            ..
        } = behavior
        else {
            unreachable!()
        };
        initially_permissive.push(String::new());
    });
    invalid_behavior(SAFETY, |behavior| {
        let BehaviorConfig::Safety {
            initially_permissive,
            ..
        } = behavior
        else {
            unreachable!()
        };
        initially_permissive.push("unknown-permissive".into());
    });
}
