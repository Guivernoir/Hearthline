use std::collections::{BTreeMap, BTreeSet};

use hearthline_model::{ComponentId, ComponentKind, MacAddress, PortId, VlanId};

use crate::{DropReason, Effect, SimulatedComponent, SimulationEvent};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwitchPort {
    pub id: PortId,
    pub allowed_vlans: BTreeSet<VlanId>,
    pub forwarding: bool,
}

impl SwitchPort {
    pub fn access(id: PortId, vlan: VlanId) -> Self {
        Self {
            id,
            allowed_vlans: [vlan].into_iter().collect(),
            forwarding: true,
        }
    }

    pub fn trunk(id: PortId, allowed_vlans: impl IntoIterator<Item = VlanId>) -> Self {
        Self {
            id,
            allowed_vlans: allowed_vlans.into_iter().collect(),
            forwarding: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LearningSwitch {
    id: ComponentId,
    kind: ComponentKind,
    ports: BTreeMap<PortId, SwitchPort>,
    forwarding_table: BTreeMap<(VlanId, MacAddress), PortId>,
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
            ports: ports
                .into_iter()
                .map(|port| (port.id.clone(), port))
                .collect(),
            forwarding_table: BTreeMap::new(),
            operational: true,
        }
    }

    pub fn learned_port(&self, vlan: VlanId, address: MacAddress) -> Option<&PortId> {
        self.forwarding_table.get(&(vlan, address))
    }
}

#[derive(Clone, Debug)]
pub struct WirelessAccessPoint {
    bridge: LearningSwitch,
    wireless_port: PortId,
    associated_clients: BTreeSet<MacAddress>,
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
            associated_clients: associated_clients.into_iter().collect(),
        }
    }

    pub fn associate(&mut self, client: MacAddress) {
        self.associated_clients.insert(client);
    }

    pub fn disassociate(&mut self, client: MacAddress) {
        self.associated_clients.remove(&client);
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

    fn handle(&mut self, event: SimulationEvent) -> Vec<Effect> {
        if let SimulationEvent::Network(ingress) = &event
            && ingress.port == self.wireless_port
            && !self.associated_clients.contains(&ingress.frame.source)
        {
            return vec![Effect::Drop(DropReason::PolicyDenied {
                rule: Some("wireless-association".into()),
            })];
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
        self.ports.contains_key(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> Vec<Effect> {
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.operational {
                    return vec![Effect::Drop(DropReason::ComponentDown)];
                }
                let Some(port) = self.ports.get(&ingress.port) else {
                    return vec![Effect::Drop(DropReason::InvalidIngress(ingress.port))];
                };
                if !port.forwarding {
                    return vec![Effect::Drop(DropReason::PortDown(ingress.port))];
                }
                if !port.allowed_vlans.contains(&ingress.frame.vlan) {
                    return vec![Effect::Drop(DropReason::VlanNotAllowed(
                        ingress.frame.vlan.get(),
                    ))];
                }

                self.forwarding_table.insert(
                    (ingress.frame.vlan, ingress.frame.source),
                    ingress.port.clone(),
                );

                let learned_egress = (!ingress.frame.destination.is_broadcast()
                    && !ingress.frame.destination.is_multicast())
                .then(|| {
                    self.forwarding_table
                        .get(&(ingress.frame.vlan, ingress.frame.destination))
                })
                .flatten();

                let egress_ports = if let Some(egress) = learned_egress {
                    if *egress == ingress.port {
                        return vec![Effect::Observe {
                            detail: "filtered destination learned on ingress port".into(),
                        }];
                    }
                    vec![egress.clone()]
                } else {
                    self.ports
                        .values()
                        .filter(|candidate| {
                            candidate.id != ingress.port
                                && candidate.forwarding
                                && candidate.allowed_vlans.contains(&ingress.frame.vlan)
                        })
                        .map(|candidate| candidate.id.clone())
                        .collect()
                };

                egress_ports
                    .into_iter()
                    .map(|egress| Effect::Transmit {
                        egress,
                        next_hop: None,
                        frame: ingress.frame.clone(),
                        delay_ms: 0,
                    })
                    .collect()
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

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use hearthline_model::{
        ApplicationData, EthernetFrame, IcmpMessage, Ipv4Packet, NetworkPayload, Transport,
    };

    use super::*;
    use crate::NetworkIngress;

    fn id(value: &str) -> ComponentId {
        ComponentId::new(value).expect("test ID")
    }

    fn port(value: &str) -> PortId {
        PortId::new(value).expect("test port")
    }

    fn frame(source: MacAddress, destination: MacAddress, vlan: VlanId) -> EthernetFrame {
        EthernetFrame {
            source,
            destination,
            vlan,
            payload: NetworkPayload::Ipv4(Ipv4Packet {
                source: Ipv4Addr::new(192, 168, 0, 2),
                destination: Ipv4Addr::new(192, 168, 0, 1),
                ttl: 64,
                transport: Transport::Icmp(IcmpMessage::EchoRequest {
                    identifier: 1,
                    sequence: 1,
                }),
                application: ApplicationData::None,
            }),
        }
    }

    #[test]
    fn learns_source_and_uses_known_unicast() {
        let vlan = VlanId::new(1).expect("VLAN");
        let mac_a = MacAddress::new([0, 0, 0, 0, 0, 1]);
        let mac_b = MacAddress::new([0, 0, 0, 0, 0, 2]);
        let mut switch = LearningSwitch::new(
            id("switch-01"),
            [
                SwitchPort::access(port("port-a"), vlan),
                SwitchPort::access(port("port-b"), vlan),
                SwitchPort::access(port("port-c"), vlan),
            ],
        );

        switch.handle(SimulationEvent::Network(NetworkIngress {
            port: port("port-a"),
            frame: frame(mac_a, mac_b, vlan),
        }));
        let effects = switch.handle(SimulationEvent::Network(NetworkIngress {
            port: port("port-b"),
            frame: frame(mac_b, mac_a, vlan),
        }));

        assert_eq!(switch.learned_port(vlan, mac_a), Some(&port("port-a")));
        assert_eq!(effects.len(), 1);
        let Effect::Transmit { egress, .. } = &effects[0] else {
            panic!("expected transmission");
        };
        assert_eq!(egress, &port("port-a"));
    }

    #[test]
    fn filters_destination_learned_on_ingress_port() {
        let vlan = VlanId::new(1).expect("VLAN");
        let mac_a = MacAddress::new([0, 0, 0, 0, 0, 1]);
        let mac_b = MacAddress::new([0, 0, 0, 0, 0, 2]);
        let mut switch = LearningSwitch::new(
            id("switch-01"),
            [
                SwitchPort::access(port("port-a"), vlan),
                SwitchPort::access(port("port-b"), vlan),
            ],
        );

        switch.handle(SimulationEvent::Network(NetworkIngress {
            port: port("port-a"),
            frame: frame(mac_a, mac_b, vlan),
        }));
        switch.handle(SimulationEvent::Network(NetworkIngress {
            port: port("port-a"),
            frame: frame(mac_b, mac_a, vlan),
        }));
        let effects = switch.handle(SimulationEvent::Network(NetworkIngress {
            port: port("port-a"),
            frame: frame(mac_a, mac_b, vlan),
        }));

        assert!(matches!(effects[0], Effect::Observe { .. }));
    }
}
