use core::cmp::max;
use core::fmt::{self, Display, Formatter, Write as _};

use hearthline_model::{ComponentId, EthernetFrame, PortId, Text};

use super::media::{ConnectionMedium, MediumKind};
use super::port::{PortDuplex, PortState, SimulatedPort};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkEndpoint {
    pub component: ComponentId,
    pub port: PortId,
    pub profile: SimulatedPort,
}

impl LinkEndpoint {
    pub const fn is_operational(&self) -> bool {
        self.profile.is_operational()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LinkDirection {
    #[default]
    Bidirectional,
    AToB,
    BToA,
}

impl LinkDirection {
    const fn permits(self, source_is_a: bool) -> bool {
        matches!(
            (self, source_is_a),
            (Self::Bidirectional, _) | (Self::AToB, true) | (Self::BToA, false)
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MediaLinkConfig {
    pub capacity_mbps: u64,
    pub latency_ms: u64,
    pub loss_every: Option<u64>,
    pub direction: LinkDirection,
    pub operational: bool,
}

impl Default for MediaLinkConfig {
    fn default() -> Self {
        Self {
            capacity_mbps: 1_000,
            latency_ms: 0,
            loss_every: None,
            direction: LinkDirection::Bidirectional,
            operational: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MediaDropReason {
    Down,
    SourcePortDown,
    DestinationPortDown,
    InvalidEndpoint,
    DirectionDenied,
    InvalidFrameLength(u16),
    MtuExceeded { wire_bytes: u16, maximum: u32 },
    UnsupportedPayload(MediumKind),
    ModeledLoss,
}

impl Display for MediaDropReason {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Down => formatter.write_str("connection is down"),
            Self::SourcePortDown => formatter.write_str("source port is down"),
            Self::DestinationPortDown => formatter.write_str("destination port is down"),
            Self::InvalidEndpoint => formatter.write_str("source is not a connection endpoint"),
            Self::DirectionDenied => formatter.write_str("connection direction denies transit"),
            Self::InvalidFrameLength(bytes) => {
                write!(formatter, "invalid Ethernet frame length {bytes} bytes")
            }
            Self::MtuExceeded {
                wire_bytes,
                maximum,
            } => write!(
                formatter,
                "Ethernet frame length {wire_bytes} exceeds {maximum} bytes"
            ),
            Self::UnsupportedPayload(kind) => {
                write!(formatter, "{kind} medium does not carry Ethernet frames")
            }
            Self::ModeledLoss => formatter.write_str("frame lost by configured impairment"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaLinkError(Text<192>);

impl MediaLinkError {
    fn from_args(arguments: fmt::Arguments<'_>) -> Self {
        let mut message = Text::default();
        message
            .write_fmt(arguments)
            .expect("media link error exceeds fixed capacity");
        Self(message)
    }
}

impl Display for MediaLinkError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl core::error::Error for MediaLinkError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaTransit {
    pub destination_component: ComponentId,
    pub destination_port: PortId,
    pub departure_us: u64,
    pub arrival_us: u64,
    pub queue_delay_us: u64,
    pub serialization_us: u64,
    pub propagation_us: u64,
}

#[derive(Clone, Debug)]
pub struct MediaLink {
    id: ComponentId,
    endpoint_a: LinkEndpoint,
    endpoint_b: LinkEndpoint,
    config: MediaLinkConfig,
    medium: ConnectionMedium,
    effective_mtu: u32,
    duplex: PortDuplex,
    frame_count: u64,
    available_a_to_b_us: u64,
    available_b_to_a_us: u64,
    available_shared_us: u64,
}

impl MediaLink {
    // The fixed-capacity error text avoids allocator use in the engine API.
    #[allow(clippy::result_large_err)]
    pub fn new(
        id: ComponentId,
        endpoint_a: LinkEndpoint,
        endpoint_b: LinkEndpoint,
        config: MediaLinkConfig,
        medium: ConnectionMedium,
    ) -> Result<Self, MediaLinkError> {
        validate_link(&endpoint_a, &endpoint_b, config, &medium)?;
        let duplex = negotiated_duplex(
            endpoint_a.profile.settings.duplex,
            endpoint_b.profile.settings.duplex,
            medium.kind(),
        );
        Ok(Self {
            id,
            effective_mtu: endpoint_a
                .profile
                .settings
                .mtu
                .min(endpoint_b.profile.settings.mtu),
            endpoint_a,
            endpoint_b,
            config,
            medium,
            duplex,
            frame_count: 0,
            available_a_to_b_us: 0,
            available_b_to_a_us: 0,
            available_shared_us: 0,
        })
    }

    pub fn id(&self) -> &ComponentId {
        &self.id
    }

    pub const fn endpoints(&self) -> (&LinkEndpoint, &LinkEndpoint) {
        (&self.endpoint_a, &self.endpoint_b)
    }

    pub fn contains(&self, component: &ComponentId, port: &PortId) -> bool {
        endpoint_matches(&self.endpoint_a, component, port)
            || endpoint_matches(&self.endpoint_b, component, port)
    }

    pub fn requires_exclusive_endpoints(&self) -> bool {
        self.medium.requires_exclusive_endpoints()
    }

    pub fn set_operational(&mut self, operational: bool) {
        self.config.operational = operational;
    }

    pub fn set_port_operational(
        &mut self,
        component: &ComponentId,
        port: &PortId,
        operational: PortState,
    ) -> Result<(), MediaDropReason> {
        if endpoint_matches(&self.endpoint_a, component, port) {
            self.endpoint_a.profile.set_operational(operational);
        } else if endpoint_matches(&self.endpoint_b, component, port) {
            self.endpoint_b.profile.set_operational(operational);
        } else {
            return Err(MediaDropReason::InvalidEndpoint);
        }
        Ok(())
    }

    pub fn transmit(
        &mut self,
        source_component: &ComponentId,
        source_port: &PortId,
        frame: &EthernetFrame,
        ready_at_us: u64,
    ) -> Result<MediaTransit, MediaDropReason> {
        if !self.config.operational {
            return Err(MediaDropReason::Down);
        }
        let source_is_a = if endpoint_matches(&self.endpoint_a, source_component, source_port) {
            true
        } else if endpoint_matches(&self.endpoint_b, source_component, source_port) {
            false
        } else {
            return Err(MediaDropReason::InvalidEndpoint);
        };
        if !self.config.direction.permits(source_is_a) {
            return Err(MediaDropReason::DirectionDenied);
        }
        let (source, destination) = if source_is_a {
            (&self.endpoint_a, &self.endpoint_b)
        } else {
            (&self.endpoint_b, &self.endpoint_a)
        };
        if !source.is_operational() {
            return Err(MediaDropReason::SourcePortDown);
        }
        if !destination.is_operational() {
            return Err(MediaDropReason::DestinationPortDown);
        }
        if !frame.has_valid_wire_length() {
            return Err(MediaDropReason::InvalidFrameLength(frame.wire_len_bytes));
        }
        if matches!(
            self.medium.kind(),
            MediumKind::FieldWiring | MediumKind::Telephone
        ) {
            return Err(MediaDropReason::UnsupportedPayload(self.medium.kind()));
        }
        let maximum = self
            .effective_mtu
            .saturating_add(EthernetFrame::VLAN_OVERHEAD_BYTES);
        if u32::from(frame.wire_len_bytes) > maximum {
            return Err(MediaDropReason::MtuExceeded {
                wire_bytes: frame.wire_len_bytes,
                maximum,
            });
        }

        self.frame_count = self.frame_count.saturating_add(1);
        if self
            .config
            .loss_every
            .is_some_and(|interval| self.frame_count.is_multiple_of(interval))
        {
            return Err(MediaDropReason::ModeledLoss);
        }

        let serialization_us = u64::from(frame.wire_len_bytes)
            .saturating_add(20)
            .saturating_mul(8)
            .div_ceil(self.config.capacity_mbps);
        let available_us = if self.duplex == PortDuplex::Half {
            self.available_shared_us
        } else if source_is_a {
            self.available_a_to_b_us
        } else {
            self.available_b_to_a_us
        };
        let departure_us = max(ready_at_us, available_us);
        let serialized_us = departure_us.saturating_add(serialization_us);
        if self.duplex == PortDuplex::Half {
            self.available_shared_us = serialized_us;
        } else if source_is_a {
            self.available_a_to_b_us = serialized_us;
        } else {
            self.available_b_to_a_us = serialized_us;
        }
        let propagation_us = self.medium.propagation_delay_us();
        let arrival_us = serialized_us
            .saturating_add(self.config.latency_ms.saturating_mul(1_000))
            .saturating_add(propagation_us);
        Ok(MediaTransit {
            destination_component: destination.component.clone(),
            destination_port: destination.port.clone(),
            departure_us,
            arrival_us,
            queue_delay_us: departure_us.saturating_sub(ready_at_us),
            serialization_us,
            propagation_us,
        })
    }
}

fn endpoint_matches(endpoint: &LinkEndpoint, component: &ComponentId, port: &PortId) -> bool {
    &endpoint.component == component && &endpoint.port == port
}

fn negotiated_duplex(a: PortDuplex, b: PortDuplex, medium: MediumKind) -> PortDuplex {
    if medium == MediumKind::Radio || a == PortDuplex::Half || b == PortDuplex::Half {
        PortDuplex::Half
    } else {
        PortDuplex::Full
    }
}

#[allow(clippy::result_large_err)]
fn validate_link(
    a: &LinkEndpoint,
    b: &LinkEndpoint,
    config: MediaLinkConfig,
    medium: &ConnectionMedium,
) -> Result<(), MediaLinkError> {
    if a.component == b.component && a.port == b.port {
        return Err(MediaLinkError::from_args(format_args!(
            "connection endpoints must differ"
        )));
    }
    if config.capacity_mbps == 0 {
        return Err(MediaLinkError::from_args(format_args!(
            "connection capacity must be non-zero"
        )));
    }
    if config.loss_every == Some(0) {
        return Err(MediaLinkError::from_args(format_args!(
            "connection loss interval must be non-zero"
        )));
    }
    medium
        .validate()
        .map_err(|error| MediaLinkError::from_args(format_args!("{error}")))?;
    for endpoint in [a, b] {
        if !endpoint.profile.hardware.supports(medium.kind()) {
            return Err(MediaLinkError::from_args(format_args!(
                "{}:{} hardware {} does not support {}",
                endpoint.component,
                endpoint.port,
                endpoint.profile.hardware,
                medium.kind()
            )));
        }
        endpoint
            .profile
            .settings
            .validate()
            .map_err(|error| MediaLinkError::from_args(format_args!("{error}")))?;
        if config.capacity_mbps > endpoint.profile.settings.speed_mbps {
            return Err(MediaLinkError::from_args(format_args!(
                "connection capacity exceeds {}:{} speed",
                endpoint.component, endpoint.port
            )));
        }
    }
    if medium
        .max_capacity_mbps()
        .is_some_and(|maximum| config.capacity_mbps > maximum)
    {
        return Err(MediaLinkError::from_args(format_args!(
            "connection capacity exceeds {} medium limit",
            medium.kind()
        )));
    }
    Ok(())
}
