use core::net::Ipv4Addr;
use heapless::Vec as FixedList;

use hearthline_model::{ComponentId, ComponentKind, EthernetFrame, NetworkPayload, PortId, Route};

use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{DropReason, Effect, EffectList, NetworkIngress, SimulatedComponent, SimulationEvent};

#[derive(Clone, Debug, Default)]
pub struct RoutingTable {
    routes: FixedList<Route, 16>,
}

impl RoutingTable {
    pub fn new(routes: impl IntoIterator<Item = Route>) -> Self {
        let mut routes = collect_fixed(routes);
        routes.sort_unstable_by(|left, right| {
            right
                .destination
                .prefix()
                .cmp(&left.destination.prefix())
                .then(left.metric.cmp(&right.metric))
        });
        Self { routes }
    }

    pub fn lookup(&self, destination: Ipv4Addr) -> Option<&Route> {
        self.routes
            .iter()
            .find(|route| route.destination.contains(destination))
    }

    pub fn routes(&self) -> &[Route] {
        self.routes.as_slice()
    }
}

#[derive(Clone, Debug)]
pub struct Router {
    id: ComponentId,
    kind: ComponentKind,
    ports: FixedList<PortId, 16>,
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
            ports: collect_fixed(ports),
            routes,
            operational: true,
        }
    }

    pub(crate) fn forward(routes: &RoutingTable, mut frame: EthernetFrame) -> EffectList {
        let NetworkPayload::Ipv4(packet) = &mut frame.payload else {
            return single_effect(Effect::Drop(DropReason::UnsupportedProtocol));
        };
        if packet.ttl <= 1 {
            return single_effect(Effect::Drop(DropReason::TtlExpired));
        }
        let Some(route) = routes.lookup(packet.destination) else {
            return single_effect(Effect::Drop(DropReason::NoRoute(packet.destination)));
        };
        packet.ttl -= 1;
        single_effect(Effect::Transmit {
            egress: route.egress.clone(),
            next_hop: route.next_hop,
            frame,
            delay_ms: 0,
        })
    }

    fn handle_network(&self, ingress: NetworkIngress) -> EffectList {
        if !self.operational {
            return single_effect(Effect::Drop(DropReason::ComponentDown));
        }
        if !self.ports.contains(&ingress.port) {
            return single_effect(Effect::Drop(DropReason::InvalidIngress(ingress.port)));
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

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::Network(ingress) => self.handle_network(ingress),
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
