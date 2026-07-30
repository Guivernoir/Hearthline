#[path = "types/carrier.rs"]
mod carrier;
#[path = "types/copper.rs"]
mod copper;
#[path = "types/fiber.rs"]
mod fiber;
#[path = "types/field_wiring.rs"]
mod field_wiring;
#[path = "types/radio.rs"]
mod radio;
#[path = "types/telephone.rs"]
mod telephone;
#[path = "types/virtual_link.rs"]
mod virtual_link;

use core::fmt::{self, Display, Formatter, Write as _};
use heapless::Vec as FixedList;
use hearthline_model::Text;

pub use carrier::CarrierMedium;
pub use copper::{CopperCategory, CopperMedium, CopperWiring};
pub use fiber::{FiberMedium, FiberMode};
pub use field_wiring::FieldWiringMedium;
pub use radio::RadioMedium;
pub use telephone::TelephoneMedium;
pub use virtual_link::VirtualMedium;

pub type MediaText = Text<192>;
pub type MediaError = Text<96>;
pub type MediaFacts = FixedList<MediaText, 6>;

pub trait SimulatedMedium {
    fn validate(&self) -> Result<(), MediaError>;
    fn detail(&self) -> MediaText;
    fn physical_facts(&self) -> MediaFacts;
    fn propagation_delay_us(&self) -> u64;
    fn max_capacity_mbps(&self) -> Option<u64>;
}

#[derive(Clone, Debug)]
pub enum ConnectionMedium {
    Copper { config: CopperMedium },
    Fiber { config: FiberMedium },
    Radio { config: RadioMedium },
    Carrier { config: CarrierMedium },
    Virtual { config: VirtualMedium },
    FieldWiring { config: FieldWiringMedium },
    Telephone { config: TelephoneMedium },
}

impl ConnectionMedium {
    pub const fn kind(&self) -> MediumKind {
        match self {
            Self::Copper { .. } => MediumKind::Copper,
            Self::Fiber { .. } => MediumKind::Fiber,
            Self::Radio { .. } => MediumKind::Radio,
            Self::Carrier { .. } => MediumKind::Carrier,
            Self::Virtual { .. } => MediumKind::Virtual,
            Self::FieldWiring { .. } => MediumKind::FieldWiring,
            Self::Telephone { .. } => MediumKind::Telephone,
        }
    }

    pub const fn requires_exclusive_endpoints(&self) -> bool {
        !matches!(self, Self::Radio { .. } | Self::Virtual { .. })
    }

    pub fn validate(&self) -> Result<(), MediaError> {
        self.behavior().validate()
    }

    pub fn detail(&self) -> MediaText {
        self.behavior().detail()
    }

    pub fn physical_facts(&self) -> MediaFacts {
        self.behavior().physical_facts()
    }

    pub fn propagation_delay_us(&self) -> u64 {
        self.behavior().propagation_delay_us()
    }

    pub fn max_capacity_mbps(&self) -> Option<u64> {
        self.behavior().max_capacity_mbps()
    }

    fn behavior(&self) -> &dyn SimulatedMedium {
        match self {
            Self::Copper { config } => config,
            Self::Fiber { config } => config,
            Self::Radio { config } => config,
            Self::Carrier { config } => config,
            Self::Virtual { config } => config,
            Self::FieldWiring { config } => config,
            Self::Telephone { config } => config,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MediumKind {
    Copper,
    Fiber,
    Radio,
    Carrier,
    Virtual,
    FieldWiring,
    Telephone,
}

impl Display for MediumKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Copper => "copper",
            Self::Fiber => "fiber",
            Self::Radio => "radio",
            Self::Carrier => "carrier",
            Self::Virtual => "virtual",
            Self::FieldWiring => "field-wiring",
            Self::Telephone => "telephone",
        };
        formatter.write_str(value)
    }
}

pub(crate) fn propagation_delay_us(distance_m: f64, velocity_mps: f64) -> u64 {
    let micros = (distance_m / velocity_mps) * 1_000_000.0;
    let whole = micros as u64;
    if micros > whole as f64 {
        whole.saturating_add(1)
    } else {
        whole
    }
}

pub(crate) fn message(arguments: fmt::Arguments<'_>) -> MediaText {
    let mut output = MediaText::default();
    output
        .write_fmt(arguments)
        .expect("media description exceeds fixed capacity");
    output
}

pub(crate) fn error(arguments: fmt::Arguments<'_>) -> MediaError {
    let mut output = MediaError::default();
    output
        .write_fmt(arguments)
        .expect("media validation error exceeds fixed capacity");
    output
}

pub(crate) fn facts<const N: usize>(values: [MediaText; N]) -> MediaFacts {
    let mut output = MediaFacts::new();
    for value in values {
        output
            .push(value)
            .expect("media facts exceed fixed capacity");
    }
    output
}
