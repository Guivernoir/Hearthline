use core::net::Ipv4Addr;
use heapless::Vec as FixedList;

use hearthline_model::{
    ComponentId, ComponentKind, FlowKey, NetworkPayload, PortId, Text, TransportProtocol,
};

use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{
    DropReason, Effect, EffectList, Router, RoutingTable, SimulatedComponent, SimulationEvent,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticNat {
    pub public_address: Ipv4Addr,
    pub private_address: Ipv4Addr,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PatFlow {
    flow: FlowKey,
    source_token: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PatMapping {
    internal_address: Ipv4Addr,
    internal_token: u16,
}

#[derive(Clone, Debug)]
pub struct NatRouter {
    id: ComponentId,
    ports: FixedList<PortId, 16>,
    inside_ports: FixedList<PortId, 16>,
    outside_address: Ipv4Addr,
    routes: RoutingTable,
    static_nat: FixedList<StaticNat, 16>,
    outbound_pat: FixedList<(PatFlow, u16), 64>,
    inbound_pat: FixedList<((TransportProtocol, u16), PatMapping), 64>,
    next_pat_token: u16,
    operational: bool,
}

impl NatRouter {
    pub fn new(
        id: ComponentId,
        ports: impl IntoIterator<Item = PortId>,
        inside_ports: impl IntoIterator<Item = PortId>,
        outside_address: Ipv4Addr,
        routes: RoutingTable,
    ) -> Self {
        Self {
            id,
            ports: collect_fixed(ports),
            inside_ports: collect_fixed(inside_ports),
            outside_address,
            routes,
            static_nat: FixedList::new(),
            outbound_pat: FixedList::new(),
            inbound_pat: FixedList::new(),
            next_pat_token: 49_152,
            operational: true,
        }
    }

    pub fn add_static_nat(&mut self, mapping: StaticNat) {
        self.static_nat
            .push(mapping)
            .expect("static NAT table exceeds capacity");
    }

    pub fn translation_count(&self) -> usize {
        self.inbound_pat.len()
    }

    fn allocate_token(&mut self) -> Option<u16> {
        let first = self.next_pat_token;
        loop {
            let candidate = self.next_pat_token;
            self.next_pat_token = if candidate == u16::MAX {
                49_152
            } else {
                candidate + 1
            };
            if !self
                .inbound_pat
                .iter()
                .any(|((_, token), _)| *token == candidate)
            {
                return Some(candidate);
            }
            if self.next_pat_token == first {
                return None;
            }
        }
    }

    fn translate_outbound(
        &mut self,
        frame: &mut hearthline_model::EthernetFrame,
    ) -> Result<Text<192>, DropReason> {
        let NetworkPayload::Ipv4(packet) = &mut frame.payload else {
            return Err(DropReason::UnsupportedProtocol);
        };

        if let Some(mapping) = self
            .static_nat
            .iter()
            .find(|mapping| mapping.private_address == packet.source)
        {
            let original = packet.source;
            packet.source = mapping.public_address;
            return Ok(runtime_text(format_args!(
                "static source NAT {original} -> {}",
                mapping.public_address
            )));
        }

        let Some(internal_token) = packet.transport.source_token() else {
            return Err(DropReason::UnsupportedProtocol);
        };
        let flow = PatFlow {
            flow: packet.flow_key(),
            source_token: internal_token,
        };
        let external_token = if let Some((_, token)) = self
            .outbound_pat
            .iter()
            .find(|(candidate, _)| *candidate == flow)
        {
            *token
        } else {
            let token = self.allocate_token().ok_or(DropReason::QueueLimit)?;
            self.outbound_pat
                .push((flow, token))
                .expect("outbound PAT table exceeds capacity");
            self.inbound_pat
                .push((
                    (packet.transport.protocol(), token),
                    PatMapping {
                        internal_address: packet.source,
                        internal_token,
                    },
                ))
                .expect("inbound PAT table exceeds capacity");
            token
        };

        let internal_address = packet.source;
        packet.source = self.outside_address;
        packet.transport.rewrite_source_token(external_token);
        Ok(runtime_text(format_args!(
            "PAT {internal_address}:{internal_token} -> {}:{external_token}",
            self.outside_address
        )))
    }

    fn translate_inbound(
        &self,
        frame: &mut hearthline_model::EthernetFrame,
    ) -> Result<Text<192>, DropReason> {
        let NetworkPayload::Ipv4(packet) = &mut frame.payload else {
            return Err(DropReason::UnsupportedProtocol);
        };

        if let Some(mapping) = self
            .static_nat
            .iter()
            .find(|mapping| mapping.public_address == packet.destination)
        {
            let original = packet.destination;
            packet.destination = mapping.private_address;
            return Ok(runtime_text(format_args!(
                "static destination NAT {original} -> {}",
                mapping.private_address
            )));
        }

        if packet.destination != self.outside_address {
            return Ok("routed without destination translation".into());
        }
        let Some(external_token) = packet.transport.destination_token() else {
            return Err(DropReason::NoTranslation);
        };
        let Some(mapping) = self
            .inbound_pat
            .iter()
            .find(|(key, _)| *key == (packet.transport.protocol(), external_token))
            .map(|(_, mapping)| mapping)
        else {
            return Err(DropReason::NoTranslation);
        };

        packet.destination = mapping.internal_address;
        packet
            .transport
            .rewrite_destination_token(mapping.internal_token);
        Ok(runtime_text(format_args!(
            "reverse PAT {}:{external_token} -> {}:{}",
            self.outside_address, mapping.internal_address, mapping.internal_token
        )))
    }
}

impl SimulatedComponent for NatRouter {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::NatRouter
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.ports.contains(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::Network(mut ingress) => {
                if !self.operational {
                    return single_effect(Effect::Drop(DropReason::ComponentDown));
                }
                if !self.ports.contains(&ingress.port) {
                    return single_effect(Effect::Drop(DropReason::InvalidIngress(ingress.port)));
                }
                let translation = if self.inside_ports.contains(&ingress.port) {
                    self.translate_outbound(&mut ingress.frame)
                } else {
                    self.translate_inbound(&mut ingress.frame)
                };
                let detail = match translation {
                    Ok(detail) => detail,
                    Err(reason) => return single_effect(Effect::Drop(reason)),
                };
                let mut effects = single_effect(Effect::Observe { detail });
                for effect in Router::forward(&self.routes, ingress.frame) {
                    effects
                        .push(effect)
                        .expect("combined NAT effects exceed capacity");
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
