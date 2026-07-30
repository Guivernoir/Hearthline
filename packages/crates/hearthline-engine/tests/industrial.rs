use hearthline_engine::{
    Comparison, DropReason, Effect, LogicRule, SafetyInterface, SimulatedComponent,
    SimulationEvent, VirtualPlc,
};
use hearthline_model::{ComponentId, ProcessEvent, ProcessSignal, SignalValue};

fn id(value: &str) -> ComponentId {
    ComponentId::new(value).expect("test ID")
}

fn signal(tag: &str, value: SignalValue) -> ProcessSignal {
    ProcessSignal {
        tag: tag.into(),
        value,
        quality_good: true,
        timestamp_ms: 0,
    }
}

#[test]
fn virtual_plc_scans_on_period_and_updates_output() {
    let mut plc = VirtualPlc::new(
        id("area-01-vplc-01"),
        [],
        100,
        [LogicRule {
            input: "level-high".into(),
            comparison: Comparison::BoolEquals(true),
            output: "pump-run".into(),
            value_when_true: SignalValue::Bool(false),
            value_when_false: SignalValue::Bool(true),
        }],
    );
    plc.handle(SimulationEvent::Process(ProcessEvent::Signal(signal(
        "level-high",
        SignalValue::Bool(false),
    ))));
    assert!(
        plc.handle(SimulationEvent::Process(ProcessEvent::Tick {
            elapsed_ms: 99
        }))
        .is_empty()
    );
    let effects = plc.handle(SimulationEvent::Process(ProcessEvent::Tick {
        elapsed_ms: 1,
    }));
    assert_eq!(effects.len(), 1);
    assert_eq!(
        plc.outputs()
            .iter()
            .find(|(tag, _)| tag.as_str() == "pump-run")
            .map(|(_, value)| value),
        Some(&SignalValue::Bool(true))
    );
}

#[test]
fn safety_reset_requires_authorization_and_all_permissives() {
    let mut safety = SafetyInterface::new(
        id("area-06-bms-01"),
        ["airflow-ok".into(), "gas-pressure-ok".into()],
    );
    safety.handle(SimulationEvent::Process(ProcessEvent::Signal(signal(
        "airflow-ok",
        SignalValue::Bool(true),
    ))));
    safety.handle(SimulationEvent::Process(ProcessEvent::Signal(signal(
        "gas-pressure-ok",
        SignalValue::Bool(true),
    ))));
    let denied = safety.handle(SimulationEvent::Process(ProcessEvent::Reset {
        authorized: false,
    }));
    assert!(matches!(denied[0], Effect::Drop(DropReason::SafetyTrip(_))));
    safety.handle(SimulationEvent::Process(ProcessEvent::Reset {
        authorized: true,
    }));
    assert!(!safety.trip_latched());
}
