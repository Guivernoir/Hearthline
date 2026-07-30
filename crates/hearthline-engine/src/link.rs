use std::collections::BTreeSet;

use hearthline_model::{ComponentId, ComponentKind, PortId};

use crate::{DropReason, Effect, SimulatedComponent, SimulationEvent};

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
    ports: BTreeSet<PortId>,
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
            ports: ports.into_iter().collect(),
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

    fn handle(&mut self, event: SimulationEvent) -> Vec<Effect> {
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.operational {
                    return vec![Effect::Drop(DropReason::ComponentDown)];
                }
                if !self.ports.contains(&ingress.port) {
                    return vec![Effect::Drop(DropReason::InvalidIngress(ingress.port))];
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
                    return vec![Effect::Drop(DropReason::LinkLoss)];
                }
                let mut effects = Vec::new();
                if self.mode == LinkMode::Encrypted {
                    effects.push(Effect::Observe {
                        detail: "frame traversed the modeled encrypted conduit".into(),
                    });
                }
                effects.extend(
                    self.ports
                        .iter()
                        .filter(|port| **port != ingress.port)
                        .cloned()
                        .map(|egress| Effect::Transmit {
                            egress,
                            next_hop: None,
                            frame: ingress.frame.clone(),
                            delay_ms,
                        }),
                );
                effects
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                vec![Effect::Observe {
                    detail: format!("operational={operational}"),
                }]
            }
            SimulationEvent::Process(_) => vec![Effect::Drop(DropReason::UnsupportedProtocol)],
        }
    }
}
