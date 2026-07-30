use std::collections::{BTreeMap, BTreeSet};
use std::net::Ipv4Addr;

use hearthline_model::{
    ApplicationData, ComponentId, ComponentKind, EthernetFrame, HttpMethod, IcmpMessage,
    NetworkPayload, PortId, ServiceKind, TcpFlags, Transport,
};

use crate::{DropReason, Effect, SimulatedComponent, SimulationEvent};

fn addressed_packet<'frame>(
    frame: &'frame EthernetFrame,
    addresses: &BTreeSet<Ipv4Addr>,
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
    std::mem::swap(&mut frame.source, &mut frame.destination);
    let NetworkPayload::Ipv4(packet) = &mut frame.payload else {
        return frame;
    };
    std::mem::swap(&mut packet.source, &mut packet.destination);
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
    ports: BTreeSet<PortId>,
    addresses: BTreeSet<Ipv4Addr>,
    services: BTreeSet<ServiceKind>,
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
            ports: ports.into_iter().collect(),
            addresses: addresses.into_iter().collect(),
            services: services.into_iter().collect(),
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

    fn handle(&mut self, event: SimulationEvent) -> Vec<Effect> {
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.operational {
                    return vec![Effect::Drop(DropReason::ComponentDown)];
                }
                if !self.ports.contains(&ingress.port) {
                    return vec![Effect::Drop(DropReason::InvalidIngress(ingress.port))];
                }
                let packet = match addressed_packet(&ingress.frame, &self.addresses) {
                    Ok(packet) => packet,
                    Err(reason) => return vec![Effect::Drop(reason)],
                };
                if matches!(
                    packet.transport,
                    Transport::Icmp(IcmpMessage::EchoRequest { .. })
                ) {
                    return vec![Effect::Transmit {
                        egress: ingress.port,
                        next_hop: None,
                        frame: response_frame(ingress.frame, ApplicationData::None),
                        delay_ms: 0,
                    }];
                }
                let Some(service) = inferred_service(packet) else {
                    return vec![Effect::Drop(DropReason::UnsupportedProtocol)];
                };
                if self.services.contains(&service) {
                    vec![Effect::Deliver {
                        service,
                        detail: format!("{} accepted {service:?}", self.id),
                    }]
                } else {
                    vec![Effect::Drop(DropReason::ServiceUnavailable(service))]
                }
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

#[derive(Clone, Debug)]
pub struct DnsServer {
    id: ComponentId,
    ports: BTreeSet<PortId>,
    addresses: BTreeSet<Ipv4Addr>,
    records: BTreeMap<String, Ipv4Addr>,
    operational: bool,
}

impl DnsServer {
    pub fn new(
        id: ComponentId,
        ports: impl IntoIterator<Item = PortId>,
        addresses: impl IntoIterator<Item = Ipv4Addr>,
        records: impl IntoIterator<Item = (String, Ipv4Addr)>,
    ) -> Self {
        Self {
            id,
            ports: ports.into_iter().collect(),
            addresses: addresses.into_iter().collect(),
            records: records.into_iter().collect(),
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

    fn handle(&mut self, event: SimulationEvent) -> Vec<Effect> {
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.operational {
                    return vec![Effect::Drop(DropReason::ComponentDown)];
                }
                if !self.ports.contains(&ingress.port) {
                    return vec![Effect::Drop(DropReason::InvalidIngress(ingress.port))];
                }
                let packet = match addressed_packet(&ingress.frame, &self.addresses) {
                    Ok(packet) => packet,
                    Err(reason) => return vec![Effect::Drop(reason)],
                };
                let name = match &packet.application {
                    ApplicationData::DnsQuery { name } => name.clone(),
                    _ => {
                        return vec![Effect::Drop(DropReason::ServiceUnavailable(
                            ServiceKind::Dns,
                        ))];
                    }
                };
                let answer = self.records.get(&name).copied();
                vec![Effect::Transmit {
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
                }]
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

#[derive(Clone, Debug)]
pub struct ReverseProxyWaf {
    id: ComponentId,
    ports: BTreeSet<PortId>,
    addresses: BTreeSet<Ipv4Addr>,
    allowed_hosts: BTreeSet<String>,
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
        allowed_hosts: impl IntoIterator<Item = String>,
        upstream: ComponentId,
    ) -> Self {
        Self {
            id,
            ports: ports.into_iter().collect(),
            addresses: addresses.into_iter().collect(),
            allowed_hosts: allowed_hosts.into_iter().collect(),
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

    fn handle(&mut self, event: SimulationEvent) -> Vec<Effect> {
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.operational {
                    return vec![Effect::Drop(DropReason::ComponentDown)];
                }
                if !self.ports.contains(&ingress.port) {
                    return vec![Effect::Drop(DropReason::InvalidIngress(ingress.port))];
                }
                let packet = match addressed_packet(&ingress.frame, &self.addresses) {
                    Ok(packet) => packet,
                    Err(reason) => return vec![Effect::Drop(reason)],
                };
                let destination_port = packet.transport.destination_port();
                if destination_port == Some(80) && self.redirect_http {
                    return vec![Effect::Transmit {
                        egress: ingress.port,
                        next_hop: None,
                        frame: response_frame(
                            ingress.frame,
                            ApplicationData::HttpResponse { status: 308 },
                        ),
                        delay_ms: 0,
                    }];
                }
                if destination_port != Some(443) {
                    return vec![Effect::Drop(DropReason::ServiceUnavailable(
                        ServiceKind::Https,
                    ))];
                }
                let (method, host, path, body_bytes) = match &packet.application {
                    ApplicationData::HttpRequest {
                        method,
                        host,
                        path,
                        body_bytes,
                    } => (*method, host.clone(), path.clone(), *body_bytes),
                    _ => {
                        return vec![Effect::Drop(DropReason::ApplicationRejected(
                            "HTTPS request metadata is required".into(),
                        ))];
                    }
                };
                if !self.allowed_hosts.contains(&host) {
                    return vec![Effect::Drop(DropReason::ApplicationRejected(
                        "host is not published".into(),
                    ))];
                }
                if path.contains("..") {
                    return vec![Effect::Drop(DropReason::ApplicationRejected(
                        "path traversal pattern".into(),
                    ))];
                }
                if body_bytes > self.maximum_body_bytes {
                    return vec![Effect::Drop(DropReason::ApplicationRejected(
                        "request body exceeds configured limit".into(),
                    ))];
                }
                if !matches!(
                    method,
                    HttpMethod::Get | HttpMethod::Head | HttpMethod::Post
                ) {
                    return vec![Effect::Drop(DropReason::ApplicationRejected(
                        "HTTP method is not allowed".into(),
                    ))];
                }
                vec![Effect::ApplicationForward {
                    service: ServiceKind::Https,
                    target: self.upstream.clone(),
                    detail: format!("accepted HTTPS request for {host}{path}"),
                }]
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

#[derive(Clone, Debug)]
pub struct PassiveSensor {
    id: ComponentId,
    ports: BTreeSet<PortId>,
    operational: bool,
    observations: u64,
}

impl PassiveSensor {
    pub fn new(id: ComponentId, ports: impl IntoIterator<Item = PortId>) -> Self {
        Self {
            id,
            ports: ports.into_iter().collect(),
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

    fn handle(&mut self, event: SimulationEvent) -> Vec<Effect> {
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.operational {
                    return vec![Effect::Drop(DropReason::ComponentDown)];
                }
                if !self.ports.contains(&ingress.port) {
                    return vec![Effect::Drop(DropReason::InvalidIngress(ingress.port))];
                }
                self.observations += 1;
                vec![Effect::Observe {
                    detail: format!("passively observed frame {}", self.observations),
                }]
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
        EthernetFrame, Ipv4Packet, MacAddress, NetworkPayload, TcpSegment, UdpDatagram, VlanId,
    };

    use super::*;
    use crate::NetworkIngress;

    fn id(value: &str) -> ComponentId {
        ComponentId::new(value).expect("test ID")
    }

    fn port(value: &str) -> PortId {
        PortId::new(value).expect("test port")
    }

    fn request_frame(application: ApplicationData, transport: Transport) -> EthernetFrame {
        EthernetFrame {
            source: MacAddress::new([0, 1, 2, 3, 4, 5]),
            destination: MacAddress::new([0, 1, 2, 3, 4, 6]),
            vlan: VlanId::new(10).expect("VLAN"),
            payload: NetworkPayload::Ipv4(Ipv4Packet {
                source: Ipv4Addr::new(203, 0, 113, 2),
                destination: Ipv4Addr::new(172, 16, 10, 2),
                ttl: 64,
                transport,
                application,
            }),
        }
    }

    #[test]
    fn dns_returns_declared_record() {
        let mut dns = DnsServer::new(
            id("isp-dns-01"),
            [port("network")],
            [Ipv4Addr::new(198, 51, 100, 50)],
            [("www.business.example".into(), Ipv4Addr::new(192, 0, 2, 10))],
        );
        let mut frame = request_frame(
            ApplicationData::DnsQuery {
                name: "www.business.example".into(),
            },
            Transport::Udp(UdpDatagram {
                source_port: 50_000,
                destination_port: 53,
            }),
        );
        let NetworkPayload::Ipv4(packet) = &mut frame.payload else {
            panic!("IPv4");
        };
        packet.destination = Ipv4Addr::new(198, 51, 100, 50);

        let effects = dns.handle(SimulationEvent::Network(NetworkIngress {
            port: port("network"),
            frame,
        }));
        let Effect::Transmit { frame, .. } = &effects[0] else {
            panic!("DNS response");
        };
        let NetworkPayload::Ipv4(packet) = &frame.payload else {
            panic!("IPv4");
        };
        assert_eq!(
            packet.application,
            ApplicationData::DnsAnswer {
                name: "www.business.example".into(),
                address: Some(Ipv4Addr::new(192, 0, 2, 10)),
            }
        );
    }

    #[test]
    fn reverse_proxy_rejects_unknown_host_and_forwards_allowed_host() {
        let mut proxy = ReverseProxyWaf::new(
            id("business-web-gw-01"),
            [port("dmz")],
            [Ipv4Addr::new(172, 16, 10, 2)],
            ["www.business.example".into()],
            id("internal-app-vip"),
        );
        let request = |host: &str| {
            request_frame(
                ApplicationData::HttpRequest {
                    method: HttpMethod::Get,
                    host: host.into(),
                    path: "/shop".into(),
                    body_bytes: 0,
                },
                Transport::Tcp(TcpSegment {
                    source_port: 50_000,
                    destination_port: 443,
                    flags: TcpFlags {
                        syn: true,
                        ..TcpFlags::default()
                    },
                }),
            )
        };

        let denied = proxy.handle(SimulationEvent::Network(NetworkIngress {
            port: port("dmz"),
            frame: request("invalid.example"),
        }));
        assert!(matches!(
            denied[0],
            Effect::Drop(DropReason::ApplicationRejected(_))
        ));

        let allowed = proxy.handle(SimulationEvent::Network(NetworkIngress {
            port: port("dmz"),
            frame: request("www.business.example"),
        }));
        assert!(matches!(allowed[0], Effect::ApplicationForward { .. }));
    }
}
