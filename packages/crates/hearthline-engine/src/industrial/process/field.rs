use hearthline_model::{
    ComponentId, ComponentKind, PortId, ProcessEvent, ProcessSignal, SignalValue, Text,
};

use super::storage::{Ports, collect_ports};
use crate::runtime::{runtime_text, single_effect};
use crate::{DropReason, Effect, EffectList, ProcessEffect, SimulatedComponent, SimulationEvent};

#[derive(Clone, Debug)]
pub struct FieldSensor {
    id: ComponentId,
    ports: Ports,
    tag: Text<64>,
    raw_value: f64,
    gain: f64,
    offset: f64,
    quality_good: bool,
    sample_period_ms: u64,
    elapsed_ms: u64,
    timestamp_ms: u64,
    operational: bool,
}

impl FieldSensor {
    pub fn new(
        id: ComponentId,
        tag: Text<64>,
        sample_period_ms: u64,
        gain: f64,
        offset: f64,
    ) -> Self {
        Self::with_ports(id, [], tag, sample_period_ms, gain, offset)
    }

    pub fn with_ports(
        id: ComponentId,
        ports: impl IntoIterator<Item = PortId>,
        tag: Text<64>,
        sample_period_ms: u64,
        gain: f64,
        offset: f64,
    ) -> Self {
        assert!(sample_period_ms > 0, "sample period must be positive");
        Self {
            id,
            ports: collect_ports(ports),
            tag,
            raw_value: 0.0,
            gain,
            offset,
            quality_good: true,
            sample_period_ms,
            elapsed_ms: 0,
            timestamp_ms: 0,
            operational: true,
        }
    }

    pub fn set_raw_value(&mut self, value: f64) {
        self.raw_value = value;
    }
}

impl SimulatedComponent for FieldSensor {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::FieldSensor
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.ports.contains(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::Process(ProcessEvent::Tick { elapsed_ms }) => {
                self.elapsed_ms = self.elapsed_ms.saturating_add(elapsed_ms);
                self.timestamp_ms = self.timestamp_ms.saturating_add(elapsed_ms);
                if self.elapsed_ms < self.sample_period_ms {
                    return EffectList::new();
                }
                self.elapsed_ms %= self.sample_period_ms;
                single_effect(Effect::Process(ProcessEffect::Signal(ProcessSignal {
                    tag: self.tag.clone(),
                    value: SignalValue::Analog(self.raw_value * self.gain + self.offset),
                    quality_good: self.operational && self.quality_good,
                    timestamp_ms: self.timestamp_ms,
                })))
            }
            SimulationEvent::Process(ProcessEvent::Command(command)) if command.tag == self.tag => {
                match command.value {
                    SignalValue::Analog(value) => {
                        self.raw_value = value;
                        EffectList::new()
                    }
                    _ => single_effect(Effect::Process(ProcessEffect::Alarm {
                        code: "SENSOR-TYPE-MISMATCH".into(),
                        active: true,
                        message: runtime_text(format_args!(
                            "{} requires an analog value",
                            self.tag
                        )),
                    })),
                }
            }
            SimulationEvent::Process(ProcessEvent::Trip { cause }) => {
                self.quality_good = false;
                single_effect(Effect::Process(ProcessEffect::Alarm {
                    code: "SENSOR-BAD-QUALITY".into(),
                    active: true,
                    message: Text::from(cause.as_str()),
                }))
            }
            SimulationEvent::Process(ProcessEvent::Reset { authorized }) => {
                if authorized {
                    self.quality_good = true;
                }
                EffectList::new()
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                EffectList::new()
            }
            SimulationEvent::Network(_)
            | SimulationEvent::Ipv4Egress(_)
            | SimulationEvent::FirewallHa(_)
            | SimulationEvent::Process(ProcessEvent::Signal(_))
            | SimulationEvent::Process(ProcessEvent::Command(_)) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Actuator {
    id: ComponentId,
    ports: Ports,
    tag: Text<64>,
    actual: SignalValue,
    safe_value: SignalValue,
    failed: bool,
    operational: bool,
}

impl Actuator {
    pub fn new(
        id: ComponentId,
        tag: Text<64>,
        initial: SignalValue,
        safe_value: SignalValue,
    ) -> Self {
        Self::with_ports(id, [], tag, initial, safe_value)
    }

    pub fn with_ports(
        id: ComponentId,
        ports: impl IntoIterator<Item = PortId>,
        tag: Text<64>,
        initial: SignalValue,
        safe_value: SignalValue,
    ) -> Self {
        Self {
            id,
            ports: collect_ports(ports),
            tag,
            actual: initial,
            safe_value,
            failed: false,
            operational: true,
        }
    }

    pub fn value(&self) -> &SignalValue {
        &self.actual
    }

    pub fn set_failed(&mut self, failed: bool) {
        self.failed = failed;
        if failed {
            self.actual = self.safe_value.clone();
        }
    }
}

impl SimulatedComponent for Actuator {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::FieldActuator
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.ports.contains(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::Process(ProcessEvent::Command(command)) if command.tag == self.tag => {
                if !self.operational || self.failed {
                    return single_effect(Effect::Drop(DropReason::ComponentDown));
                }
                self.actual = command.value;
                single_effect(Effect::Process(ProcessEffect::Output {
                    tag: self.tag.clone(),
                    value: self.actual.clone(),
                }))
            }
            SimulationEvent::Process(ProcessEvent::Trip { cause }) => {
                self.actual = self.safe_value.clone();
                let mut effects = EffectList::new();
                effects
                    .push(Effect::Drop(DropReason::SafetyTrip(Text::from(
                        cause.as_str(),
                    ))))
                    .expect("actuator trip effect must fit");
                effects
                    .push(Effect::Process(ProcessEffect::Output {
                        tag: self.tag.clone(),
                        value: self.actual.clone(),
                    }))
                    .expect("actuator output effect must fit");
                effects
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                if !operational {
                    self.actual = self.safe_value.clone();
                }
                single_effect(Effect::Process(ProcessEffect::Output {
                    tag: self.tag.clone(),
                    value: self.actual.clone(),
                }))
            }
            SimulationEvent::Process(ProcessEvent::Tick { .. })
            | SimulationEvent::Process(ProcessEvent::Signal(_))
            | SimulationEvent::Process(ProcessEvent::Reset { .. }) => EffectList::new(),
            SimulationEvent::Network(_)
            | SimulationEvent::Ipv4Egress(_)
            | SimulationEvent::FirewallHa(_)
            | SimulationEvent::Process(ProcessEvent::Command(_)) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
        }
    }
}
