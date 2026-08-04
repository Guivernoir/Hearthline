use heapless::Vec as FixedList;
use hearthline_model::{
    ComponentId, ComponentKind, NetworkPayload, PortId, ProcessEvent, ProcessSignal, ServiceKind,
    SignalValue, Text,
};

use super::is_industrial_communication;
use super::storage::{Ports, TaggedValues, collect_ports, get, upsert};
use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{DropReason, Effect, EffectList, ProcessEffect, SimulatedComponent, SimulationEvent};

#[derive(Clone, Debug, PartialEq)]
pub enum Comparison {
    BoolEquals(bool),
    AnalogGreaterThan(f64),
    AnalogLessThan(f64),
    IntegerGreaterThan(i64),
    IntegerLessThan(i64),
}

impl Comparison {
    fn matches(&self, value: &SignalValue) -> bool {
        match (self, value) {
            (Self::BoolEquals(expected), SignalValue::Bool(actual)) => expected == actual,
            (Self::AnalogGreaterThan(limit), SignalValue::Analog(actual)) => actual > limit,
            (Self::AnalogLessThan(limit), SignalValue::Analog(actual)) => actual < limit,
            (Self::IntegerGreaterThan(limit), SignalValue::Integer(actual)) => actual > limit,
            (Self::IntegerLessThan(limit), SignalValue::Integer(actual)) => actual < limit,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LogicRule {
    pub input: Text<64>,
    pub comparison: Comparison,
    pub output: Text<64>,
    pub value_when_true: SignalValue,
    pub value_when_false: SignalValue,
}

#[derive(Clone, Debug)]
pub struct VirtualPlc {
    id: ComponentId,
    ports: Ports,
    scan_period_ms: u64,
    elapsed_ms: u64,
    inputs: TaggedValues<ProcessSignal>,
    outputs: TaggedValues<SignalValue>,
    rules: FixedList<LogicRule, 16>,
    operational: bool,
}

impl VirtualPlc {
    pub fn new(
        id: ComponentId,
        ports: impl IntoIterator<Item = PortId>,
        scan_period_ms: u64,
        rules: impl IntoIterator<Item = LogicRule>,
    ) -> Self {
        assert!(scan_period_ms > 0, "scan period must be positive");
        Self {
            id,
            ports: collect_ports(ports),
            scan_period_ms,
            elapsed_ms: 0,
            inputs: TaggedValues::new(),
            outputs: TaggedValues::new(),
            rules: collect_fixed(rules),
            operational: true,
        }
    }

    pub fn outputs(&self) -> &[(Text<64>, SignalValue)] {
        self.outputs.as_slice()
    }

    fn scan(&mut self) -> EffectList {
        let mut effects = EffectList::new();
        for rule in &self.rules {
            let Some(input) = get(&self.inputs, &rule.input) else {
                push_effect(
                    &mut effects,
                    Effect::Process(ProcessEffect::Alarm {
                        code: "PLC-MISSING-INPUT".into(),
                        active: true,
                        message: runtime_text(format_args!(
                            "{} has no value for {}",
                            self.id, rule.input
                        )),
                    }),
                );
                continue;
            };
            let value = if input.quality_good && rule.comparison.matches(&input.value) {
                rule.value_when_true.clone()
            } else {
                rule.value_when_false.clone()
            };
            let changed = get(&self.outputs, &rule.output) != Some(&value);
            upsert(&mut self.outputs, rule.output.clone(), value.clone());
            if changed {
                push_effect(
                    &mut effects,
                    Effect::Process(ProcessEffect::Output {
                        tag: rule.output.clone(),
                        value,
                    }),
                );
            }
        }
        effects
    }
}

impl SimulatedComponent for VirtualPlc {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::VirtualPlc
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.ports.contains(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        if !self.operational && !matches!(event, SimulationEvent::SetOperational(_)) {
            return single_effect(Effect::Drop(DropReason::ComponentDown));
        }
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.ports.contains(&ingress.port) {
                    return single_effect(Effect::Drop(DropReason::InvalidIngress(ingress.port)));
                }
                let NetworkPayload::Ipv4(packet) = ingress.frame.payload else {
                    return single_effect(Effect::Drop(DropReason::UnsupportedProtocol));
                };
                if matches!(
                    packet.application,
                    hearthline_model::ApplicationData::Service(
                        ServiceKind::IndustrialIo | ServiceKind::PlcEngineering
                    )
                ) || matches!(packet.transport.destination_port(), Some(502 | 4840))
                {
                    single_effect(Effect::Deliver {
                        service: ServiceKind::IndustrialIo,
                        detail: runtime_text(format_args!(
                            "{} accepted modeled controller communication",
                            self.id
                        )),
                    })
                } else {
                    single_effect(Effect::Drop(DropReason::PolicyDenied { rule: None }))
                }
            }
            SimulationEvent::Ipv4Egress(_) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
            SimulationEvent::Process(ProcessEvent::Signal(signal)) => {
                upsert(&mut self.inputs, signal.tag.clone(), signal);
                EffectList::new()
            }
            SimulationEvent::Process(ProcessEvent::Tick { elapsed_ms }) => {
                self.elapsed_ms = self.elapsed_ms.saturating_add(elapsed_ms);
                let mut effects = EffectList::new();
                while self.elapsed_ms >= self.scan_period_ms {
                    self.elapsed_ms -= self.scan_period_ms;
                    for effect in self.scan() {
                        push_effect(&mut effects, effect);
                    }
                }
                effects
            }
            SimulationEvent::Process(ProcessEvent::Command(command)) => {
                single_effect(Effect::Process(ProcessEffect::Command(command)))
            }
            SimulationEvent::Process(ProcessEvent::Trip { cause }) => {
                self.outputs.clear();
                single_effect(Effect::Process(ProcessEffect::Alarm {
                    code: "PLC-EXTERNAL-TRIP".into(),
                    active: true,
                    message: Text::from(cause.as_str()),
                }))
            }
            SimulationEvent::Process(ProcessEvent::Reset { authorized }) => {
                single_effect(Effect::Process(ProcessEffect::State {
                    name: "reset-request".into(),
                    value: if authorized {
                        "accepted".into()
                    } else {
                        "denied".into()
                    },
                }))
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                if !operational {
                    self.outputs.clear();
                }
                single_effect(Effect::Observe {
                    detail: runtime_text(format_args!("operational={operational}")),
                })
            }
            SimulationEvent::FirewallHa(_) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct OperatorInterface {
    id: ComponentId,
    ports: Ports,
    allowed_command_tags: FixedList<Text<64>, 16>,
    operational: bool,
}

impl OperatorInterface {
    pub fn new(
        id: ComponentId,
        ports: impl IntoIterator<Item = PortId>,
        allowed_command_tags: impl IntoIterator<Item = Text<64>>,
    ) -> Self {
        Self {
            id,
            ports: collect_ports(ports),
            allowed_command_tags: collect_fixed(allowed_command_tags),
            operational: true,
        }
    }
}

impl SimulatedComponent for OperatorInterface {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::Hmi
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.ports.contains(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        if !self.operational && !matches!(event, SimulationEvent::SetOperational(_)) {
            return single_effect(Effect::Drop(DropReason::ComponentDown));
        }
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.ports.contains(&ingress.port) {
                    single_effect(Effect::Drop(DropReason::InvalidIngress(ingress.port)))
                } else if is_industrial_communication(&ingress.frame) {
                    single_effect(Effect::Deliver {
                        service: ServiceKind::IndustrialIo,
                        detail: "HMI received controller state".into(),
                    })
                } else {
                    single_effect(Effect::Drop(DropReason::PolicyDenied { rule: None }))
                }
            }
            SimulationEvent::Ipv4Egress(_) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
            SimulationEvent::Process(ProcessEvent::Command(command)) => {
                if self.allowed_command_tags.contains(&command.tag) {
                    single_effect(Effect::Process(ProcessEffect::Command(command)))
                } else {
                    single_effect(Effect::Process(ProcessEffect::Alarm {
                        code: "HMI-COMMAND-DENIED".into(),
                        active: true,
                        message: runtime_text(format_args!(
                            "command tag is not allowed: {}",
                            command.tag
                        )),
                    }))
                }
            }
            SimulationEvent::Process(ProcessEvent::Signal(signal)) => {
                single_effect(Effect::Process(ProcessEffect::Signal(signal)))
            }
            SimulationEvent::Process(ProcessEvent::Tick { .. }) => EffectList::new(),
            SimulationEvent::Process(ProcessEvent::Trip { cause }) => {
                single_effect(Effect::Process(ProcessEffect::Alarm {
                    code: "HMI-TRIP-DISPLAY".into(),
                    active: true,
                    message: Text::from(cause.as_str()),
                }))
            }
            SimulationEvent::Process(ProcessEvent::Reset { authorized }) => {
                single_effect(Effect::Process(ProcessEffect::State {
                    name: "reset-request".into(),
                    value: if authorized {
                        "authorized=true".into()
                    } else {
                        "authorized=false".into()
                    },
                }))
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                single_effect(Effect::Observe {
                    detail: runtime_text(format_args!("operational={operational}")),
                })
            }
            SimulationEvent::FirewallHa(_) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
        }
    }
}

fn push_effect(effects: &mut EffectList, effect: Effect) {
    effects
        .push(effect)
        .expect("industrial effects exceed fixed capacity");
}
