use core::net::Ipv4Addr;

use heapless::Vec as FixedList;
use hearthline_model::{ComponentId, ComponentKind, PortId, VlanId};

use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{DropReason, Effect, EffectList, NetworkIngress, SimulatedComponent, SimulationEvent};

use super::{LearningSwitch, MacTableEntry, SwitchAggregationGroup, SwitchPort};
use crate::network::forwarding::{
    ForwardingPlane, NeighborEntry, ReceiveOutcome, RoutedInterface, RoutingTable, local_response,
};

const PORT_CAPACITY: usize = 16;

#[derive(Clone, Debug)]
pub struct Layer3Switch {
    id: ComponentId,
    bridge: LearningSwitch,
    plane: ForwardingPlane,
    bridge_ports: FixedList<PortId, PORT_CAPACITY>,
    routed_ports: FixedList<PortId, PORT_CAPACITY>,
    svi_ports: FixedList<PortId, PORT_CAPACITY>,
    operational: bool,
}

impl Layer3Switch {
    pub fn new(
        id: ComponentId,
        switch_ports: impl IntoIterator<Item = SwitchPort>,
        routed_interfaces: impl IntoIterator<Item = RoutedInterface>,
        svi_ports: impl IntoIterator<Item = PortId>,
        routes: RoutingTable,
    ) -> Self {
        let interfaces: FixedList<RoutedInterface, PORT_CAPACITY> =
            collect_fixed(routed_interfaces);
        let svi_ports: FixedList<PortId, PORT_CAPACITY> = collect_fixed(svi_ports);
        let mut bridge_ports = FixedList::new();
        let mut routed_ports = FixedList::new();
        let mut bridge_config: FixedList<SwitchPort, PORT_CAPACITY> = FixedList::new();
        let mut svi_vlans: FixedList<VlanId, PORT_CAPACITY> = FixedList::new();

        for port in switch_ports {
            assert!(
                !interfaces.iter().any(|interface| interface.id == port.id),
                "physical switch port cannot also be a routed interface"
            );
            bridge_ports
                .push(port.id.clone())
                .expect("layer-3 switch bridge-port capacity exceeded");
            bridge_config
                .push(port)
                .expect("layer-3 switch bridge-port capacity exceeded");
        }
        for interface in &interfaces {
            if svi_ports.contains(&interface.id) {
                assert!(
                    !svi_vlans.contains(&interface.vlan),
                    "layer-3 switch supports one SVI per VLAN"
                );
                svi_vlans
                    .push(interface.vlan)
                    .expect("layer-3 switch SVI capacity exceeded");
                let mut port = SwitchPort::access(interface.id.clone(), interface.vlan);
                port.forwarding = interface.forwarding;
                bridge_config
                    .push(port)
                    .expect("layer-3 switch bridge-port capacity exceeded");
            } else {
                routed_ports
                    .push(interface.id.clone())
                    .expect("layer-3 switch routed-port capacity exceeded");
            }
        }
        assert!(
            svi_ports
                .iter()
                .all(|port| interfaces.iter().any(|interface| interface.id == *port)),
            "every SVI must reference a routed interface"
        );

        Self {
            id: id.clone(),
            bridge: LearningSwitch::new(id, bridge_config),
            plane: ForwardingPlane::new(interfaces, routes),
            bridge_ports,
            routed_ports,
            svi_ports,
            operational: true,
        }
    }

    pub fn set_port_forwarding(&mut self, port: &PortId, forwarding: bool) -> bool {
        if !self.bridge_ports.contains(port) {
            return false;
        }
        self.bridge.set_port_forwarding(port, forwarding)
    }

    pub fn set_spanning_tree_forwarding(
        &mut self,
        port: &PortId,
        vlan: VlanId,
        forwarding: bool,
    ) -> bool {
        self.bridge_ports.contains(port)
            && self
                .bridge
                .set_spanning_tree_forwarding(port, vlan, forwarding)
    }

    pub fn add_link_aggregation_group(&mut self, group: SwitchAggregationGroup) -> bool {
        self.bridge.add_link_aggregation_group(group)
    }

    pub fn set_multi_chassis_peer_link(&mut self, port: PortId) -> bool {
        self.bridge_ports.contains(&port) && self.bridge.set_multi_chassis_peer_link(port)
    }

    pub fn set_link_aggregation_forwarding(&mut self, port: &PortId, forwarding: bool) -> bool {
        self.bridge_ports.contains(port)
            && self
                .bridge
                .set_link_aggregation_forwarding(port, forwarding)
    }

    pub fn set_multi_chassis_peer_forwarding(
        &mut self,
        logical_id: &ComponentId,
        forwarding: bool,
    ) -> bool {
        self.bridge
            .set_multi_chassis_peer_forwarding(logical_id, forwarding)
    }

    pub fn set_first_hop_active(&mut self, port: &PortId, address: Ipv4Addr, active: bool) -> bool {
        self.svi_ports.contains(port) && self.plane.set_first_hop_active(port, address, active)
    }

    pub fn active_mac_table(&self, now_us: u64) -> impl Iterator<Item = (&MacTableEntry, u64)> {
        self.bridge.active_mac_table(now_us)
    }

    pub fn neighbors(&self, now_us: u64) -> impl Iterator<Item = &NeighborEntry> {
        self.plane.neighbors(now_us)
    }

    fn handle_network(&mut self, ingress: NetworkIngress) -> EffectList {
        if !self.operational {
            return single_effect(Effect::Drop(DropReason::ComponentDown));
        }
        if self.bridge_ports.contains(&ingress.port) {
            let received_at_us = ingress.received_at_us;
            let effects = self.bridge.handle(SimulationEvent::Network(ingress));
            return self.expand_bridge_effects(effects, received_at_us);
        }
        if self.routed_ports.contains(&ingress.port) {
            return self.handle_plane_ingress(ingress);
        }
        single_effect(Effect::Drop(DropReason::InvalidIngress(ingress.port)))
    }

    fn handle_plane_ingress(&mut self, ingress: NetworkIngress) -> EffectList {
        let received_at_us = ingress.received_at_us;
        let effects = match self.plane.receive(ingress) {
            ReceiveOutcome::Handled(effects) => effects,
            ReceiveOutcome::Transit {
                frame,
                received_at_us,
                ..
            } => self.plane.forward(frame, received_at_us),
            ReceiveOutcome::Local { ingress, frame } => local_response(ingress, frame),
        };
        self.expand_plane_effects(effects, received_at_us)
    }

    fn expand_bridge_effects(&mut self, effects: EffectList, now_us: u64) -> EffectList {
        let mut expanded = EffectList::new();
        for effect in effects {
            match effect {
                Effect::Transmit {
                    egress,
                    frame,
                    delay_ms,
                    ..
                } if self.svi_ports.contains(&egress) => {
                    let routed = self.handle_plane_ingress(NetworkIngress {
                        port: egress,
                        frame,
                        received_at_us: now_us.saturating_add(delay_ms.saturating_mul(1_000)),
                    });
                    append_effects(&mut expanded, routed);
                }
                other => push_effect(&mut expanded, other),
            }
        }
        expanded
    }

    fn expand_plane_effects(&mut self, effects: EffectList, now_us: u64) -> EffectList {
        let mut expanded = EffectList::new();
        for effect in effects {
            match effect {
                Effect::Transmit {
                    egress,
                    next_hop,
                    frame,
                    delay_ms,
                } if self.svi_ports.contains(&egress) => {
                    let bridged = self.bridge.handle(SimulationEvent::Network(NetworkIngress {
                        port: egress,
                        frame,
                        received_at_us: now_us.saturating_add(delay_ms.saturating_mul(1_000)),
                    }));
                    for effect in bridged {
                        let effect = match effect {
                            Effect::Transmit {
                                egress,
                                frame,
                                delay_ms: bridge_delay,
                                ..
                            } => Effect::Transmit {
                                egress,
                                next_hop,
                                frame,
                                delay_ms: delay_ms.saturating_add(bridge_delay),
                            },
                            other => other,
                        };
                        push_effect(&mut expanded, effect);
                    }
                }
                other => push_effect(&mut expanded, other),
            }
        }
        expanded
    }
}

impl SimulatedComponent for Layer3Switch {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::Layer3Switch
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.bridge_ports.contains(port) || self.routed_ports.contains(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::Network(ingress) => self.handle_network(ingress),
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

fn append_effects(target: &mut EffectList, effects: EffectList) {
    for effect in effects {
        push_effect(target, effect);
    }
}

fn push_effect(target: &mut EffectList, effect: Effect) {
    target
        .push(effect)
        .expect("layer-3 switch effect capacity exceeded");
}
