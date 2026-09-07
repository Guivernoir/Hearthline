use super::{
    MediaError, MediaFacts, MediaText, SimulatedMedium, distance_text, facts, message,
    propagation_delay_us,
};
use hearthline_model::{Position, Text};

#[derive(Clone, Debug)]
pub struct TelephoneMedium {
    pub connector: Text<32>,
    pub pairs: u8,
    pub length: Position,
}

impl SimulatedMedium for TelephoneMedium {
    fn validate(&self) -> Result<(), MediaError> {
        if self.connector.trim().is_empty() {
            return Err("telephone connector cannot be empty".into());
        }
        if self.pairs == 0 || self.pairs > 4 {
            return Err("telephone cabling must declare between one and four pairs".into());
        }
        if self.length <= Position::ZERO || self.length > Position::from_raw(5_000_000_000) {
            return Err("telephone segment length must be within 0 and 5000 m".into());
        }
        Ok(())
    }

    fn detail(&self) -> MediaText {
        message(format_args!(
            "{} / {} pair(s) / {} m",
            self.connector,
            self.pairs,
            distance_text(self.length)
        ))
    }

    fn physical_facts(&self) -> MediaFacts {
        facts([
            message(format_args!(
                "{} analog telephone connector",
                self.connector
            )),
            message(format_args!("{} copper pair(s)", self.pairs)),
            message(format_args!(
                "{} m physical segment",
                distance_text(self.length)
            )),
        ])
    }

    fn propagation_delay_us(&self) -> u64 {
        propagation_delay_us(self.length, 200_000_000)
    }

    fn max_capacity_mbps(&self) -> Option<u64> {
        Some(1)
    }
}
