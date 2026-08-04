use std::path::PathBuf;

use hearthline_config::{ConfigRepository, HmiAction, HmiActionStatus, HmiSession};

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
    assert_eq!(snapshot.remote_io, "area-01-rio-01");
    assert_eq!(snapshot.signals.len(), 2);
    assert_eq!(snapshot.actuators.len(), 2);
    assert_eq!(snapshot.safety.len(), 1);
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
        value: "running".into(),
    });
    assert!(matches!(inhibited.status, HmiActionStatus::Denied));
    assert_eq!(inhibited.snapshot.actuators[0].current_state, "stopped");

    let reset = session.execute(HmiAction::ResetSafety {
        safety_id: "area-01-intlk-01".into(),
    });
    assert!(matches!(reset.status, HmiActionStatus::Applied));
    assert!(!reset.snapshot.safety[0].trip_latched);
    assert_eq!(reset.trace.len(), 2);

    let command = session.execute(HmiAction::Command {
        tag: "area-01-pmp-01-command".into(),
        value: "running".into(),
    });
    assert!(matches!(command.status, HmiActionStatus::Applied));
    assert_eq!(command.trace.len(), 4);
    assert_eq!(command.trace[0].component, "area-01-hmi-01");
    assert_eq!(command.trace[1].component, "area-01-vplc-01");
    assert_eq!(command.trace[2].component, "area-01-rio-01");
    assert_eq!(command.trace[3].component, "area-01-pmp-01");
    assert_eq!(
        command
            .snapshot
            .actuators
            .iter()
            .find(|actuator| actuator.command_tag == "area-01-pmp-01-command")
            .expect("pump")
            .current_state,
        "running"
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
    const HMI_IDS: [&str; 10] = [
        "area-01-hmi-01",
        "area-02-hmi-01",
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
        assert_eq!(initial.signals.len(), 2, "{hmi_id} signals");
        assert_eq!(initial.actuators.len(), 2, "{hmi_id} actuators");
        assert_eq!(initial.safety.len(), 1, "{hmi_id} safety interfaces");
        assert!(initial.safety[0].trip_latched, "{hmi_id} startup trip");
        assert!(
            initial.safety[0]
                .permissives
                .iter()
                .all(|permissive| permissive.satisfied),
            "{hmi_id} startup permissives"
        );

        let first_actuator = &initial.actuators[0];
        let denied = session.execute(HmiAction::Command {
            tag: first_actuator.command_tag.clone(),
            value: first_actuator.states[0].clone(),
        });
        assert!(
            matches!(denied.status, HmiActionStatus::Denied),
            "{hmi_id} must inhibit commands before reset"
        );

        let safety_id = initial.safety[0].component_id.clone();
        let reset = session.execute(HmiAction::ResetSafety { safety_id });
        assert!(
            matches!(reset.status, HmiActionStatus::Applied),
            "{hmi_id} safety reset"
        );

        for actuator in &initial.actuators {
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
                assert_eq!(
                    command
                        .trace
                        .iter()
                        .map(|entry| entry.component.as_str())
                        .collect::<Vec<_>>(),
                    [
                        hmi_id,
                        initial.controller.as_str(),
                        initial.remote_io.as_str(),
                        actuator.component_id.as_str(),
                    ],
                    "{} command path",
                    actuator.component_id
                );
            }
        }
    }
}
