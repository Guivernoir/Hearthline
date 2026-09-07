use core::fmt::{self, Display, Formatter};

use hearthline_model::{Position, Text};
use serde::Deserialize;

use super::{
    MediaError, MediaFacts, MediaText, SimulatedMedium, distance_text, error, facts, message,
    propagation_delay_us,
};

#[derive(Clone, Debug)]
pub struct FiberMedium {
    pub mode: FiberMode,
    pub connector: Text<32>,
    pub length: Position,
}

impl SimulatedMedium for FiberMedium {
    fn validate(&self) -> Result<(), MediaError> {
        if self.length <= Position::ZERO {
            return Err("fiber length must be greater than zero".into());
        }
        let limit = match self.mode {
            FiberMode::SingleMode => Position::from_raw(100_000_000_000),
            FiberMode::MultiMode => Position::from_raw(550_000_000),
        };
        if self.length > limit {
            return Err(error(format_args!(
                "{} fiber length {} m exceeds {} m",
                self.mode,
                distance_text(self.length),
                distance_text(limit)
            )));
        }
        if self.connector.trim().is_empty() {
            return Err("fiber connector cannot be empty".into());
        }
        Ok(())
    }

    fn detail(&self) -> MediaText {
        message(format_args!(
            "{} / {} / {} m",
            self.mode,
            self.connector,
            distance_text(self.length)
        ))
    }

    fn physical_facts(&self) -> MediaFacts {
        facts([
            message(format_args!("{} optical fiber", self.mode)),
            message(format_args!("{} connector", self.connector)),
            message(format_args!(
                "{} m physical segment",
                distance_text(self.length)
            )),
        ])
    }

    fn propagation_delay_us(&self) -> u64 {
        propagation_delay_us(self.length, 204_000_000)
    }

    fn max_capacity_mbps(&self) -> Option<u64> {
        Some(100_000)
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FiberMode {
    SingleMode,
    MultiMode,
}

impl Display for FiberMode {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::SingleMode => formatter.write_str("single-mode"),
            Self::MultiMode => formatter.write_str("multi-mode"),
        }
    }
}
