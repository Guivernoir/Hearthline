use heapless::Vec as FixedList;
use hearthline_model::{ComponentId, ComponentKind, PortId};

use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{DropReason, Effect, EffectList, SimulatedComponent, SimulationEvent};

#[derive(Clone, Debug)]
pub struct PassiveSensor {
    id: ComponentId,
    ports: FixedList<PortId, 16>,
    operational: bool,
    observations: u64,
}

impl PassiveSensor {
    pub fn new(id: ComponentId, ports: impl IntoIterator<Item = PortId>) -> Self {
        Self {
            id,
            ports: collect_fixed(ports),
            operational: true,
            observations: 0,
        }
    }

    pub const fn observation_count(&self) -> u64 {
        self.observations
    }
}

impl SimulatedComponent for PassiveSensor {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::PassiveNetworkSensor
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.ports.contains(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.operational {
                    return single_effect(Effect::Drop(DropReason::ComponentDown));
                }
                if !self.ports.contains(&ingress.port) {
                    return single_effect(Effect::Drop(DropReason::InvalidIngress(ingress.port)));
                }
                if !ingress.frame.has_valid_wire_length() {
                    return single_effect(Effect::Drop(DropReason::InvalidEthernetFrame));
                }
                self.observations += 1;
                single_effect(Effect::Observe {
                    detail: runtime_text(format_args!(
                        "passively observed frame {}",
                        self.observations
                    )),
                })
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                single_effect(Effect::Observe {
                    detail: runtime_text(format_args!("operational={operational}")),
                })
            }
            SimulationEvent::Ipv4Egress(_)
            | SimulationEvent::Process(_)
            | SimulationEvent::FirewallHa(_) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
        }
    }
}
