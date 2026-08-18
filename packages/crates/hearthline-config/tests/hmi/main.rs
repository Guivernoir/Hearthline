use std::path::PathBuf;

mod body_preparation;
mod control_program;
mod forming_authority;
mod forming_process;
mod guarded_cell;
mod robot_program;

use hearthline_config::{
    ConfigRepository, ConnectionRepository, HmiAction, HmiActionStatus, HmiControlMode,
    HmiPreparationTrain, HmiProcessFault, HmiRobotPose, HmiSession, HmiSessionStore,
    ScenarioApplicationConfig, ScenarioRepository, build_forming_telemetry_packet,
    run_scenario_with_state_overrides,
};
use hearthline_engine::PUMP_HEARTBEAT_TIMEOUT_MS;

fn repository() -> ConfigRepository {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config/appliances");
    ConfigRepository::load(root).expect("appliances")
}

#[test]
fn body_preparation_hmi_is_built_from_area_configuration() {
    let appliances = repository();
    let session = HmiSession::from_repository(&appliances, "area-01-hmi-01").expect("HMI session");
    let snapshot = session.snapshot();

    assert_eq!(snapshot.environment, "Body Preparation");
    assert_eq!(snapshot.zone, "OT-AREA-01");
    assert_eq!(snapshot.controller, "area-01-vplc-01");
    assert!(
        snapshot
            .remote_io_stations
            .contains(&"area-01-rio-01".into())
    );
    assert_eq!(snapshot.remote_io_stations.len(), 2);
    assert_eq!(snapshot.signals.len(), 20);
    assert_eq!(snapshot.actuators.len(), 13);
    assert_eq!(snapshot.safety.len(), 1);
    assert_eq!(snapshot.parameters.len(), 11);
    assert_eq!(snapshot.recipes.len(), 1);
    let batch = snapshot.body_preparation.expect("batch process state");
    assert_eq!(batch.slip.ingredients.len(), 6);
    assert_eq!(batch.glaze.ingredients.len(), 9);
    assert!((batch.slip.target_batch_mass_kg - 1_335.3).abs() < 0.1);
    assert_eq!(batch.simulated_ms_per_process_minute, 50);
    assert!(snapshot.safety[0].trip_latched);
    assert!(
        snapshot.safety[0]
            .permissives
            .iter()
            .all(|permissive| permissive.satisfied)
    );
    assert_eq!(snapshot.alarms[0].code, "SAFETY-RESET-REQUIRED");
}

#[test]
fn safety_reset_then_command_traverses_the_configured_control_path() {
    let appliances = repository();
    let mut session =
        HmiSession::from_repository(&appliances, "area-01-hmi-01").expect("HMI session");

    let inhibited = session.execute(HmiAction::Command {
        tag: "area-01-pmp-01-command".into(),
        value: "transferring".into(),
    });
    assert!(matches!(inhibited.status, HmiActionStatus::Denied));
    assert_eq!(
        inhibited
            .snapshot
            .actuators
            .iter()
            .find(|actuator| actuator.command_tag == "area-01-pmp-01-command")
            .expect("pump")
            .current_state,
        "stopped"
    );

    let reset = session.execute(HmiAction::ResetSafety {
        safety_id: "area-01-intlk-01".into(),
    });
    assert!(matches!(reset.status, HmiActionStatus::Applied));
    assert!(!reset.snapshot.safety[0].trip_latched);
    assert_eq!(reset.trace.len(), 2);

    let command = session.execute(HmiAction::Command {
        tag: "area-01-pmp-01-command".into(),
        value: "transferring".into(),
    });
    assert!(matches!(command.status, HmiActionStatus::Applied));
    assert_eq!(command.trace.len(), 4);
    assert_eq!(command.trace[0].component, "area-01-hmi-01");
    assert_eq!(command.trace[1].component, "area-01-vplc-01");
    assert_eq!(command.trace[2].component, "area-01-rio-02");
    assert_eq!(command.trace[3].component, "area-01-pmp-01");
    assert_eq!(
        command
            .snapshot
            .actuators
            .iter()
            .find(|actuator| actuator.command_tag == "area-01-pmp-01-command")
            .expect("pump")
            .current_state,
        "transferring"
    );
}

#[test]
fn alarms_can_be_acknowledged_and_undeclared_commands_are_denied() {
    let appliances = repository();
    let mut session =
        HmiSession::from_repository(&appliances, "area-01-hmi-01").expect("HMI session");

    let acknowledgement = session.execute(HmiAction::AcknowledgeAlarm {
        alarm_id: "startup-area-01-intlk-01".into(),
    });
    assert!(matches!(acknowledgement.status, HmiActionStatus::Completed));
    assert!(acknowledgement.snapshot.alarms[0].acknowledged);

    session.execute(HmiAction::ResetSafety {
        safety_id: "area-01-intlk-01".into(),
    });
    let denied = session.execute(HmiAction::Command {
        tag: "area-01-unknown-command".into(),
        value: "running".into(),
    });
    assert!(matches!(denied.status, HmiActionStatus::Denied));
    assert!(
        denied
            .snapshot
            .alarms
            .iter()
            .any(|alarm| alarm.code == "HMI-COMMAND-DENIED")
    );
}

#[test]
fn every_process_hmi_executes_each_configured_field_state() {
    const HMI_IDS: [&str; 15] = [
        "area-01-hmi-01",
        "area-02-hmi-01",
        "area-02-hmi-02",
        "area-02-hmi-03",
        "area-02-hmi-04",
        "area-02-joystick-01",
        "area-02-machine-pc-01",
        "area-03-hmi-01",
        "area-04-hmi-01",
        "area-05-hmi-01",
        "area-06-hmi-01",
        "area-07-hmi-01",
        "area-08-hmi-01",
        "area-09-hmi-01",
        "area-10-hmi-01",
    ];

    let appliances = repository();
    for hmi_id in HMI_IDS {
        let mut session =
            HmiSession::from_repository(&appliances, hmi_id).expect("interactive HMI");
        let initial = session.snapshot();

        assert_eq!(initial.id, hmi_id);
        assert!(!initial.signals.is_empty(), "{hmi_id} signals");
        assert!(!initial.actuators.is_empty(), "{hmi_id} actuators");
        assert!(!initial.safety.is_empty(), "{hmi_id} safety interfaces");
        assert!(
            initial
                .safety
                .iter()
                .flat_map(|safety| &safety.permissives)
                .all(|permissive| permissive.satisfied),
            "{hmi_id} startup permissives"
        );

        if initial.safety.iter().any(|safety| safety.trip_latched) {
            let first_actuator = &initial.actuators[0];
            let denied = session.execute(HmiAction::Command {
                tag: first_actuator.command_tag.clone(),
                value: first_actuator.states[0].clone(),
            });
            assert!(
                matches!(denied.status, HmiActionStatus::Denied),
                "{hmi_id} must inhibit commands before reset"
            );

            for safety_id in initial
                .safety
                .iter()
                .filter(|safety| safety.trip_latched)
                .map(|safety| safety.component_id.clone())
            {
                let reset = session.execute(HmiAction::ResetSafety { safety_id });
                assert!(
                    matches!(reset.status, HmiActionStatus::Applied),
                    "{hmi_id} safety reset"
                );
            }
        }

        if initial
            .control_station
            .as_ref()
            .is_some_and(|station| !station.positions.is_empty())
        {
            let mode = session.execute(HmiAction::SetControlMode {
                mode: HmiControlMode::Manual,
                password: None,
            });
            assert!(
                matches!(mode.status, HmiActionStatus::Applied),
                "{hmi_id} manual selector"
            );

            if initial
                .control_station
                .as_ref()
                .is_some_and(|station| station.station_type == "robot-joystick")
            {
                let enabled = session.execute(HmiAction::SetRobotMotionEnable { enabled: true });
                assert!(
                    matches!(enabled.status, HmiActionStatus::Applied),
                    "{hmi_id} robot motion enable"
                );
            }
        }

        for actuator in &initial.actuators {
            if hmi_id == "area-02-machine-pc-01" {
                continue;
            }
            assert!(
                actuator.states.len() >= 2,
                "{} state domain",
                actuator.component_id
            );
            for state in &actuator.states {
                let command = session.execute(HmiAction::Command {
                    tag: actuator.command_tag.clone(),
                    value: state.clone(),
                });
                assert!(
                    matches!(command.status, HmiActionStatus::Applied),
                    "{} state {state}",
                    actuator.component_id
                );
                assert_eq!(
                    command
                        .snapshot
                        .actuators
                        .iter()
                        .find(|candidate| candidate.component_id == actuator.component_id)
                        .expect("commanded actuator")
                        .current_state,
                    *state
                );
                let path = command
                    .trace
                    .iter()
                    .map(|entry| entry.component.as_str())
                    .collect::<Vec<_>>();
                assert_eq!(path[0], hmi_id, "{} HMI path", actuator.component_id);
                assert_eq!(
                    path[1], initial.controller,
                    "{} controller path",
                    actuator.component_id
                );
                assert!(
                    initial.remote_io_stations.iter().any(|id| id == path[2]),
                    "{} remote-I/O path",
                    actuator.component_id
                );
                assert_eq!(
                    path[3], actuator.component_id,
                    "{} field path",
                    actuator.component_id
                );
            }
        }
    }
}
