use heapless::Vec as FixedList;
use hearthline_model::{ComponentId, ComponentKind, PortId, ProcessEvent, SignalValue, Text};

use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{DropReason, Effect, EffectList, ProcessEffect, SimulatedComponent, SimulationEvent};

#[derive(Clone, Debug)]
pub struct SafetyInterface {
    id: ComponentId,
    required_permissives: FixedList<Text<64>, 16>,
    permissives: FixedList<(Text<64>, bool), 16>,
    trip_latched: bool,
    trip_cause: Option<Text<96>>,
    operational: bool,
}

impl SafetyInterface {
    pub fn new(id: ComponentId, required_permissives: impl IntoIterator<Item = Text<64>>) -> Self {
        let required_permissives = collect_fixed(required_permissives);
        let mut permissives = FixedList::new();
        for tag in &required_permissives {
            permissives
                .push((tag.clone(), false))
                .expect("safety permissive table exceeds capacity");
        }
        Self {
            id,
            required_permissives,
            permissives,
            trip_latched: true,
            trip_cause: Some("permissives not established".into()),
            operational: true,
        }
    }

    pub const fn trip_latched(&self) -> bool {
        self.trip_latched
    }

    fn all_permissive(&self) -> bool {
        self.required_permissives.iter().all(|tag| {
            self.permissives
                .iter()
                .find(|(candidate, _)| candidate == tag)
                .is_some_and(|(_, value)| *value)
        })
    }
}

impl SimulatedComponent for SafetyInterface {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::SafetyInterface
    }

    fn has_port(&self, _port: &PortId) -> bool {
        false
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::Process(ProcessEvent::Signal(signal)) => {
                if self.required_permissives.contains(&signal.tag) {
                    let value =
                        matches!(signal.value, SignalValue::Bool(true)) && signal.quality_good;
                    if let Some((_, current)) = self
                        .permissives
                        .iter_mut()
                        .find(|(candidate, _)| *candidate == signal.tag)
                    {
                        *current = value;
                    }
                    if !value {
                        self.trip_latched = true;
                        self.trip_cause = Some(runtime_text(format_args!(
                            "permissive lost: {}",
                            signal.tag
                        )));
                    }
                }
                single_effect(Effect::Process(ProcessEffect::Output {
                    tag: "safety-permissive".into(),
                    value: SignalValue::Bool(
                        self.operational && !self.trip_latched && self.all_permissive(),
                    ),
                }))
            }
            SimulationEvent::Process(ProcessEvent::Trip { cause }) => {
                self.trip_latched = true;
                let cause = Text::from(cause.as_str());
                self.trip_cause = Some(cause.clone());
                single_effect(Effect::Drop(DropReason::SafetyTrip(cause)))
            }
            SimulationEvent::Process(ProcessEvent::Reset { authorized }) => {
                if authorized && self.operational && self.all_permissive() {
                    self.trip_latched = false;
                    self.trip_cause = None;
                    single_effect(Effect::Process(ProcessEffect::Output {
                        tag: "safety-permissive".into(),
                        value: SignalValue::Bool(true),
                    }))
                } else {
                    single_effect(Effect::Drop(DropReason::SafetyTrip(
                        self.trip_cause
                            .clone()
                            .unwrap_or_else(|| "reset conditions not satisfied".into()),
                    )))
                }
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                if !operational {
                    self.trip_latched = true;
                    self.trip_cause = Some("safety interface unavailable".into());
                }
                EffectList::new()
            }
            SimulationEvent::Process(ProcessEvent::Tick { .. })
            | SimulationEvent::Process(ProcessEvent::Command(_)) => EffectList::new(),
            SimulationEvent::Network(_) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
        }
    }
}
