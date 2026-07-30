use core::mem;
use core::net::Ipv4Addr;
use heapless::Vec as FixedList;

use hearthline_model::{
    ApplicationData, ComponentId, ComponentKind, EthernetFrame, HttpMethod, IcmpMessage,
    NetworkPayload, PortId, ServiceKind, TcpFlags, Text, Transport,
};

use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{DropReason, Effect, EffectList, SimulatedComponent, SimulationEvent};

fn addressed_packet<'frame>(
    frame: &'frame EthernetFrame,
    addresses: &[Ipv4Addr],
) -> Result<&'frame hearthline_model::Ipv4Packet, DropReason> {
    let NetworkPayload::Ipv4(packet) = &frame.payload else {
        return Err(DropReason::UnsupportedProtocol);
    };
    if !addresses.contains(&packet.destination) {
        return Err(DropReason::NotAddressedToComponent);
    }
    Ok(packet)
}

fn response_frame(mut frame: EthernetFrame, application: ApplicationData) -> EthernetFrame {
    mem::swap(&mut frame.source, &mut frame.destination);
    let NetworkPayload::Ipv4(packet) = &mut frame.payload else {
        return frame;
    };
    mem::swap(&mut packet.source, &mut packet.destination);
    packet.ttl = 64;
    packet.application = application;
    packet.transport = match packet.transport {
        Transport::Icmp(IcmpMessage::EchoRequest {
            identifier,
            sequence,
        }) => Transport::Icmp(IcmpMessage::EchoReply {
            identifier,
            sequence,
        }),
        Transport::Tcp(segment) => Transport::Tcp(hearthline_model::TcpSegment {
            source_port: segment.destination_port,
            destination_port: segment.source_port,
            flags: TcpFlags {
                ack: true,
                ..TcpFlags::default()
            },
        }),
        Transport::Udp(datagram) => Transport::Udp(hearthline_model::UdpDatagram {
            source_port: datagram.destination_port,
            destination_port: datagram.source_port,
        }),
        other => other,
    };
    frame
}

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
    ports: FixedList<PortId, 16>,
    addresses: FixedList<Ipv4Addr, 8>,
    services: FixedList<ServiceKind, 16>,
    operational: bool,
}

impl ServiceNode {
    pub fn new(
        id: ComponentId,
        kind: ComponentKind,
        ports: impl IntoIterator<Item = PortId>,
        addresses: impl IntoIterator<Item = Ipv4Addr>,
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
            ports: collect_fixed(ports),
            addresses: collect_fixed(addresses),
            services: collect_fixed(services),
            operational: true,
        }
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
        self.ports.contains(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.operational {
                    return single_effect(Effect::Drop(DropReason::ComponentDown));
                }
                if !self.ports.contains(&ingress.port) {
                    return single_effect(Effect::Drop(DropReason::InvalidIngress(ingress.port)));
                }
                let packet = match addressed_packet(&ingress.frame, &self.addresses) {
                    Ok(packet) => packet,
                    Err(reason) => return single_effect(Effect::Drop(reason)),
                };
                if matches!(
                    packet.transport,
                    Transport::Icmp(IcmpMessage::EchoRequest { .. })
                ) {
                    return single_effect(Effect::Transmit {
                        egress: ingress.port,
                        next_hop: None,
                        frame: response_frame(ingress.frame, ApplicationData::None),
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
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                operational_effect(operational)
            }
            SimulationEvent::Process(_) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct DnsServer {
    id: ComponentId,
    ports: FixedList<PortId, 16>,
    addresses: FixedList<Ipv4Addr, 8>,
    records: FixedList<(Text<128>, Ipv4Addr), 8>,
    operational: bool,
}

impl DnsServer {
    pub fn new(
        id: ComponentId,
        ports: impl IntoIterator<Item = PortId>,
        addresses: impl IntoIterator<Item = Ipv4Addr>,
        records: impl IntoIterator<Item = (Text<128>, Ipv4Addr)>,
    ) -> Self {
        Self {
            id,
            ports: collect_fixed(ports),
            addresses: collect_fixed(addresses),
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
        self.ports.contains(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.operational {
                    return single_effect(Effect::Drop(DropReason::ComponentDown));
                }
                if !self.ports.contains(&ingress.port) {
                    return single_effect(Effect::Drop(DropReason::InvalidIngress(ingress.port)));
                }
                let packet = match addressed_packet(&ingress.frame, &self.addresses) {
                    Ok(packet) => packet,
                    Err(reason) => return single_effect(Effect::Drop(reason)),
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
                    egress: ingress.port,
                    next_hop: None,
                    frame: response_frame(
                        ingress.frame,
                        ApplicationData::DnsAnswer {
                            name,
                            address: answer,
                        },
                    ),
                    delay_ms: 0,
                })
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                operational_effect(operational)
            }
            SimulationEvent::Process(_) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReverseProxyWaf {
    id: ComponentId,
    ports: FixedList<PortId, 16>,
    addresses: FixedList<Ipv4Addr, 8>,
    allowed_hosts: FixedList<Text<128>, 8>,
    upstream: ComponentId,
    maximum_body_bytes: usize,
    redirect_http: bool,
    operational: bool,
}

impl ReverseProxyWaf {
    pub fn new(
        id: ComponentId,
        ports: impl IntoIterator<Item = PortId>,
        addresses: impl IntoIterator<Item = Ipv4Addr>,
        allowed_hosts: impl IntoIterator<Item = Text<128>>,
        upstream: ComponentId,
    ) -> Self {
        Self {
            id,
            ports: collect_fixed(ports),
            addresses: collect_fixed(addresses),
            allowed_hosts: collect_fixed(allowed_hosts),
            upstream,
            maximum_body_bytes: 1_048_576,
            redirect_http: true,
            operational: true,
        }
    }

    pub fn set_maximum_body_bytes(&mut self, maximum: usize) {
        self.maximum_body_bytes = maximum;
    }

    pub fn set_redirect_http(&mut self, enabled: bool) {
        self.redirect_http = enabled;
    }
}

impl SimulatedComponent for ReverseProxyWaf {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::ReverseProxyWaf
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.ports.contains(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.operational {
                    return single_effect(Effect::Drop(DropReason::ComponentDown));
                }
                if !self.ports.contains(&ingress.port) {
                    return single_effect(Effect::Drop(DropReason::InvalidIngress(ingress.port)));
                }
                let packet = match addressed_packet(&ingress.frame, &self.addresses) {
                    Ok(packet) => packet,
                    Err(reason) => return single_effect(Effect::Drop(reason)),
                };
                let destination_port = packet.transport.destination_port();
                if destination_port == Some(80) && self.redirect_http {
                    return single_effect(Effect::Transmit {
                        egress: ingress.port,
                        next_hop: None,
                        frame: response_frame(
                            ingress.frame,
                            ApplicationData::HttpResponse { status: 308 },
                        ),
                        delay_ms: 0,
                    });
                }
                if destination_port != Some(443) {
                    return single_effect(Effect::Drop(DropReason::ServiceUnavailable(
                        ServiceKind::Https,
                    )));
                }
                let (method, host, path, body_bytes) = match &packet.application {
                    ApplicationData::HttpRequest {
                        method,
                        host,
                        path,
                        body_bytes,
                    } => (*method, host.clone(), path.clone(), *body_bytes),
                    _ => {
                        return single_effect(Effect::Drop(DropReason::ApplicationRejected(
                            "HTTPS request metadata is required".into(),
                        )));
                    }
                };
                if !self.allowed_hosts.contains(&host) {
                    return single_effect(Effect::Drop(DropReason::ApplicationRejected(
                        "host is not published".into(),
                    )));
                }
                if path.contains("..") {
                    return single_effect(Effect::Drop(DropReason::ApplicationRejected(
                        "path traversal pattern".into(),
                    )));
                }
                if body_bytes > self.maximum_body_bytes {
                    return single_effect(Effect::Drop(DropReason::ApplicationRejected(
                        "request body exceeds configured limit".into(),
                    )));
                }
                if !matches!(
                    method,
                    HttpMethod::Get | HttpMethod::Head | HttpMethod::Post
                ) {
                    return single_effect(Effect::Drop(DropReason::ApplicationRejected(
                        "HTTP method is not allowed".into(),
                    )));
                }
                single_effect(Effect::ApplicationForward {
                    service: ServiceKind::Https,
                    target: self.upstream.clone(),
                    detail: runtime_text(format_args!("accepted HTTPS request for {host}{path}")),
                })
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                operational_effect(operational)
            }
            SimulationEvent::Process(_) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct PassiveSensor {
    id: ComponentId,
    ports: FixedList<PortId, 16>,
    operational: bool,
    observations: u64,
}

impl PassiveSensor {
    pub fn new(id: ComponentId, ports: impl IntoIterator<Item = PortId>) -> Self {
        Self {
            id,
            ports: collect_fixed(ports),
            operational: true,
            observations: 0,
        }
    }

    pub const fn observation_count(&self) -> u64 {
        self.observations
    }
}

impl SimulatedComponent for PassiveSensor {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::PassiveNetworkSensor
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.ports.contains(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.operational {
                    return single_effect(Effect::Drop(DropReason::ComponentDown));
                }
                if !self.ports.contains(&ingress.port) {
                    return single_effect(Effect::Drop(DropReason::InvalidIngress(ingress.port)));
                }
                self.observations += 1;
                single_effect(Effect::Observe {
                    detail: runtime_text(format_args!(
                        "passively observed frame {}",
                        self.observations
                    )),
                })
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                operational_effect(operational)
            }
            SimulationEvent::Process(_) => {
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
