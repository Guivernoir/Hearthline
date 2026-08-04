use hearthline_engine::{Effect, EffectList, ProcessEffect};
use hearthline_model::{ComponentId, PortId, SignalValue};

use super::HmiTraceEntry;

pub(super) fn component_id(value: &str) -> ComponentId {
    ComponentId::new(value).expect("validated HMI component id")
}

pub(super) fn ports(values: &[String]) -> impl Iterator<Item = PortId> + '_ {
    values
        .iter()
        .map(|value| PortId::new(value).expect("validated HMI port id"))
}

pub(super) fn forwards_command(effects: EffectList) -> bool {
    effects
        .iter()
        .any(|effect| matches!(effect, Effect::Process(ProcessEffect::Command(_))))
}

pub(super) fn produces_output(effects: EffectList) -> bool {
    effects
        .iter()
        .any(|effect| matches!(effect, Effect::Process(ProcessEffect::Output { .. })))
}

pub(super) fn produces_true_output(effects: EffectList) -> bool {
    effects.iter().any(|effect| {
        matches!(
            effect,
            Effect::Process(ProcessEffect::Output {
                value: SignalValue::Bool(true),
                ..
            })
        )
    })
}

pub(super) fn signal_value_text(value: &SignalValue) -> String {
    match value {
        SignalValue::Text(value) => value.to_string(),
        SignalValue::Bool(value) => value.to_string(),
        SignalValue::Analog(value) => value.to_string(),
        SignalValue::Integer(value) => value.to_string(),
    }
}

pub(super) fn trace_entry(component: &str, stage: &str, detail: String) -> HmiTraceEntry {
    HmiTraceEntry {
        sequence: 0,
        component: component.into(),
        stage: stage.into(),
        detail,
    }
}
