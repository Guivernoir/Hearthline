use heapless::Vec as FixedList;
use hearthline_model::{ComponentId, ComponentKind, PortId};

use crate::capacity::UNADDRESSED_PORT_CAPACITY;
use crate::runtime::{collect_fixed, single_effect};
use crate::{DropReason, Effect, EffectList, SimulatedComponent, SimulationEvent};

/// A physically modeled endpoint whose logical addressing is not configured yet.
#[derive(Clone, Debug)]
pub struct UnaddressedNode {
    id: ComponentId,
    kind: ComponentKind,
    ports: FixedList<PortId, UNADDRESSED_PORT_CAPACITY>,
    operational: bool,
}

impl UnaddressedNode {
    pub fn new(
        id: ComponentId,
        kind: ComponentKind,
        ports: impl IntoIterator<Item = PortId>,
    ) -> Self {
        let ports = collect_fixed(ports);
        assert!(
            !ports.is_empty(),
            "unaddressed endpoint requires at least one physical port"
        );
        Self {
            id,
            kind,
            ports,
            operational: true,
        }
    }
}

impl SimulatedComponent for UnaddressedNode {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        self.kind
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.ports.contains(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                EffectList::new()
            }
            SimulationEvent::Network(ingress) => {
                if !self.operational {
                    return single_effect(Effect::Drop(DropReason::ComponentDown));
                }
                if !self.has_port(&ingress.port) {
                    return single_effect(Effect::Drop(DropReason::InvalidIngress(ingress.port)));
                }
                single_effect(Effect::Drop(DropReason::NoInterfaceAddress(ingress.port)))
            }
            SimulationEvent::Ipv4Egress(egress) => {
                single_effect(Effect::Drop(DropReason::NoRoute(egress.packet.destination)))
            }
            SimulationEvent::Process(_) | SimulationEvent::FirewallHa(_) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
        }
    }
}
