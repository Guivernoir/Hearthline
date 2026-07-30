mod carrier;
mod copper;
mod fiber;
mod field_wiring;
mod radio;
mod telephone;
mod virtual_link;

use std::fmt::{self, Display, Formatter};

use serde::Deserialize;

pub use carrier::CarrierMedium;
pub use copper::{CopperCategory, CopperMedium, CopperWiring};
pub use fiber::{FiberMedium, FiberMode};
pub use field_wiring::FieldWiringMedium;
pub use radio::RadioMedium;
pub use telephone::TelephoneMedium;
pub use virtual_link::VirtualMedium;

pub trait SimulatedMedium {
    fn validate(&self) -> Result<(), String>;
    fn detail(&self) -> String;
    fn physical_facts(&self) -> Vec<String>;
    fn propagation_delay_us(&self) -> u64;
    fn max_capacity_mbps(&self) -> Option<u64>;
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ConnectionMedium {
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

    pub(crate) const fn requires_exclusive_endpoints(&self) -> bool {
        !matches!(self, Self::Radio { .. } | Self::Virtual { .. })
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        self.behavior().validate()
    }

    pub(crate) fn detail(&self) -> String {
        self.behavior().detail()
    }

    pub(crate) fn physical_facts(&self) -> Vec<String> {
        self.behavior().physical_facts()
    }

    pub(crate) fn propagation_delay_us(&self) -> u64 {
        self.behavior().propagation_delay_us()
    }

    pub(crate) fn max_capacity_mbps(&self) -> Option<u64> {
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
    ((distance_m / velocity_mps) * 1_000_000.0).ceil() as u64
}
