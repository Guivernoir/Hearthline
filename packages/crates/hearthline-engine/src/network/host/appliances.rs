use core::net::Ipv4Addr;

use heapless::Vec as FixedList;

use hearthline_model::{
    ApplicationData, ComponentId, ComponentKind, HttpDocument, IcmpMessage, NetworkPayload, PortId,
    ServiceKind, Text, Transport,
};

use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{DropReason, Effect, EffectList, SimulatedComponent, SimulationEvent};

use super::stack::{EndpointReceive, EndpointStack, response_frame};
use crate::RoutedInterface;

fn inferred_service(packet: &hearthline_model::Ipv4Packet) -> Option<ServiceKind> {
    if let ApplicationData::Service(service) = &packet.application {
        return Some(*service);
    }
    match (
        packet.transport.protocol(),
        packet.transport.destination_port(),
    ) {
        (_, Some(53)) => Some(ServiceKind::Dns),
        (_, Some(67 | 68)) => Some(ServiceKind::Dhcp),
        (_, Some(80)) => Some(ServiceKind::Http),
        (_, Some(443)) => Some(ServiceKind::Https),
        (_, Some(22)) => Some(ServiceKind::Ssh),
        (_, Some(3389)) => Some(ServiceKind::Rdp),
        (_, Some(161)) => Some(ServiceKind::Snmp),
        (_, Some(514)) => Some(ServiceKind::Syslog),
        (_, Some(123)) => Some(ServiceKind::Ntp),
        (_, Some(631)) => Some(ServiceKind::Printing),
        (_, Some(5060 | 5061)) => Some(ServiceKind::VoiceSignaling),
        (_, Some(502 | 4840)) => Some(ServiceKind::IndustrialIo),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub struct ServiceNode {
    id: ComponentId,
    kind: ComponentKind,
    network: EndpointStack,
    services: FixedList<ServiceKind, 16>,
    http_site: Option<(Text<128>, HttpDocument)>,
    operational: bool,
}

impl ServiceNode {
    pub fn new(
        id: ComponentId,
        kind: ComponentKind,
        interfaces: impl IntoIterator<Item = RoutedInterface>,
        services: impl IntoIterator<Item = ServiceKind>,
    ) -> Self {
        Self::from_network(id, kind, EndpointStack::new(interfaces), services)
    }

    pub fn with_default_gateway(
        id: ComponentId,
        kind: ComponentKind,
        interfaces: impl IntoIterator<Item = RoutedInterface>,
        default_gateway: Ipv4Addr,
        services: impl IntoIterator<Item = ServiceKind>,
    ) -> Self {
        Self::from_network(
            id,
            kind,
            EndpointStack::with_default_gateway(interfaces, default_gateway),
            services,
        )
    }

    fn from_network(
        id: ComponentId,
        kind: ComponentKind,
        network: EndpointStack,
        services: impl IntoIterator<Item = ServiceKind>,
    ) -> Self {
        assert!(
            matches!(
                kind.behavior_family(),
                hearthline_model::BehaviorFamily::Endpoint
                    | hearthline_model::BehaviorFamily::ServiceHost
                    | hearthline_model::BehaviorFamily::PolicyService
                    | hearthline_model::BehaviorFamily::Voice
                    | hearthline_model::BehaviorFamily::ComputeHost
            ),
            "component kind does not use service-node behavior"
        );
        Self {
            id,
            kind,
            network,
            services: collect_fixed(services),
            http_site: None,
            operational: true,
        }
    }

    pub fn set_http_site(&mut self, host: Text<128>, document: HttpDocument) {
        self.http_site = Some((host, document));
    }
}

impl SimulatedComponent for ServiceNode {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        self.kind
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.network.has_port(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.operational {
                    return single_effect(Effect::Drop(DropReason::ComponentDown));
                }
                let (interface, frame) = match self.network.receive(ingress) {
                    EndpointReceive::Handled(effects) => return effects,
                    EndpointReceive::Ipv4 { interface, frame } => (interface, frame),
                };
                let NetworkPayload::Ipv4(packet) = &frame.payload else {
                    return single_effect(Effect::Drop(DropReason::UnsupportedProtocol));
                };
                if matches!(
                    packet.transport,
                    Transport::Icmp(IcmpMessage::EchoRequest { .. })
                ) {
                    return single_effect(Effect::Transmit {
                        egress: interface.id.clone(),
                        next_hop: None,
                        frame: response_frame(&interface, frame, ApplicationData::None),
                        delay_ms: 0,
                    });
                }
                if let Transport::Icmp(message) = packet.transport {
                    return single_effect(Effect::Observe {
                        detail: runtime_text(format_args!("{} received ICMP {message:?}", self.id)),
                    });
                }
                if let ApplicationData::DnsAnswer { name, address } = &packet.application {
                    let detail = match address {
                        Some(address) => {
                            runtime_text(format_args!("{} resolved {name} to {address}", self.id))
                        }
                        None => {
                            runtime_text(format_args!("{} received no address for {name}", self.id))
                        }
                    };
                    return single_effect(Effect::Deliver {
                        service: ServiceKind::Dns,
                        detail,
                    });
                }
                if let ApplicationData::HttpResponse { status, .. } = &packet.application {
                    return single_effect(Effect::Deliver {
                        service: ServiceKind::Https,
                        detail: runtime_text(format_args!(
                            "{} received HTTPS response {status}",
                            self.id
                        )),
                    });
                }
                if let ApplicationData::HttpRequest { host, .. } = &packet.application
                    && self.services.contains(&ServiceKind::Https)
                {
                    let Some((configured_host, document)) = &self.http_site else {
                        return single_effect(Effect::Drop(DropReason::ServiceUnavailable(
                            ServiceKind::Https,
                        )));
                    };
                    let (status, document) = if host == configured_host {
                        (200, Some(document.clone()))
                    } else {
                        (404, None)
                    };
                    return single_effect(Effect::Transmit {
                        egress: interface.id.clone(),
                        next_hop: None,
                        frame: response_frame(
                            &interface,
                            frame,
                            ApplicationData::HttpResponse { status, document },
                        ),
                        delay_ms: 0,
                    });
                }
                let Some(service) = inferred_service(packet) else {
                    return single_effect(Effect::Drop(DropReason::UnsupportedProtocol));
                };
                if self.services.contains(&service) {
                    single_effect(Effect::Deliver {
                        service,
                        detail: runtime_text(format_args!("{} accepted {service:?}", self.id)),
                    })
                } else {
                    single_effect(Effect::Drop(DropReason::ServiceUnavailable(service)))
                }
            }
            SimulationEvent::Ipv4Egress(egress) => {
                if !self.operational {
                    return single_effect(Effect::Drop(DropReason::ComponentDown));
                }
                self.network.send(egress)
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                operational_effect(operational)
            }
            SimulationEvent::Process(_) | SimulationEvent::FirewallHa(_) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct DnsServer {
    id: ComponentId,
    network: EndpointStack,
    records: FixedList<(Text<128>, Ipv4Addr), 8>,
    operational: bool,
}

impl DnsServer {
    pub fn new(
        id: ComponentId,
        interfaces: impl IntoIterator<Item = RoutedInterface>,
        records: impl IntoIterator<Item = (Text<128>, Ipv4Addr)>,
    ) -> Self {
        Self::from_network(id, EndpointStack::new(interfaces), records)
    }

    pub fn with_default_gateway(
        id: ComponentId,
        interfaces: impl IntoIterator<Item = RoutedInterface>,
        default_gateway: Ipv4Addr,
        records: impl IntoIterator<Item = (Text<128>, Ipv4Addr)>,
    ) -> Self {
        Self::from_network(
            id,
            EndpointStack::with_default_gateway(interfaces, default_gateway),
            records,
        )
    }

    fn from_network(
        id: ComponentId,
        network: EndpointStack,
        records: impl IntoIterator<Item = (Text<128>, Ipv4Addr)>,
    ) -> Self {
        Self {
            id,
            network,
            records: collect_fixed(records),
            operational: true,
        }
    }
}

impl SimulatedComponent for DnsServer {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::DnsServer
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.network.has_port(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.operational {
                    return single_effect(Effect::Drop(DropReason::ComponentDown));
                }
                let (interface, frame) = match self.network.receive(ingress) {
                    EndpointReceive::Handled(effects) => return effects,
                    EndpointReceive::Ipv4 { interface, frame } => (interface, frame),
                };
                let NetworkPayload::Ipv4(packet) = &frame.payload else {
                    return single_effect(Effect::Drop(DropReason::UnsupportedProtocol));
                };
                let name = match &packet.application {
                    ApplicationData::DnsQuery { name } => name.clone(),
                    _ => {
                        return single_effect(Effect::Drop(DropReason::ServiceUnavailable(
                            ServiceKind::Dns,
                        )));
                    }
                };
                let answer = self
                    .records
                    .iter()
                    .find(|(candidate, _)| *candidate == name)
                    .map(|(_, address)| *address);
                single_effect(Effect::Transmit {
                    egress: interface.id.clone(),
                    next_hop: None,
                    frame: response_frame(
                        &interface,
                        frame,
                        ApplicationData::DnsAnswer {
                            name,
                            address: answer,
                        },
                    ),
                    delay_ms: 0,
                })
            }
            SimulationEvent::Ipv4Egress(egress) => {
                if !self.operational {
                    return single_effect(Effect::Drop(DropReason::ComponentDown));
                }
                self.network.send(egress)
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                operational_effect(operational)
            }
            SimulationEvent::Process(_) | SimulationEvent::FirewallHa(_) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
        }
    }
}

fn operational_effect(operational: bool) -> EffectList {
    single_effect(Effect::Observe {
        detail: runtime_text(format_args!("operational={operational}")),
    })
}
