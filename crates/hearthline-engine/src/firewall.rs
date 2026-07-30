use std::collections::{BTreeMap, BTreeSet};

use hearthline_model::{
    ComponentId, ComponentKind, FlowKey, Ipv4Cidr, NetworkPayload, PortId, TransportProtocol,
};

use crate::{
    DropReason, Effect, NetworkIngress, Router, RoutingTable, SimulatedComponent, SimulationEvent,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FirewallAction {
    Permit,
    Deny,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirewallRule {
    pub id: String,
    pub source_zone: Option<String>,
    pub destination_zone: Option<String>,
    pub source: Option<Ipv4Cidr>,
    pub destination: Option<Ipv4Cidr>,
    pub protocol: Option<TransportProtocol>,
    pub destination_port: Option<u16>,
    pub action: FirewallAction,
}

impl FirewallRule {
    fn matches(
        &self,
        source_zone: &str,
        destination_zone: &str,
        packet: &hearthline_model::Ipv4Packet,
    ) -> bool {
        self.source_zone
            .as_deref()
            .is_none_or(|zone| zone == source_zone)
            && self
                .destination_zone
                .as_deref()
                .is_none_or(|zone| zone == destination_zone)
            && self
                .source
                .is_none_or(|prefix| prefix.contains(packet.source))
            && self
                .destination
                .is_none_or(|prefix| prefix.contains(packet.destination))
            && self
                .protocol
                .is_none_or(|protocol| protocol == packet.transport.protocol())
            && self
                .destination_port
                .is_none_or(|port| packet.transport.destination_port() == Some(port))
    }
}

#[derive(Clone, Debug)]
pub struct StatefulFirewall {
    id: ComponentId,
    ports: BTreeSet<PortId>,
    zones: BTreeMap<PortId, String>,
    routes: RoutingTable,
    rules: Vec<FirewallRule>,
    sessions: BTreeSet<FlowKey>,
    operational: bool,
}

impl StatefulFirewall {
    pub fn new(
        id: ComponentId,
        zones: impl IntoIterator<Item = (PortId, String)>,
        routes: RoutingTable,
        rules: Vec<FirewallRule>,
    ) -> Self {
        let zones = zones.into_iter().collect::<BTreeMap<_, _>>();
        Self {
            id,
            ports: zones.keys().cloned().collect(),
            zones,
            routes,
            rules,
            sessions: BTreeSet::new(),
            operational: true,
        }
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len() / 2
    }

    fn handle_network(&mut self, ingress: NetworkIngress) -> Vec<Effect> {
        if !self.operational {
            return vec![Effect::Drop(DropReason::ComponentDown)];
        }
        let Some(source_zone) = self.zones.get(&ingress.port) else {
            return vec![Effect::Drop(DropReason::InvalidIngress(ingress.port))];
        };
        let NetworkPayload::Ipv4(packet) = &ingress.frame.payload else {
            return vec![Effect::Drop(DropReason::UnsupportedProtocol)];
        };
        let Some(route) = self.routes.lookup(packet.destination) else {
            return vec![Effect::Drop(DropReason::NoRoute(packet.destination))];
        };
        let Some(destination_zone) = self.zones.get(&route.egress) else {
            return vec![Effect::Drop(DropReason::InvalidIngress(
                route.egress.clone(),
            ))];
        };
        let flow = packet.flow_key();

        if self.sessions.contains(&flow) {
            let mut effects = vec![Effect::Observe {
                detail: "allowed by existing stateful session".into(),
            }];
            effects.extend(Router::forward(&self.routes, ingress.frame));
            return effects;
        }

        let matched_rule = self
            .rules
            .iter()
            .find(|rule| rule.matches(source_zone, destination_zone, packet));
        match matched_rule {
            Some(rule) if rule.action == FirewallAction::Permit => {
                self.sessions.insert(flow);
                self.sessions.insert(flow.reverse());
                let mut effects = vec![Effect::Observe {
                    detail: format!("allowed by firewall rule {}", rule.id),
                }];
                effects.extend(Router::forward(&self.routes, ingress.frame));
                effects
            }
            Some(rule) => vec![Effect::Drop(DropReason::PolicyDenied {
                rule: Some(rule.id.clone()),
            })],
            None => vec![Effect::Drop(DropReason::PolicyDenied { rule: None })],
        }
    }
}

impl SimulatedComponent for StatefulFirewall {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::Firewall
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.ports.contains(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> Vec<Effect> {
        match event {
            SimulationEvent::Network(ingress) => self.handle_network(ingress),
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                if !operational {
                    self.sessions.clear();
                }
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
        ApplicationData, EthernetFrame, Ipv4Packet, MacAddress, Route, TcpFlags, TcpSegment,
        Transport, VlanId,
    };

    use super::*;

    fn id(value: &str) -> ComponentId {
        ComponentId::new(value).expect("test ID")
    }

    fn port(value: &str) -> PortId {
        PortId::new(value).expect("test port")
    }

    fn frame(
        source: Ipv4Addr,
        destination: Ipv4Addr,
        source_port: u16,
        destination_port: u16,
    ) -> hearthline_model::EthernetFrame {
        EthernetFrame {
            source: MacAddress::new([0, 1, 2, 3, 4, 5]),
            destination: MacAddress::new([0, 1, 2, 3, 4, 6]),
            vlan: VlanId::new(10).expect("VLAN"),
            payload: NetworkPayload::Ipv4(Ipv4Packet {
                source,
                destination,
                ttl: 64,
                transport: Transport::Tcp(TcpSegment {
                    source_port,
                    destination_port,
                    flags: TcpFlags {
                        syn: true,
                        ..TcpFlags::default()
                    },
                }),
                application: ApplicationData::None,
            }),
        }
    }

    #[test]
    fn permits_https_tracks_reverse_state_and_denies_other_ports() {
        let public = Ipv4Cidr::new(Ipv4Addr::new(172, 16, 10, 0), 24).expect("public");
        let routes = RoutingTable::new(vec![
            Route {
                destination: public,
                egress: port("dmz"),
                next_hop: None,
                metric: 0,
            },
            Route {
                destination: Ipv4Cidr::new(Ipv4Addr::UNSPECIFIED, 0).expect("default"),
                egress: port("outside"),
                next_hop: None,
                metric: 10,
            },
        ]);
        let rules = vec![FirewallRule {
            id: "allow-public-https".into(),
            source_zone: Some("outside".into()),
            destination_zone: Some("dmz".into()),
            source: None,
            destination: Some(public),
            protocol: Some(TransportProtocol::Tcp),
            destination_port: Some(443),
            action: FirewallAction::Permit,
        }];
        let mut firewall = StatefulFirewall::new(
            id("business-frw-01a"),
            [
                (port("outside"), "outside".into()),
                (port("dmz"), "dmz".into()),
            ],
            routes,
            rules,
        );
        let client = Ipv4Addr::new(203, 0, 113, 2);
        let server = Ipv4Addr::new(172, 16, 10, 2);

        let allowed = firewall.handle(SimulationEvent::Network(NetworkIngress {
            port: port("outside"),
            frame: frame(client, server, 50_000, 443),
        }));
        assert!(matches!(allowed[1], Effect::Transmit { .. }));
        assert_eq!(firewall.session_count(), 1);

        let returned = firewall.handle(SimulationEvent::Network(NetworkIngress {
            port: port("dmz"),
            frame: frame(server, client, 443, 50_000),
        }));
        assert!(matches!(returned[1], Effect::Transmit { .. }));

        let denied = firewall.handle(SimulationEvent::Network(NetworkIngress {
            port: port("outside"),
            frame: frame(client, server, 50_001, 22),
        }));
        assert!(matches!(
            denied[0],
            Effect::Drop(DropReason::PolicyDenied { rule: None })
        ));
    }
}
