use std::net::Ipv4Addr;

use hearthline_model::{
    ApplicationData, ComponentId, HttpMethod, IcmpMessage, Ipv4Packet, TcpFlags, TcpSegment, Text,
    Transport, UdpDatagram,
};
use serde::{Deserialize, Serialize};

use crate::ConfigError;
use crate::runtime::parse_service_kind;

use super::require_value;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioPacketConfig {
    pub source_ip: String,
    pub destination_ip: String,
    pub ttl: u8,
    pub wire_length_bytes: u16,
    pub transport: ScenarioTransportConfig,
    pub application: ScenarioApplicationConfig,
}

impl ScenarioPacketConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        let source = parse_address(&self.source_ip, "packet source")?;
        let destination = parse_address(&self.destination_ip, "packet destination")?;
        if !usable_unicast(source) || !usable_unicast(destination) {
            return Err(ConfigError::new(
                "scenario packet requires usable unicast source and destination addresses",
            ));
        }
        if self.ttl == 0 {
            return Err(ConfigError::new("scenario packet TTL must be non-zero"));
        }
        if self.wire_length_bytes < 64 {
            return Err(ConfigError::new(
                "scenario packet wire_length_bytes must be at least 64",
            ));
        }
        self.transport.validate()?;
        self.application.validate()?;
        if matches!(self.application, ScenarioApplicationConfig::DnsQuery { .. })
            && self.transport.destination_port() != Some(53)
        {
            return Err(ConfigError::new(
                "DNS query scenarios must use destination port 53",
            ));
        }
        if matches!(
            self.application,
            ScenarioApplicationConfig::HttpRequest { .. }
        ) && !matches!(self.transport.destination_port(), Some(80 | 443))
        {
            return Err(ConfigError::new(
                "HTTP request scenarios must use destination port 80 or 443",
            ));
        }
        Ok(())
    }

    pub fn ipv4_packet(&self) -> Result<Ipv4Packet, ConfigError> {
        self.validate()?;
        Ok(Ipv4Packet {
            source: parse_address(&self.source_ip, "packet source")?,
            destination: parse_address(&self.destination_ip, "packet destination")?,
            ttl: self.ttl,
            transport: self.transport.runtime(),
            application: self.application.runtime()?,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "protocol", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ScenarioTransportConfig {
    Udp {
        source_port: u16,
        destination_port: u16,
    },
    Tcp {
        source_port: u16,
        destination_port: u16,
        #[serde(default)]
        syn: bool,
        #[serde(default)]
        ack: bool,
        #[serde(default)]
        fin: bool,
        #[serde(default)]
        rst: bool,
    },
    IcmpEcho {
        identifier: u16,
        sequence: u16,
    },
}

impl ScenarioTransportConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        match self {
            Self::Udp {
                source_port,
                destination_port,
            }
            | Self::Tcp {
                source_port,
                destination_port,
                ..
            } if *source_port == 0 || *destination_port == 0 => Err(ConfigError::new(
                "scenario TCP and UDP ports must be non-zero",
            )),
            _ => Ok(()),
        }
    }

    fn destination_port(&self) -> Option<u16> {
        match self {
            Self::Udp {
                destination_port, ..
            }
            | Self::Tcp {
                destination_port, ..
            } => Some(*destination_port),
            Self::IcmpEcho { .. } => None,
        }
    }

    fn runtime(&self) -> Transport {
        match *self {
            Self::Udp {
                source_port,
                destination_port,
            } => Transport::Udp(UdpDatagram {
                source_port,
                destination_port,
            }),
            Self::Tcp {
                source_port,
                destination_port,
                syn,
                ack,
                fin,
                rst,
            } => Transport::Tcp(TcpSegment {
                source_port,
                destination_port,
                flags: TcpFlags { syn, ack, fin, rst },
            }),
            Self::IcmpEcho {
                identifier,
                sequence,
            } => Transport::Icmp(IcmpMessage::EchoRequest {
                identifier,
                sequence,
            }),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ScenarioApplicationConfig {
    None,
    DnsQuery {
        name: String,
    },
    HttpRequest {
        method: ScenarioHttpMethod,
        host: String,
        path: String,
        #[serde(default)]
        body: Option<String>,
        body_bytes: usize,
    },
    Telemetry {
        service: String,
        source: String,
        sequence: u64,
        payload: String,
    },
    Service {
        service: String,
    },
}

impl ScenarioApplicationConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        if let Self::DnsQuery { name } = self {
            require_value("DNS query name", name)?;
            Text::<128>::try_new(name).map_err(|error| ConfigError::new(error.to_string()))?;
        }
        if let Self::Service { service } = self {
            parse_service_kind(service)?;
        }
        if let Self::Telemetry {
            service,
            source,
            payload,
            ..
        } = self
        {
            parse_service_kind(service)?;
            ComponentId::new(source).map_err(|error| ConfigError::new(error.to_string()))?;
            require_value("telemetry payload", payload)?;
            Text::<256>::try_new(payload).map_err(|error| ConfigError::new(error.to_string()))?;
        }
        if let Self::HttpRequest {
            host,
            path,
            body,
            body_bytes,
            ..
        } = self
        {
            require_value("HTTP host", host)?;
            require_value("HTTP path", path)?;
            if !path.starts_with('/') {
                return Err(ConfigError::new("HTTP request path must start with /"));
            }
            Text::<128>::try_new(host).map_err(|error| ConfigError::new(error.to_string()))?;
            Text::<192>::try_new(path).map_err(|error| ConfigError::new(error.to_string()))?;
            if let Some(body) = body {
                Text::<256>::try_new(body).map_err(|error| ConfigError::new(error.to_string()))?;
                if *body_bytes != body.len() {
                    return Err(ConfigError::new(
                        "HTTP request body_bytes must match the configured body length",
                    ));
                }
            }
        }
        Ok(())
    }

    fn runtime(&self) -> Result<ApplicationData, ConfigError> {
        match self {
            Self::None => Ok(ApplicationData::None),
            Self::DnsQuery { name } => Ok(ApplicationData::DnsQuery {
                name: Text::try_new(name).map_err(|error| ConfigError::new(error.to_string()))?,
            }),
            Self::HttpRequest {
                method,
                host,
                path,
                body,
                body_bytes,
            } => Ok(ApplicationData::HttpRequest {
                method: method.runtime(),
                host: Text::try_new(host).map_err(|error| ConfigError::new(error.to_string()))?,
                path: Text::try_new(path).map_err(|error| ConfigError::new(error.to_string()))?,
                body: body
                    .as_deref()
                    .map(Text::try_new)
                    .transpose()
                    .map_err(|error| ConfigError::new(error.to_string()))?,
                body_bytes: *body_bytes,
            }),
            Self::Telemetry {
                service,
                source,
                sequence,
                payload,
            } => Ok(ApplicationData::Telemetry {
                service: parse_service_kind(service)?,
                source: ComponentId::new(source)
                    .map_err(|error| ConfigError::new(error.to_string()))?,
                sequence: *sequence,
                payload: Text::try_new(payload)
                    .map_err(|error| ConfigError::new(error.to_string()))?,
            }),
            Self::Service { service } => Ok(ApplicationData::Service(parse_service_kind(service)?)),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScenarioHttpMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
    Options,
}

impl ScenarioHttpMethod {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Options => "OPTIONS",
        }
    }

    const fn runtime(self) -> HttpMethod {
        match self {
            Self::Get => HttpMethod::Get,
            Self::Head => HttpMethod::Head,
            Self::Post => HttpMethod::Post,
            Self::Put => HttpMethod::Put,
            Self::Patch => HttpMethod::Patch,
            Self::Delete => HttpMethod::Delete,
            Self::Options => HttpMethod::Options,
        }
    }
}

fn parse_address(value: &str, field: &str) -> Result<Ipv4Addr, ConfigError> {
    value
        .parse()
        .map_err(|_| ConfigError::new(format!("{field} address {value} is invalid")))
}

fn usable_unicast(address: Ipv4Addr) -> bool {
    !address.is_unspecified()
        && !address.is_loopback()
        && !address.is_multicast()
        && address != Ipv4Addr::BROADCAST
}
