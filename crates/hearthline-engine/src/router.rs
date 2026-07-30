use std::collections::BTreeSet;

use hearthline_model::{ComponentId, ComponentKind, EthernetFrame, NetworkPayload, PortId, Route};

use crate::{DropReason, Effect, NetworkIngress, SimulatedComponent, SimulationEvent};

#[derive(Clone, Debug, Default)]
pub struct RoutingTable {
    routes: Vec<Route>,
}

impl RoutingTable {
    pub fn new(mut routes: Vec<Route>) -> Self {
        routes.sort_by(|left, right| {
            right
                .destination
                .prefix()
                .cmp(&left.destination.prefix())
                .then(left.metric.cmp(&right.metric))
        });
        Self { routes }
    }

    pub fn lookup(&self, destination: std::net::Ipv4Addr) -> Option<&Route> {
        self.routes
            .iter()
            .find(|route| route.destination.contains(destination))
    }

    pub fn routes(&self) -> &[Route] {
        &self.routes
    }
}

#[derive(Clone, Debug)]
pub struct Router {
    id: ComponentId,
    kind: ComponentKind,
    ports: BTreeSet<PortId>,
    routes: RoutingTable,
    operational: bool,
}

impl Router {
    pub fn new(
        id: ComponentId,
        kind: ComponentKind,
        ports: impl IntoIterator<Item = PortId>,
        routes: RoutingTable,
    ) -> Self {
        assert!(
            matches!(kind, ComponentKind::Router | ComponentKind::Layer3Switch),
            "Router supports router or layer-3-switch kinds"
        );
        Self {
            id,
            kind,
            ports: ports.into_iter().collect(),
            routes,
            operational: true,
        }
    }

    pub(crate) fn forward(routes: &RoutingTable, mut frame: EthernetFrame) -> Vec<Effect> {
        let NetworkPayload::Ipv4(packet) = &mut frame.payload else {
            return vec![Effect::Drop(DropReason::UnsupportedProtocol)];
        };
        if packet.ttl <= 1 {
            return vec![Effect::Drop(DropReason::TtlExpired)];
        }
        let Some(route) = routes.lookup(packet.destination) else {
            return vec![Effect::Drop(DropReason::NoRoute(packet.destination))];
        };
        packet.ttl -= 1;
        vec![Effect::Transmit {
            egress: route.egress.clone(),
            next_hop: route.next_hop,
            frame,
            delay_ms: 0,
        }]
    }

    fn handle_network(&self, ingress: NetworkIngress) -> Vec<Effect> {
        if !self.operational {
            return vec![Effect::Drop(DropReason::ComponentDown)];
        }
        if !self.ports.contains(&ingress.port) {
            return vec![Effect::Drop(DropReason::InvalidIngress(ingress.port))];
        }
        Self::forward(&self.routes, ingress.frame)
    }
}

impl SimulatedComponent for Router {
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
            SimulationEvent::Network(ingress) => self.handle_network(ingress),
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
        ApplicationData, Ipv4Cidr, Ipv4Packet, MacAddress, NetworkPayload, TcpFlags, TcpSegment,
        Transport, VlanId,
    };

    use super::*;

    fn id(value: &str) -> ComponentId {
        ComponentId::new(value).expect("test ID")
    }

    fn port(value: &str) -> PortId {
        PortId::new(value).expect("test port")
    }

    #[test]
    fn longest_prefix_route_wins_and_ttl_decrements() {
        let routes = RoutingTable::new(vec![
            Route {
                destination: Ipv4Cidr::new(Ipv4Addr::UNSPECIFIED, 0).expect("default"),
                egress: port("outside"),
                next_hop: Some(Ipv4Addr::new(203, 0, 113, 1)),
                metric: 10,
            },
            Route {
                destination: Ipv4Cidr::new(Ipv4Addr::new(10, 10, 20, 0), 24).expect("internal"),
                egress: port("inside"),
                next_hop: None,
                metric: 0,
            },
        ]);
        let mut router = Router::new(
            id("router-01"),
            ComponentKind::Router,
            [port("inside"), port("outside")],
            routes,
        );
        let frame = EthernetFrame {
            source: MacAddress::new([0, 1, 2, 3, 4, 5]),
            destination: MacAddress::new([0, 1, 2, 3, 4, 6]),
            vlan: VlanId::new(20).expect("VLAN"),
            payload: NetworkPayload::Ipv4(Ipv4Packet {
                source: Ipv4Addr::new(10, 10, 30, 10),
                destination: Ipv4Addr::new(10, 10, 20, 10),
                ttl: 64,
                transport: Transport::Tcp(TcpSegment {
                    source_port: 50_000,
                    destination_port: 443,
                    flags: TcpFlags::default(),
                }),
                application: ApplicationData::None,
            }),
        };
        let effects = router.handle(SimulationEvent::Network(NetworkIngress {
            port: port("inside"),
            frame,
        }));

        let Effect::Transmit { egress, frame, .. } = &effects[0] else {
            panic!("expected transmission");
        };
        assert_eq!(egress, &port("inside"));
        let NetworkPayload::Ipv4(packet) = &frame.payload else {
            panic!("expected IPv4 packet");
        };
        assert_eq!(packet.ttl, 63);
    }
}
