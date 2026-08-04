use std::fmt::{self, Display, Formatter};

use hearthline_model::ComponentId;
use serde::{Deserialize, Deserializer};

use crate::appliance::{ConfigError, ConfigRepository, Lifecycle};
use hearthline_engine::{
    CarrierMedium, ConnectionMedium, CopperMedium, FiberMedium, FieldWiringMedium, MediaLink,
    MediumKind, RadioMedium, TelephoneMedium, VirtualMedium,
};

use super::{build_media_link, default_capacity, default_true};

pub const CONNECTION_SCHEMA_VERSION: &str = "0.2.0";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionConfig {
    pub schema_version: String,
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub lifecycle: Lifecycle,
    pub transport: TransportKind,
    #[serde(deserialize_with = "deserialize_connection_medium")]
    pub medium: ConnectionMedium,
    pub endpoints: ConnectionEndpoints,
    #[serde(default)]
    pub properties: ConnectionProperties,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl ConnectionConfig {
    pub fn from_yaml(source: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_yaml_ng::from_str(source)
            .map_err(|error| ConfigError::new(format!("invalid connection YAML: {error}")))?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != CONNECTION_SCHEMA_VERSION {
            return Err(ConfigError::new(format!(
                "connection {} uses schema {}, expected {}",
                self.id, self.schema_version, CONNECTION_SCHEMA_VERSION
            )));
        }
        ComponentId::new(&self.id).map_err(|error| ConfigError::new(error.to_string()))?;
        if self.label.trim().is_empty() {
            return Err(ConfigError::new("connection label cannot be empty"));
        }
        for endpoint in [&self.endpoints.a, &self.endpoints.b] {
            ComponentId::new(&endpoint.appliance)
                .map_err(|error| ConfigError::new(error.to_string()))?;
            ComponentId::new(&endpoint.interface)
                .map_err(|error| ConfigError::new(error.to_string()))?;
        }
        if self.endpoints.a == self.endpoints.b {
            return Err(ConfigError::new(format!(
                "connection {} cannot join an endpoint to itself",
                self.id
            )));
        }
        if self.properties.capacity_mbps == 0 {
            return Err(ConfigError::new(format!(
                "connection {} requires non-zero capacity",
                self.id
            )));
        }
        if self.properties.loss_every == Some(0) {
            return Err(ConfigError::new(format!(
                "connection {} loss interval cannot be zero",
                self.id
            )));
        }
        if !self.transport.accepts(self.medium.kind()) {
            return Err(ConfigError::new(format!(
                "connection {} transport {} is incompatible with {} medium",
                self.id,
                self.transport,
                self.medium.kind()
            )));
        }
        self.medium
            .validate()
            .map_err(|error| ConfigError::new(format!("connection {} medium: {error}", self.id)))?;
        if self
            .medium
            .max_capacity_mbps()
            .is_some_and(|maximum| self.properties.capacity_mbps > maximum)
        {
            return Err(ConfigError::new(format!(
                "connection {} capacity {} Mbps exceeds {} medium limit",
                self.id,
                self.properties.capacity_mbps,
                self.medium.kind()
            )));
        }
        Ok(())
    }

    pub fn media_link(&self, appliances: &ConfigRepository) -> Result<MediaLink, ConfigError> {
        build_media_link(self, appliances)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum TransportKind {
    Ethernet,
    WirelessLan,
    WideArea,
    FieldIo,
    Virtual,
    Mirror,
    EncryptedIp,
    AnalogTelephone,
}

impl TransportKind {
    const fn accepts(self, medium: MediumKind) -> bool {
        matches!(
            (self, medium),
            (Self::Ethernet, MediumKind::Copper | MediumKind::Fiber)
                | (Self::WirelessLan, MediumKind::Radio)
                | (Self::WideArea, MediumKind::Carrier)
                | (Self::FieldIo, MediumKind::FieldWiring)
                | (Self::Virtual, MediumKind::Virtual)
                | (Self::Mirror, MediumKind::Copper | MediumKind::Fiber)
                | (
                    Self::EncryptedIp,
                    MediumKind::Copper
                        | MediumKind::Fiber
                        | MediumKind::Virtual
                        | MediumKind::Carrier
                )
                | (Self::AnalogTelephone, MediumKind::Telephone)
        )
    }
}

impl Display for TransportKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Ethernet => "ethernet",
            Self::WirelessLan => "wireless-lan",
            Self::WideArea => "wide-area",
            Self::FieldIo => "field-io",
            Self::Virtual => "virtual",
            Self::Mirror => "mirror",
            Self::EncryptedIp => "encrypted-ip",
            Self::AnalogTelephone => "analog-telephone",
        };
        formatter.write_str(value)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConnectionEndpoint {
    pub appliance: String,
    pub interface: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionEndpoints {
    pub a: ConnectionEndpoint,
    pub b: ConnectionEndpoint,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionProperties {
    #[serde(default = "default_capacity")]
    pub capacity_mbps: u64,
    #[serde(default)]
    pub latency_ms: u64,
    #[serde(default)]
    pub loss_every: Option<u64>,
    #[serde(default)]
    pub direction: ConnectionDirection,
    #[serde(default = "default_true")]
    pub operational: bool,
}

impl Default for ConnectionProperties {
    fn default() -> Self {
        Self {
            capacity_mbps: default_capacity(),
            latency_ms: 0,
            loss_every: None,
            direction: ConnectionDirection::Bidirectional,
            operational: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ConnectionDirection {
    #[default]
    Bidirectional,
    AToB,
    BToA,
}

impl Display for ConnectionDirection {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Bidirectional => "bidirectional",
            Self::AToB => "a-to-b",
            Self::BToA => "b-to-a",
        };
        formatter.write_str(value)
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
enum ParsedConnectionMedium {
    Copper {
        #[serde(flatten)]
        config: CopperMedium,
    },
    Fiber {
        #[serde(flatten)]
        config: FiberMedium,
    },
    Radio {
        #[serde(flatten)]
        config: RadioMedium,
    },
    Carrier {
        #[serde(flatten)]
        config: CarrierMedium,
    },
    Virtual {
        #[serde(flatten)]
        config: VirtualMedium,
    },
    FieldWiring {
        #[serde(flatten)]
        config: FieldWiringMedium,
    },
    Telephone {
        #[serde(flatten)]
        config: TelephoneMedium,
    },
}

fn deserialize_connection_medium<'de, D>(deserializer: D) -> Result<ConnectionMedium, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(match ParsedConnectionMedium::deserialize(deserializer)? {
        ParsedConnectionMedium::Copper { config } => ConnectionMedium::Copper { config },
        ParsedConnectionMedium::Fiber { config } => ConnectionMedium::Fiber { config },
        ParsedConnectionMedium::Radio { config } => ConnectionMedium::Radio { config },
        ParsedConnectionMedium::Carrier { config } => ConnectionMedium::Carrier { config },
        ParsedConnectionMedium::Virtual { config } => ConnectionMedium::Virtual { config },
        ParsedConnectionMedium::FieldWiring { config } => ConnectionMedium::FieldWiring { config },
        ParsedConnectionMedium::Telephone { config } => ConnectionMedium::Telephone { config },
    })
}
