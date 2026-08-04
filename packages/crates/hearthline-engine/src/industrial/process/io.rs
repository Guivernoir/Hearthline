use hearthline_model::{
    ComponentId, ComponentKind, PortId, ProcessEvent, ServiceKind, SignalValue, Text,
};

use super::is_industrial_communication;
use super::storage::{Ports, TaggedValues, collect_ports, get, tagged_values, upsert};
use crate::runtime::{runtime_text, single_effect};
use crate::{DropReason, Effect, EffectList, ProcessEffect, SimulatedComponent, SimulationEvent};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IoDirection {
    Input,
    Output,
}

#[derive(Clone, Debug)]
pub struct RemoteIo {
    id: ComponentId,
    ports: Ports,
    channels: TaggedValues<IoDirection>,
    values: TaggedValues<SignalValue>,
    operational: bool,
}

impl RemoteIo {
    pub fn new(
        id: ComponentId,
        ports: impl IntoIterator<Item = PortId>,
        channels: impl IntoIterator<Item = (Text<64>, IoDirection)>,
    ) -> Self {
        Self {
            id,
            ports: collect_ports(ports),
            channels: tagged_values(channels),
            values: TaggedValues::new(),
            operational: true,
        }
    }

    pub fn values(&self) -> &[(Text<64>, SignalValue)] {
        self.values.as_slice()
    }
}

impl SimulatedComponent for RemoteIo {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::RemoteIo
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
                        detail: "remote I/O exchanged channel data".into(),
                    })
                } else {
                    single_effect(Effect::Drop(DropReason::PolicyDenied { rule: None }))
                }
            }
            SimulationEvent::Ipv4Egress(_) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
            SimulationEvent::Process(ProcessEvent::Signal(signal)) => {
                if get(&self.channels, &signal.tag) == Some(&IoDirection::Input) {
                    upsert(&mut self.values, signal.tag.clone(), signal.value.clone());
                    single_effect(Effect::Process(ProcessEffect::Signal(signal)))
                } else {
                    single_effect(Effect::Process(ProcessEffect::Alarm {
                        code: "RIO-INVALID-INPUT".into(),
                        active: true,
                        message: runtime_text(format_args!(
                            "input signal is not mapped: {}",
                            signal.tag
                        )),
                    }))
                }
            }
            SimulationEvent::Process(ProcessEvent::Command(command)) => {
                if get(&self.channels, &command.tag) == Some(&IoDirection::Output) {
                    upsert(&mut self.values, command.tag.clone(), command.value.clone());
                    single_effect(Effect::Process(ProcessEffect::Output {
                        tag: command.tag,
                        value: command.value,
                    }))
                } else {
                    single_effect(Effect::Process(ProcessEffect::Alarm {
                        code: "RIO-INVALID-OUTPUT".into(),
                        active: true,
                        message: runtime_text(format_args!(
                            "output channel is not mapped: {}",
                            command.tag
                        )),
                    }))
                }
            }
            SimulationEvent::Process(ProcessEvent::Tick { .. }) => EffectList::new(),
            SimulationEvent::Process(ProcessEvent::Trip { cause }) => {
                self.values.clear();
                single_effect(Effect::Process(ProcessEffect::Alarm {
                    code: "RIO-TRIP".into(),
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
                if !operational {
                    self.values.clear();
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
