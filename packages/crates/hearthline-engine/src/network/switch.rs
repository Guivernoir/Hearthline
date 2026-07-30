use heapless::Vec as FixedList;

use hearthline_model::{ComponentId, ComponentKind, MacAddress, PortId, VlanId};

use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{DropReason, Effect, EffectList, SimulatedComponent, SimulationEvent};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwitchPort {
    pub id: PortId,
    pub allowed_vlans: FixedList<VlanId, 32>,
    pub forwarding: bool,
}

impl SwitchPort {
    pub fn access(id: PortId, vlan: VlanId) -> Self {
        Self {
            id,
            allowed_vlans: collect_fixed([vlan]),
            forwarding: true,
        }
    }

    pub fn trunk(id: PortId, allowed_vlans: impl IntoIterator<Item = VlanId>) -> Self {
        Self {
            id,
            allowed_vlans: collect_fixed(allowed_vlans),
            forwarding: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LearningSwitch {
    id: ComponentId,
    kind: ComponentKind,
    ports: FixedList<SwitchPort, 16>,
    forwarding_table: FixedList<((VlanId, MacAddress), PortId), 32>,
    operational: bool,
}

impl LearningSwitch {
    pub fn new(id: ComponentId, ports: impl IntoIterator<Item = SwitchPort>) -> Self {
        Self::with_kind(id, ComponentKind::Layer2Switch, ports)
    }

    fn with_kind(
        id: ComponentId,
        kind: ComponentKind,
        ports: impl IntoIterator<Item = SwitchPort>,
    ) -> Self {
        Self {
            id,
            kind,
            ports: collect_fixed(ports),
            forwarding_table: FixedList::new(),
            operational: true,
        }
    }

    pub fn learned_port(&self, vlan: VlanId, address: MacAddress) -> Option<&PortId> {
        self.forwarding_table
            .iter()
            .find(|(key, _)| *key == (vlan, address))
            .map(|(_, port)| port)
    }
}

#[derive(Clone, Debug)]
pub struct WirelessAccessPoint {
    bridge: LearningSwitch,
    wireless_port: PortId,
    associated_clients: FixedList<MacAddress, 16>,
}

impl WirelessAccessPoint {
    pub fn new(
        id: ComponentId,
        ports: impl IntoIterator<Item = SwitchPort>,
        wireless_port: PortId,
        associated_clients: impl IntoIterator<Item = MacAddress>,
    ) -> Self {
        let bridge = LearningSwitch::with_kind(id, ComponentKind::WirelessAccessPoint, ports);
        assert!(
            bridge.has_port(&wireless_port),
            "wireless port must exist on the access point"
        );
        Self {
            bridge,
            wireless_port,
            associated_clients: collect_fixed(associated_clients),
        }
    }

    pub fn associate(&mut self, client: MacAddress) {
        if !self.associated_clients.contains(&client) {
            self.associated_clients
                .push(client)
                .expect("wireless association table exceeds capacity");
        }
    }

    pub fn disassociate(&mut self, client: MacAddress) {
        if let Some(index) = self
            .associated_clients
            .iter()
            .position(|candidate| *candidate == client)
        {
            self.associated_clients.swap_remove(index);
        }
    }
}

impl SimulatedComponent for WirelessAccessPoint {
    fn id(&self) -> &ComponentId {
        self.bridge.id()
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::WirelessAccessPoint
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.bridge.has_port(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        if let SimulationEvent::Network(ingress) = &event
            && ingress.port == self.wireless_port
            && !self.associated_clients.contains(&ingress.frame.source)
        {
            return single_effect(Effect::Drop(DropReason::PolicyDenied {
                rule: Some("wireless-association".into()),
            }));
        }
        self.bridge.handle(event)
    }
}

impl SimulatedComponent for LearningSwitch {
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
                let Some(port) = self
                    .ports
                    .iter()
                    .find(|candidate| candidate.id == ingress.port)
                else {
                    return single_effect(Effect::Drop(DropReason::InvalidIngress(ingress.port)));
                };
                if !port.forwarding {
                    return single_effect(Effect::Drop(DropReason::PortDown(ingress.port)));
                }
                if !port.allowed_vlans.contains(&ingress.frame.vlan) {
                    return single_effect(Effect::Drop(DropReason::VlanNotAllowed(
                        ingress.frame.vlan.get(),
                    )));
                }

                let source_key = (ingress.frame.vlan, ingress.frame.source);
                if let Some((_, learned_port)) = self
                    .forwarding_table
                    .iter_mut()
                    .find(|(key, _)| *key == source_key)
                {
                    *learned_port = ingress.port.clone();
                } else {
                    self.forwarding_table
                        .push((source_key, ingress.port.clone()))
                        .expect("switch forwarding table exceeds capacity");
                }

                let learned_egress = (!ingress.frame.destination.is_broadcast()
                    && !ingress.frame.destination.is_multicast())
                .then(|| {
                    self.forwarding_table
                        .iter()
                        .find(|(key, _)| *key == (ingress.frame.vlan, ingress.frame.destination))
                        .map(|(_, port)| port)
                })
                .flatten();

                let egress_ports: FixedList<PortId, 16> = if let Some(egress) = learned_egress {
                    if *egress == ingress.port {
                        return single_effect(Effect::Observe {
                            detail: "filtered destination learned on ingress port".into(),
                        });
                    }
                    collect_fixed([egress.clone()])
                } else {
                    collect_fixed(
                        self.ports
                            .iter()
                            .filter(|candidate| {
                                candidate.id != ingress.port
                                    && candidate.forwarding
                                    && candidate.allowed_vlans.contains(&ingress.frame.vlan)
                            })
                            .map(|candidate| candidate.id.clone()),
                    )
                };

                let mut effects = EffectList::new();
                for egress in egress_ports {
                    effects
                        .push(Effect::Transmit {
                            egress,
                            next_hop: None,
                            frame: ingress.frame.clone(),
                            delay_ms: 0,
                        })
                        .expect("switch fan-out exceeds effect capacity");
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
