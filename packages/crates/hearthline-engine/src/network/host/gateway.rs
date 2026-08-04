use core::net::Ipv4Addr;

use heapless::Vec as FixedList;
use hearthline_model::{
    ApplicationData, ComponentId, ComponentKind, EthernetFrame, HttpMethod, Ipv4Packet,
    NetworkPayload, PortId, Route, ServiceKind, TcpFlags, TcpSegment, Text, Transport,
};

use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{
    DropReason, Effect, EffectList, Ipv4Egress, RoutedInterface, SimulatedComponent,
    SimulationEvent,
};

use super::stack::{EndpointReceive, EndpointStack, response_frame};

const PENDING_REQUEST_CAPACITY: usize = 8;
const FIRST_UPSTREAM_PORT: u16 = 49_152;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpInspectionTarget {
    Path,
    Body,
}

#[derive(Clone, Debug)]
pub struct HttpInspectionRule {
    target: HttpInspectionTarget,
    pattern: Text<96>,
    case_sensitive: bool,
    reason: Text<96>,
}

impl HttpInspectionRule {
    pub fn new(
        target: HttpInspectionTarget,
        pattern: Text<96>,
        case_sensitive: bool,
        reason: Text<96>,
    ) -> Self {
        Self {
            target,
            pattern,
            case_sensitive,
            reason,
        }
    }

    fn rejection(&self, path: &Text<192>, body: Option<&Text<256>>) -> Option<DropReason> {
        let inspected = match self.target {
            HttpInspectionTarget::Path => path.as_str(),
            HttpInspectionTarget::Body => body?.as_str(),
        };
        contains_pattern(inspected, &self.pattern, self.case_sensitive)
            .then(|| DropReason::ApplicationRejected(self.reason.clone()))
    }
}

#[derive(Clone, Debug)]
struct PendingRequest {
    upstream_port: u16,
    client_interface: RoutedInterface,
    client_frame: EthernetFrame,
}

#[derive(Clone, Debug)]
pub struct ReverseProxyWaf {
    id: ComponentId,
    network: EndpointStack,
    allowed_hosts: FixedList<Text<128>, 8>,
    allowed_methods: FixedList<HttpMethod, 8>,
    inspection_rules: FixedList<HttpInspectionRule, 16>,
    upstream: ComponentId,
    upstream_address: Ipv4Addr,
    pending: FixedList<PendingRequest, PENDING_REQUEST_CAPACITY>,
    next_upstream_port: u16,
    maximum_body_bytes: usize,
    redirect_http: bool,
    operational: bool,
}

impl ReverseProxyWaf {
    pub fn new(
        id: ComponentId,
        interfaces: impl IntoIterator<Item = RoutedInterface>,
        allowed_hosts: impl IntoIterator<Item = Text<128>>,
        upstream: ComponentId,
        upstream_address: Ipv4Addr,
    ) -> Self {
        Self::with_routes(
            id,
            interfaces,
            None,
            [],
            allowed_hosts,
            upstream,
            upstream_address,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_routes(
        id: ComponentId,
        interfaces: impl IntoIterator<Item = RoutedInterface>,
        default_gateway: Option<Ipv4Addr>,
        routes: impl IntoIterator<Item = Route>,
        allowed_hosts: impl IntoIterator<Item = Text<128>>,
        upstream: ComponentId,
        upstream_address: Ipv4Addr,
    ) -> Self {
        Self {
            id,
            network: EndpointStack::with_routes(interfaces, default_gateway, routes),
            allowed_hosts: collect_fixed(allowed_hosts),
            allowed_methods: collect_fixed([HttpMethod::Get, HttpMethod::Head, HttpMethod::Post]),
            inspection_rules: FixedList::new(),
            upstream,
            upstream_address,
            pending: FixedList::new(),
            next_upstream_port: FIRST_UPSTREAM_PORT,
            maximum_body_bytes: 1_048_576,
            redirect_http: true,
            operational: true,
        }
    }

    pub fn set_maximum_body_bytes(&mut self, maximum: usize) {
        self.maximum_body_bytes = maximum;
    }

    pub fn set_allowed_methods(&mut self, methods: impl IntoIterator<Item = HttpMethod>) {
        self.allowed_methods = collect_fixed(methods);
    }

    pub fn set_inspection_rules(&mut self, rules: impl IntoIterator<Item = HttpInspectionRule>) {
        self.inspection_rules = collect_fixed(rules);
    }

    pub fn set_redirect_http(&mut self, enabled: bool) {
        self.redirect_http = enabled;
    }

    fn handle_network(&mut self, event: crate::NetworkIngress) -> EffectList {
        if !self.operational {
            return single_effect(Effect::Drop(DropReason::ComponentDown));
        }
        let received_at_us = event.received_at_us;
        let (interface, frame) = match self.network.receive(event) {
            EndpointReceive::Handled(effects) => return effects,
            EndpointReceive::Ipv4 { interface, frame } => (interface, frame),
        };
        let NetworkPayload::Ipv4(packet) = &frame.payload else {
            return single_effect(Effect::Drop(DropReason::UnsupportedProtocol));
        };
        if let ApplicationData::HttpResponse { .. } = &packet.application {
            return self.relay_response(interface, frame);
        }

        let destination_port = packet.transport.destination_port();
        if destination_port == Some(80) && self.redirect_http {
            return single_effect(Effect::Transmit {
                egress: interface.id.clone(),
                next_hop: None,
                frame: response_frame(
                    &interface,
                    frame,
                    ApplicationData::HttpResponse {
                        status: 308,
                        document: None,
                    },
                ),
                delay_ms: 0,
            });
        }
        if destination_port != Some(443) {
            return single_effect(Effect::Drop(DropReason::ServiceUnavailable(
                ServiceKind::Https,
            )));
        }
        let (method, host, path, body, body_bytes) = match &packet.application {
            ApplicationData::HttpRequest {
                method,
                host,
                path,
                body,
                body_bytes,
            } => (
                *method,
                host.clone(),
                path.clone(),
                body.clone(),
                *body_bytes,
            ),
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
        if body_bytes > self.maximum_body_bytes {
            return single_effect(Effect::Drop(DropReason::ApplicationRejected(
                "request body exceeds configured limit".into(),
            )));
        }
        if !self.allowed_methods.contains(&method) {
            return single_effect(Effect::Drop(DropReason::ApplicationRejected(
                "HTTP method is not allowed".into(),
            )));
        }
        if let Some(reason) = self
            .inspection_rules
            .iter()
            .find_map(|rule| rule.rejection(&path, body.as_ref()))
        {
            return single_effect(Effect::Drop(reason));
        }
        self.forward_request(
            interface,
            frame,
            ApplicationData::HttpRequest {
                method,
                host,
                path,
                body,
                body_bytes,
            },
            received_at_us,
        )
    }

    fn forward_request(
        &mut self,
        interface: RoutedInterface,
        frame: EthernetFrame,
        application: ApplicationData,
        sent_at_us: u64,
    ) -> EffectList {
        let Some(source) = interface.primary_address() else {
            return single_effect(Effect::Drop(DropReason::NoInterfaceAddress(interface.id)));
        };
        if self.pending.is_full() {
            return single_effect(Effect::Drop(DropReason::QueueLimit));
        }
        let upstream_port = self.next_upstream_port;
        self.next_upstream_port = if upstream_port == u16::MAX {
            FIRST_UPSTREAM_PORT
        } else {
            upstream_port + 1
        };
        self.pending
            .push(PendingRequest {
                upstream_port,
                client_interface: interface,
                client_frame: frame.clone(),
            })
            .expect("checked pending proxy capacity");
        let detail = match &application {
            ApplicationData::HttpRequest { host, path, .. } => runtime_text(format_args!(
                "proxied HTTPS request for {host}{path} to {} ({})",
                self.upstream, self.upstream_address
            )),
            _ => runtime_text(format_args!(
                "proxied HTTPS request to {} ({})",
                self.upstream, self.upstream_address
            )),
        };
        let mut effects = single_effect(Effect::ApplicationForward {
            service: ServiceKind::Https,
            target: self.upstream.clone(),
            detail,
        });
        append_effects(
            &mut effects,
            self.network.send(Ipv4Egress {
                packet: Ipv4Packet {
                    source,
                    destination: self.upstream_address,
                    ttl: 64,
                    transport: Transport::Tcp(TcpSegment {
                        source_port: upstream_port,
                        destination_port: 443,
                        flags: TcpFlags {
                            syn: true,
                            ..TcpFlags::default()
                        },
                    }),
                    application,
                },
                wire_len_bytes: frame.wire_len_bytes,
                sent_at_us,
            }),
        );
        effects
    }

    fn relay_response(
        &mut self,
        _upstream_interface: RoutedInterface,
        frame: EthernetFrame,
    ) -> EffectList {
        let NetworkPayload::Ipv4(packet) = &frame.payload else {
            return single_effect(Effect::Drop(DropReason::UnsupportedProtocol));
        };
        if packet.source != self.upstream_address || packet.transport.source_port() != Some(443) {
            return single_effect(Effect::Drop(DropReason::ApplicationRejected(
                "response is not from the configured upstream".into(),
            )));
        }
        let Some(upstream_port) = packet.transport.destination_port() else {
            return single_effect(Effect::Drop(DropReason::UnsupportedProtocol));
        };
        let Some(index) = self
            .pending
            .iter()
            .position(|pending| pending.upstream_port == upstream_port)
        else {
            return single_effect(Effect::Drop(DropReason::InvalidTcpState));
        };
        let pending = self.pending.swap_remove(index);
        let application = packet.application.clone();
        single_effect(Effect::Transmit {
            egress: pending.client_interface.id.clone(),
            next_hop: None,
            frame: response_frame(&pending.client_interface, pending.client_frame, application),
            delay_ms: 0,
        })
    }
}

fn contains_pattern(value: &str, pattern: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        return value.contains(pattern);
    }
    let pattern = pattern.as_bytes();
    !pattern.is_empty()
        && value
            .as_bytes()
            .windows(pattern.len())
            .any(|candidate| candidate.eq_ignore_ascii_case(pattern))
}

impl SimulatedComponent for ReverseProxyWaf {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::ReverseProxyWaf
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.network.has_port(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::Network(event) => self.handle_network(event),
            SimulationEvent::Ipv4Egress(egress) => {
                if !self.operational {
                    return single_effect(Effect::Drop(DropReason::ComponentDown));
                }
                self.network.send(egress)
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                if !operational {
                    self.pending.clear();
                }
                single_effect(Effect::Observe {
                    detail: runtime_text(format_args!("operational={operational}")),
                })
            }
            SimulationEvent::Process(_) | SimulationEvent::FirewallHa(_) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
        }
    }
}

fn append_effects(target: &mut EffectList, source: EffectList) {
    for effect in source {
        target
            .push(effect)
            .expect("combined gateway effects exceed capacity");
    }
}
