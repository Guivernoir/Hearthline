use core::mem;
use core::net::Ipv4Addr;

use heapless::Vec as FixedList;
use hearthline_model::{
    ApplicationData, ComponentId, ComponentKind, EthernetFrame, IcmpMessage, NetworkPayload,
    PortId, Route, Transport,
};

use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{DropReason, Effect, EffectList, NetworkIngress, SimulatedComponent, SimulationEvent};

use super::{ForwardingPlane, NeighborEntry, ReceiveOutcome, RoutedInterface};

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
    plane: ForwardingPlane,
    operational: bool,
}

impl Router {
    pub fn new(
        id: ComponentId,
        kind: ComponentKind,
        interfaces: impl IntoIterator<Item = RoutedInterface>,
        routes: RoutingTable,
    ) -> Self {
        assert!(
            matches!(kind, ComponentKind::Router | ComponentKind::Layer3Switch),
            "Router supports router or layer-3-switch kinds"
        );
        Self {
            id,
            kind,
            plane: ForwardingPlane::new(interfaces, routes),
            operational: true,
        }
    }

    pub fn neighbor(
        &self,
        address: Ipv4Addr,
        port: &PortId,
        now_us: u64,
    ) -> Option<&NeighborEntry> {
        self.plane.neighbor(address, port, now_us)
    }

    pub fn neighbors(&self, now_us: u64) -> impl Iterator<Item = &NeighborEntry> {
        self.plane.neighbors(now_us)
    }

    pub fn set_first_hop_active(&mut self, port: &PortId, address: Ipv4Addr, active: bool) -> bool {
        self.plane.set_first_hop_active(port, address, active)
    }

    fn handle_network(&mut self, ingress: NetworkIngress) -> EffectList {
        if !self.operational {
            return single_effect(Effect::Drop(DropReason::ComponentDown));
        }
        match self.plane.receive(ingress) {
            ReceiveOutcome::Handled(effects) => effects,
            ReceiveOutcome::Transit {
                frame,
                received_at_us,
                ..
            } => self.plane.forward(frame, received_at_us),
            ReceiveOutcome::Local { ingress, frame } => local_response(ingress, frame),
        }
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
        self.plane.has_port(port)
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

pub(crate) fn local_response(ingress: PortId, mut frame: EthernetFrame) -> EffectList {
    let NetworkPayload::Ipv4(packet) = &mut frame.payload else {
        return single_effect(Effect::Drop(DropReason::UnsupportedProtocol));
    };
    let Transport::Icmp(IcmpMessage::EchoRequest {
        identifier,
        sequence,
    }) = packet.transport
    else {
        return single_effect(Effect::Drop(DropReason::UnsupportedProtocol));
    };
    mem::swap(&mut frame.source, &mut frame.destination);
    mem::swap(&mut packet.source, &mut packet.destination);
    packet.ttl = 64;
    packet.transport = Transport::Icmp(IcmpMessage::EchoReply {
        identifier,
        sequence,
    });
    packet.application = ApplicationData::None;
    single_effect(Effect::Transmit {
        egress: ingress,
        next_hop: Some(packet.destination),
        frame,
        delay_ms: 0,
    })
}
