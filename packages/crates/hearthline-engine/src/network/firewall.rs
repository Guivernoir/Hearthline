use heapless::Vec as FixedList;

use hearthline_model::{
    ComponentId, ComponentKind, FlowKey, Ipv4Cidr, NetworkPayload, PortId, Text, TransportProtocol,
};

use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{
    DropReason, Effect, EffectList, NetworkIngress, Router, RoutingTable, SimulatedComponent,
    SimulationEvent,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FirewallAction {
    Permit,
    Deny,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirewallRule {
    pub id: Text<64>,
    pub source_zone: Option<Text<64>>,
    pub destination_zone: Option<Text<64>>,
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
    zones: FixedList<(PortId, Text<64>), 16>,
    routes: RoutingTable,
    rules: FixedList<FirewallRule, 16>,
    sessions: FixedList<FlowKey, 128>,
    operational: bool,
}

impl StatefulFirewall {
    pub fn new(
        id: ComponentId,
        zones: impl IntoIterator<Item = (PortId, Text<64>)>,
        routes: RoutingTable,
        rules: impl IntoIterator<Item = FirewallRule>,
    ) -> Self {
        Self {
            id,
            zones: collect_fixed(zones),
            routes,
            rules: collect_fixed(rules),
            sessions: FixedList::new(),
            operational: true,
        }
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len() / 2
    }

    fn handle_network(&mut self, ingress: NetworkIngress) -> EffectList {
        if !self.operational {
            return single_effect(Effect::Drop(DropReason::ComponentDown));
        }
        let Some(source_zone) = self
            .zones
            .iter()
            .find(|(port, _)| *port == ingress.port)
            .map(|(_, zone)| zone)
        else {
            return single_effect(Effect::Drop(DropReason::InvalidIngress(ingress.port)));
        };
        let NetworkPayload::Ipv4(packet) = &ingress.frame.payload else {
            return single_effect(Effect::Drop(DropReason::UnsupportedProtocol));
        };
        let Some(route) = self.routes.lookup(packet.destination) else {
            return single_effect(Effect::Drop(DropReason::NoRoute(packet.destination)));
        };
        let Some(destination_zone) = self
            .zones
            .iter()
            .find(|(port, _)| *port == route.egress)
            .map(|(_, zone)| zone)
        else {
            return single_effect(Effect::Drop(DropReason::InvalidIngress(
                route.egress.clone(),
            )));
        };
        let flow = packet.flow_key();

        if self.sessions.contains(&flow) {
            let mut effects = single_effect(Effect::Observe {
                detail: "allowed by existing stateful session".into(),
            });
            append_effects(&mut effects, Router::forward(&self.routes, ingress.frame));
            return effects;
        }

        let matched_rule = self
            .rules
            .iter()
            .find(|rule| rule.matches(source_zone, destination_zone, packet));
        match matched_rule {
            Some(rule) if rule.action == FirewallAction::Permit => {
                for session in [flow, flow.reverse()] {
                    if !self.sessions.contains(&session) {
                        self.sessions
                            .push(session)
                            .expect("firewall session table exceeds capacity");
                    }
                }
                let mut effects = single_effect(Effect::Observe {
                    detail: runtime_text(format_args!("allowed by firewall rule {}", rule.id)),
                });
                append_effects(&mut effects, Router::forward(&self.routes, ingress.frame));
                effects
            }
            Some(rule) => single_effect(Effect::Drop(DropReason::PolicyDenied {
                rule: Some(rule.id.clone()),
            })),
            None => single_effect(Effect::Drop(DropReason::PolicyDenied { rule: None })),
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
        self.zones.iter().any(|(candidate, _)| candidate == port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::Network(ingress) => self.handle_network(ingress),
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                if !operational {
                    self.sessions.clear();
                }
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

fn append_effects(target: &mut EffectList, source: EffectList) {
    for effect in source {
        target
            .push(effect)
            .expect("combined firewall effects exceed capacity");
    }
}
