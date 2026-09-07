use crate::{
    ApplicationData, ArpOperation, ComponentId, EthernetFrame, FirewallHaMessage, HttpMethod,
    IcmpMessage, NetworkPayload, PortId, ProcessEvent, ProcessSignal, SignalValue,
    TELEMETRY_PAYLOAD_CAPACITY, Text, Transport,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PartitionMessageClass {
    Network,
    Process,
    Telemetry,
    Heartbeat,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum PartitionMessage {
    Network {
        target: ComponentId,
        ingress: PortId,
        frame: EthernetFrame,
    },
    Process {
        target: ComponentId,
        event: ProcessEvent,
    },
    Telemetry {
        source: ComponentId,
        sequence: u64,
        payload: Text<TELEMETRY_PAYLOAD_CAPACITY>,
    },
    Heartbeat {
        source: ComponentId,
        counter: u64,
    },
}

impl PartitionMessage {
    pub const fn class(&self) -> PartitionMessageClass {
        match self {
            Self::Network { .. } => PartitionMessageClass::Network,
            Self::Process { .. } => PartitionMessageClass::Process,
            Self::Telemetry { .. } => PartitionMessageClass::Telemetry,
            Self::Heartbeat { .. } => PartitionMessageClass::Heartbeat,
        }
    }

    /// Feeds a stable, allocation-free representation of the complete message to `sink`.
    ///
    /// This is the hashing boundary used by snapshots and replay artifacts. Variant tags and
    /// length-prefixed text make different messages unambiguous without depending on `Debug`
    /// formatting or a host serialization format.
    pub fn visit_canonical_bytes(&self, mut sink: impl FnMut(&[u8])) {
        let mut encoder = CanonicalEncoder { sink: &mut sink };
        encoder.message(self);
    }
}

struct CanonicalEncoder<'a, F> {
    sink: &'a mut F,
}

impl<F: FnMut(&[u8])> CanonicalEncoder<'_, F> {
    fn bytes(&mut self, value: &[u8]) {
        (self.sink)(value);
    }

    fn tag(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    fn bool(&mut self, value: bool) {
        self.tag(u8::from(value));
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.bytes(&value.to_le_bytes());
    }

    fn usize(&mut self, value: usize) {
        self.u64(value as u64);
    }

    fn text(&mut self, value: &str) {
        self.usize(value.len());
        self.bytes(value.as_bytes());
    }

    fn component(&mut self, value: &ComponentId) {
        self.text(value.as_str());
    }

    fn port(&mut self, value: &PortId) {
        self.text(value.as_str());
    }

    fn message(&mut self, message: &PartitionMessage) {
        match message {
            PartitionMessage::Network {
                target,
                ingress,
                frame,
            } => {
                self.tag(0);
                self.component(target);
                self.port(ingress);
                self.frame(frame);
            }
            PartitionMessage::Process { target, event } => {
                self.tag(1);
                self.component(target);
                self.process_event(event);
            }
            PartitionMessage::Telemetry {
                source,
                sequence,
                payload,
            } => {
                self.tag(2);
                self.component(source);
                self.u64(*sequence);
                self.text(payload.as_str());
            }
            PartitionMessage::Heartbeat { source, counter } => {
                self.tag(3);
                self.component(source);
                self.u64(*counter);
            }
        }
    }

    fn frame(&mut self, frame: &EthernetFrame) {
        self.bytes(&frame.source.bytes());
        self.bytes(&frame.destination.bytes());
        self.u16(frame.vlan.get());
        self.u16(frame.wire_len_bytes);
        self.network_payload(&frame.payload);
    }

    fn network_payload(&mut self, payload: &NetworkPayload) {
        match payload {
            NetworkPayload::Arp(packet) => {
                self.tag(0);
                self.tag(match packet.operation {
                    ArpOperation::Request => 0,
                    ArpOperation::Reply => 1,
                });
                self.bytes(&packet.sender_mac.bytes());
                self.bytes(&packet.sender_ip.octets());
                match packet.target_mac {
                    Some(address) => {
                        self.tag(1);
                        self.bytes(&address.bytes());
                    }
                    None => self.tag(0),
                }
                self.bytes(&packet.target_ip.octets());
            }
            NetworkPayload::FirewallHa(message) => {
                self.tag(1);
                self.firewall_ha(message);
            }
            NetworkPayload::Ipv4(packet) => {
                self.tag(2);
                self.bytes(&packet.source.octets());
                self.bytes(&packet.destination.octets());
                self.tag(packet.ttl);
                self.transport(packet.transport);
                self.application(&packet.application);
            }
        }
    }

    fn firewall_ha(&mut self, message: &FirewallHaMessage) {
        match message {
            FirewallHaMessage::Heartbeat {
                domain,
                sequence,
                sent_at_us,
            } => {
                self.tag(0);
                self.text(domain.as_str());
                self.u64(*sequence);
                self.u64(*sent_at_us);
            }
            FirewallHaMessage::SessionUpsert {
                domain,
                generation,
                flow,
                expires_at_us,
            } => {
                self.tag(1);
                self.text(domain.as_str());
                self.u64(*generation);
                self.bytes(&flow.source.octets());
                self.bytes(&flow.destination.octets());
                match flow.protocol {
                    crate::TransportProtocol::Icmp => self.tag(0),
                    crate::TransportProtocol::Tcp => self.tag(1),
                    crate::TransportProtocol::Udp => self.tag(2),
                    crate::TransportProtocol::Other(value) => {
                        self.tag(3);
                        self.tag(value);
                    }
                }
                self.optional_u16(flow.source_port);
                self.optional_u16(flow.destination_port);
                self.u64(*expires_at_us);
            }
        }
    }

    fn optional_u16(&mut self, value: Option<u16>) {
        match value {
            Some(value) => {
                self.tag(1);
                self.u16(value);
            }
            None => self.tag(0),
        }
    }

    fn transport(&mut self, transport: Transport) {
        match transport {
            Transport::Icmp(message) => {
                self.tag(0);
                match message {
                    IcmpMessage::EchoRequest {
                        identifier,
                        sequence,
                    } => {
                        self.tag(0);
                        self.u16(identifier);
                        self.u16(sequence);
                    }
                    IcmpMessage::EchoReply {
                        identifier,
                        sequence,
                    } => {
                        self.tag(1);
                        self.u16(identifier);
                        self.u16(sequence);
                    }
                    IcmpMessage::DestinationUnreachable => self.tag(2),
                    IcmpMessage::TimeExceeded => self.tag(3),
                }
            }
            Transport::Tcp(segment) => {
                self.tag(1);
                self.u16(segment.source_port);
                self.u16(segment.destination_port);
                self.bool(segment.flags.syn);
                self.bool(segment.flags.ack);
                self.bool(segment.flags.fin);
                self.bool(segment.flags.rst);
            }
            Transport::Udp(datagram) => {
                self.tag(2);
                self.u16(datagram.source_port);
                self.u16(datagram.destination_port);
            }
            Transport::Other(value) => {
                self.tag(3);
                self.tag(value);
            }
        }
    }

    fn application(&mut self, application: &ApplicationData) {
        match application {
            ApplicationData::None => self.tag(0),
            ApplicationData::DnsQuery { name } => {
                self.tag(1);
                self.text(name.as_str());
            }
            ApplicationData::DnsAnswer { name, address } => {
                self.tag(2);
                self.text(name.as_str());
                match address {
                    Some(address) => {
                        self.tag(1);
                        self.bytes(&address.octets());
                    }
                    None => self.tag(0),
                }
            }
            ApplicationData::HttpRequest {
                method,
                host,
                path,
                body,
                body_bytes,
            } => {
                self.tag(3);
                self.tag(http_method_tag(*method));
                self.text(host.as_str());
                self.text(path.as_str());
                match body {
                    Some(body) => {
                        self.tag(1);
                        self.text(body.as_str());
                    }
                    None => self.tag(0),
                }
                self.usize(*body_bytes);
            }
            ApplicationData::HttpResponse { status, document } => {
                self.tag(4);
                self.u16(*status);
                match document {
                    Some(document) => {
                        self.tag(1);
                        self.text(document.title.as_str());
                        self.text(document.heading.as_str());
                        self.text(document.body.as_str());
                    }
                    None => self.tag(0),
                }
            }
            ApplicationData::Telemetry {
                service,
                source,
                sequence,
                payload,
            } => {
                self.tag(5);
                self.tag(*service as u8);
                self.component(source);
                self.u64(*sequence);
                self.text(payload.as_str());
            }
            ApplicationData::Service(service) => {
                self.tag(6);
                self.tag(*service as u8);
            }
        }
    }

    fn process_event(&mut self, event: &ProcessEvent) {
        match event {
            ProcessEvent::Tick { elapsed_ms } => {
                self.tag(0);
                self.u64(*elapsed_ms);
            }
            ProcessEvent::Signal(signal) => {
                self.tag(1);
                self.process_signal(signal);
            }
            ProcessEvent::Command(command) => {
                self.tag(2);
                self.text(command.tag.as_str());
                self.signal_value(&command.value);
                self.text(command.source.as_str());
            }
            ProcessEvent::Trip { cause } => {
                self.tag(3);
                self.text(cause.as_str());
            }
            ProcessEvent::Reset { authorized } => {
                self.tag(4);
                self.bool(*authorized);
            }
        }
    }

    fn process_signal(&mut self, signal: &ProcessSignal) {
        self.text(signal.tag.as_str());
        self.signal_value(&signal.value);
        self.bool(signal.quality_good);
        self.u64(signal.timestamp_ms);
    }

    fn signal_value(&mut self, value: &SignalValue) {
        match value {
            SignalValue::Bool(value) => {
                self.tag(0);
                self.bool(*value);
            }
            SignalValue::Analog(value) => {
                self.tag(1);
                self.i64(value.raw());
            }
            SignalValue::Integer(value) => {
                self.tag(2);
                self.i64(*value);
            }
            SignalValue::Text(value) => {
                self.tag(3);
                self.text(value.as_str());
            }
        }
    }
}

const fn http_method_tag(method: HttpMethod) -> u8 {
    match method {
        HttpMethod::Get => 0,
        HttpMethod::Head => 1,
        HttpMethod::Post => 2,
        HttpMethod::Put => 3,
        HttpMethod::Patch => 4,
        HttpMethod::Delete => 5,
        HttpMethod::Options => 6,
    }
}
