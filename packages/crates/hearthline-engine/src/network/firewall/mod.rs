use heapless::Vec as FixedList;

use hearthline_model::{
    ComponentId, ComponentKind, FlowKey, Ipv4Cidr, NetworkPayload, PortId, Text, Transport,
    TransportProtocol,
};

use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{
    DropReason, Effect, EffectList, NetworkIngress, RoutedInterface, RoutingTable,
    SimulatedComponent, SimulationEvent,
};

use super::forwarding::{ForwardingPlane, ReceiveOutcome};

mod ha;

pub use ha::{FirewallHaRuntimeConfig, FirewallHaStatus};

const SESSION_CAPACITY: usize = 128;
const TCP_SESSION_TIMEOUT_US: u64 = 300_000_000;
const UDP_SESSION_TIMEOUT_US: u64 = 60_000_000;
const ICMP_SESSION_TIMEOUT_US: u64 = 30_000_000;
const CLOSING_SESSION_TIMEOUT_US: u64 = 15_000_000;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FirewallSession {
    forward: FlowKey,
    reverse: FlowKey,
    expires_at_us: u64,
}

impl FirewallSession {
    fn new(flow: FlowKey, now_us: u64) -> Self {
        Self {
            forward: flow,
            reverse: flow.reverse(),
            expires_at_us: now_us.saturating_add(session_timeout_us(flow.protocol)),
        }
    }

    fn matches(&self, flow: FlowKey) -> bool {
        self.forward == flow || self.reverse == flow
    }

    fn refresh(&mut self, now_us: u64, closing: bool) {
        let timeout = if closing {
            CLOSING_SESSION_TIMEOUT_US
        } else {
            session_timeout_us(self.forward.protocol)
        };
        self.expires_at_us = now_us.saturating_add(timeout);
    }
}

#[derive(Clone, Debug)]
pub struct StatefulFirewall {
    id: ComponentId,
    zones: FixedList<(PortId, Text<64>), 16>,
    plane: ForwardingPlane,
    rules: FixedList<FirewallRule, 16>,
    sessions: FixedList<FirewallSession, SESSION_CAPACITY>,
    ha: Option<FirewallHaRuntimeConfig>,
    ha_active: bool,
    ha_next_sequence: u64,
    ha_replicated_updates: u64,
    ha_last_heartbeat_us: Option<u64>,
    ha_last_heartbeat_sequence: Option<u64>,
    ha_promoted_at_us: Option<u64>,
    ha_promotion_inhibited_at_us: Option<u64>,
    ha_sync_attached: bool,
    operational: bool,
}

impl StatefulFirewall {
    pub fn new(
        id: ComponentId,
        zones: impl IntoIterator<Item = (PortId, Text<64>)>,
        interfaces: impl IntoIterator<Item = RoutedInterface>,
        routes: RoutingTable,
        rules: impl IntoIterator<Item = FirewallRule>,
    ) -> Self {
        let zones = collect_fixed(zones);
        let interfaces: FixedList<RoutedInterface, 16> = collect_fixed(interfaces);
        assert!(
            zones
                .iter()
                .all(|(port, _)| interfaces.iter().any(|interface| interface.id == *port)),
            "every firewall zone requires a routed interface"
        );
        Self {
            id,
            zones,
            plane: ForwardingPlane::new(interfaces, routes),
            rules: collect_fixed(rules),
            sessions: FixedList::new(),
            ha: None,
            ha_active: true,
            ha_next_sequence: 0,
            ha_replicated_updates: 0,
            ha_last_heartbeat_us: None,
            ha_last_heartbeat_sequence: None,
            ha_promoted_at_us: None,
            ha_promotion_inhibited_at_us: None,
            ha_sync_attached: true,
            operational: true,
        }
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn set_first_hop_active(
        &mut self,
        port: &PortId,
        address: core::net::Ipv4Addr,
        active: bool,
    ) -> bool {
        self.plane.set_first_hop_active(port, address, active)
    }

    fn expire_sessions(&mut self, now_us: u64) -> usize {
        let before = self.sessions.len();
        let mut index = 0;
        while index < self.sessions.len() {
            if now_us >= self.sessions[index].expires_at_us {
                self.sessions.swap_remove(index);
            } else {
                index += 1;
            }
        }
        before.saturating_sub(self.sessions.len())
    }

    fn add_session(&mut self, flow: FlowKey, now_us: u64) {
        if self.sessions.is_full() {
            let oldest = self
                .sessions
                .iter()
                .enumerate()
                .min_by_key(|(_, session)| session.expires_at_us)
                .map(|(index, _)| index)
                .expect("full session table has an entry");
            self.sessions.swap_remove(oldest);
        }
        self.sessions
            .push(FirewallSession::new(flow, now_us))
            .expect("session table has capacity after eviction");
    }

    fn handle_network(&mut self, ingress: NetworkIngress) -> EffectList {
        if !self.operational {
            return single_effect(Effect::Drop(DropReason::ComponentDown));
        }
        if self
            .ha
            .as_ref()
            .is_some_and(|ha| ha.sync_port == ingress.port)
        {
            return self.handle_ha_network(ingress);
        }
        if !self.ha_active {
            return single_effect(Effect::Drop(DropReason::FirewallStandby));
        }
        let Some(source_zone) = self
            .zones
            .iter()
            .find(|(port, _)| *port == ingress.port)
            .map(|(_, zone)| zone.clone())
        else {
            return single_effect(Effect::Drop(DropReason::InvalidIngress(ingress.port)));
        };
        let (frame, received_at_us) = match self.plane.receive(ingress) {
            ReceiveOutcome::Handled(effects) => return effects,
            ReceiveOutcome::Local { .. } => {
                return single_effect(Effect::Drop(DropReason::PolicyDenied { rule: None }));
            }
            ReceiveOutcome::Transit {
                frame,
                received_at_us,
            } => (frame, received_at_us),
        };
        let NetworkPayload::Ipv4(packet) = &frame.payload else {
            return single_effect(Effect::Drop(DropReason::UnsupportedProtocol));
        };
        let Some(route) = self.plane.route(packet.destination) else {
            return single_effect(Effect::Drop(DropReason::NoRoute(packet.destination)));
        };
        let Some(destination_zone) = self
            .zones
            .iter()
            .find(|(port, _)| *port == route.egress)
            .map(|(_, zone)| zone.clone())
        else {
            return single_effect(Effect::Drop(DropReason::InvalidIngress(
                route.egress.clone(),
            )));
        };
        let flow = packet.flow_key();
        let expired_sessions = self.expire_sessions(received_at_us);
        let mut effects = EffectList::new();
        if expired_sessions > 0 {
            effects
                .push(Effect::Observe {
                    detail: runtime_text(format_args!(
                        "expired {expired_sessions} stale stateful session(s) at {received_at_us} us"
                    )),
                })
                .expect("firewall session-expiry observation fits effect capacity");
        }

        if let Some(session_index) = self
            .sessions
            .iter()
            .position(|session| session.matches(flow))
        {
            let (closing, reset) = match packet.transport {
                Transport::Tcp(segment) => {
                    (segment.flags.fin || segment.flags.rst, segment.flags.rst)
                }
                Transport::Icmp(_) | Transport::Udp(_) | Transport::Other(_) => (false, false),
            };
            self.sessions[session_index].refresh(received_at_us, closing);
            effects
                .push(Effect::Observe {
                    detail: "allowed by existing stateful session".into(),
                })
                .expect("firewall stateful-session observation fits effect capacity");
            append_effects(&mut effects, self.plane.forward(frame, received_at_us));
            if reset {
                self.sessions.swap_remove(session_index);
            }
            return effects;
        }

        let matched_rule = self
            .rules
            .iter()
            .find(|rule| rule.matches(&source_zone, &destination_zone, packet))
            .map(|rule| (rule.action, rule.id.clone()));
        match matched_rule {
            Some((FirewallAction::Permit, rule_id)) => {
                if let Transport::Tcp(segment) = packet.transport
                    && (!segment.flags.syn || segment.flags.ack)
                {
                    effects
                        .push(Effect::Drop(DropReason::InvalidTcpState))
                        .expect("firewall invalid-state drop fits effect capacity");
                    return effects;
                }
                self.add_session(flow, received_at_us);
                let session = self
                    .sessions
                    .iter()
                    .find(|session| session.forward == flow)
                    .copied()
                    .expect("new firewall session exists");
                effects
                    .push(Effect::Observe {
                        detail: runtime_text(format_args!("allowed by firewall rule {rule_id}")),
                    })
                    .expect("firewall policy observation fits effect capacity");
                append_effects(&mut effects, self.plane.forward(frame, received_at_us));
                if let Some(effect) = self.session_sync_effect(session) {
                    effects
                        .push(effect)
                        .expect("firewall session sync effect fits capacity");
                }
                if let Some(effect) = self.heartbeat_effect(received_at_us) {
                    effects
                        .push(effect)
                        .expect("firewall heartbeat effect fits capacity");
                }
                effects
            }
            Some((FirewallAction::Deny, rule_id)) => {
                effects
                    .push(Effect::Drop(DropReason::PolicyDenied {
                        rule: Some(rule_id),
                    }))
                    .expect("firewall policy drop fits effect capacity");
                effects
            }
            None => {
                effects
                    .push(Effect::Drop(DropReason::PolicyDenied { rule: None }))
                    .expect("firewall default-policy drop fits effect capacity");
                effects
            }
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
        self.plane.has_port(port) || self.ha.as_ref().is_some_and(|ha| ha.sync_port == *port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::Network(ingress) => self.handle_network(ingress),
            SimulationEvent::FirewallHa(control) => self.handle_ha_control(control),
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                if !operational {
                    self.sessions.clear();
                    self.ha_active = false;
                    self.apply_ha_role_to_first_hops();
                }
                single_effect(Effect::Observe {
                    detail: runtime_text(format_args!("operational={operational}")),
                })
            }
            SimulationEvent::Ipv4Egress(_) | SimulationEvent::Process(_) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
        }
    }
}

const fn session_timeout_us(protocol: TransportProtocol) -> u64 {
    match protocol {
        TransportProtocol::Tcp => TCP_SESSION_TIMEOUT_US,
        TransportProtocol::Udp => UDP_SESSION_TIMEOUT_US,
        TransportProtocol::Icmp | TransportProtocol::Other(_) => ICMP_SESSION_TIMEOUT_US,
    }
}

fn append_effects(target: &mut EffectList, source: EffectList) {
    for effect in source {
        target
            .push(effect)
            .expect("combined firewall effects exceed capacity");
    }
}
