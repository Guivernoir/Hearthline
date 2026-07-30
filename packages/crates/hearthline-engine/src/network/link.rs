use heapless::Vec as FixedList;

use hearthline_model::{ComponentId, ComponentKind, PortId};

use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{DropReason, Effect, EffectList, SimulatedComponent, SimulationEvent};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkMode {
    Transparent,
    Encrypted,
    Wan {
        delay_ms: u64,
        drop_every: Option<u64>,
    },
}

#[derive(Clone, Debug)]
pub struct LinkAppliance {
    id: ComponentId,
    kind: ComponentKind,
    ports: FixedList<PortId, 16>,
    mode: LinkMode,
    operational: bool,
    frame_count: u64,
}

impl LinkAppliance {
    pub fn new(
        id: ComponentId,
        kind: ComponentKind,
        ports: impl IntoIterator<Item = PortId>,
        mode: LinkMode,
    ) -> Self {
        assert!(
            matches!(
                kind,
                ComponentKind::TransparentCpe
                    | ComponentKind::WanCircuit
                    | ComponentKind::EncryptedConduit
            ),
            "link appliance kind does not use link behavior"
        );
        Self {
            id,
            kind,
            ports: collect_fixed(ports),
            mode,
            operational: true,
            frame_count: 0,
        }
    }
}

impl SimulatedComponent for LinkAppliance {
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
            SimulationEvent::Network(ingress) => {
                if !self.operational {
                    return single_effect(Effect::Drop(DropReason::ComponentDown));
                }
                if !self.ports.contains(&ingress.port) {
                    return single_effect(Effect::Drop(DropReason::InvalidIngress(ingress.port)));
                }
                self.frame_count += 1;
                let (delay_ms, should_drop) = match self.mode {
                    LinkMode::Transparent | LinkMode::Encrypted => (0, false),
                    LinkMode::Wan {
                        delay_ms,
                        drop_every,
                    } => (
                        delay_ms,
                        drop_every
                            .is_some_and(|interval| self.frame_count.is_multiple_of(interval)),
                    ),
                };
                if should_drop {
                    return single_effect(Effect::Drop(DropReason::LinkLoss));
                }
                let mut effects = EffectList::new();
                if self.mode == LinkMode::Encrypted {
                    effects
                        .push(Effect::Observe {
                            detail: "frame traversed the modeled encrypted conduit".into(),
                        })
                        .expect("encrypted observation must fit effect capacity");
                }
                for egress in self
                    .ports
                    .iter()
                    .filter(|port| **port != ingress.port)
                    .cloned()
                {
                    effects
                        .push(Effect::Transmit {
                            egress,
                            next_hop: None,
                            frame: ingress.frame.clone(),
                            delay_ms,
                        })
                        .expect("link fan-out exceeds effect capacity");
                }
                effects
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                single_effect(Effect::Observe {
                    detail: runtime_text(format_args!("operational={operational}")),
                })
            }
            SimulationEvent::Process(_) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
        }
    }
}
