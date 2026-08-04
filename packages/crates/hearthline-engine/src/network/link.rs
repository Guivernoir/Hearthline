use heapless::Vec as FixedList;

use hearthline_model::{ComponentId, ComponentKind, PortId};

use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{DropReason, Effect, EffectList, SimulatedComponent, SimulationEvent};

#[derive(Clone, Debug)]
struct LinkPort {
    id: PortId,
    forwarding: bool,
}

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
    ports: FixedList<LinkPort, 16>,
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
        Self::build(id, kind, ports, mode)
    }

    pub fn embedded_virtual_switch(
        id: ComponentId,
        ports: impl IntoIterator<Item = PortId>,
    ) -> Self {
        Self::build(
            id,
            ComponentKind::VirtualizationHost,
            ports,
            LinkMode::Transparent,
        )
    }

    fn build(
        id: ComponentId,
        kind: ComponentKind,
        ports: impl IntoIterator<Item = PortId>,
        mode: LinkMode,
    ) -> Self {
        let ports: FixedList<LinkPort, 16> = collect_fixed(ports.into_iter().map(|id| LinkPort {
            id,
            forwarding: true,
        }));
        assert!(
            ports.len() >= 2,
            "link appliance requires at least two ports"
        );
        Self {
            id,
            kind,
            ports,
            mode,
            operational: true,
            frame_count: 0,
        }
    }

    pub fn set_port_forwarding(&mut self, port: &PortId, forwarding: bool) -> bool {
        let Some(configured) = self
            .ports
            .iter_mut()
            .find(|candidate| candidate.id == *port)
        else {
            return false;
        };
        configured.forwarding = forwarding;
        true
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
        self.ports.iter().any(|candidate| candidate.id == *port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.operational {
                    return single_effect(Effect::Drop(DropReason::ComponentDown));
                }
                let Some(ingress_port) = self
                    .ports
                    .iter()
                    .find(|candidate| candidate.id == ingress.port)
                else {
                    return single_effect(Effect::Drop(DropReason::InvalidIngress(ingress.port)));
                };
                if !ingress_port.forwarding {
                    return single_effect(Effect::Drop(DropReason::PortDown(ingress.port)));
                }
                if !ingress.frame.has_valid_wire_length() {
                    return single_effect(Effect::Drop(DropReason::InvalidEthernetFrame));
                }
                if !ingress.frame.source.is_unicast() {
                    return single_effect(Effect::Drop(DropReason::InvalidSourceMac(
                        ingress.frame.source,
                    )));
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
                let mut transmitted = false;
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
                    .filter(|port| port.id != ingress.port && port.forwarding)
                    .map(|port| port.id.clone())
                {
                    transmitted = true;
                    effects
                        .push(Effect::Transmit {
                            egress,
                            next_hop: None,
                            frame: ingress.frame.clone(),
                            delay_ms,
                        })
                        .expect("link fan-out exceeds effect capacity");
                }
                if !transmitted {
                    single_effect(Effect::Drop(DropReason::LinkLoss))
                } else {
                    effects
                }
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
