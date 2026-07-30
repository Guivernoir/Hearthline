use hearthline_model::Text;
use serde::Deserialize;

use super::{
    MediaError, MediaFacts, MediaText, SimulatedMedium, facts, message, propagation_delay_us,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelephoneMedium {
    pub connector: Text<32>,
    pub pairs: u8,
    pub length_m: f64,
}

impl SimulatedMedium for TelephoneMedium {
    fn validate(&self) -> Result<(), MediaError> {
        if self.connector.trim().is_empty() {
            return Err("telephone connector cannot be empty".into());
        }
        if self.pairs == 0 || self.pairs > 4 {
            return Err("telephone cabling must declare between one and four pairs".into());
        }
        if self.length_m <= 0.0 || self.length_m > 5_000.0 {
            return Err("telephone segment length must be within 0 and 5000 m".into());
        }
        Ok(())
    }

    fn detail(&self) -> MediaText {
        message(format_args!(
            "{} / {} pair(s) / {:.1} m",
            self.connector, self.pairs, self.length_m
        ))
    }

    fn physical_facts(&self) -> MediaFacts {
        facts([
            message(format_args!(
                "{} analog telephone connector",
                self.connector
            )),
            message(format_args!("{} copper pair(s)", self.pairs)),
            message(format_args!("{:.1} m physical segment", self.length_m)),
        ])
    }

    fn propagation_delay_us(&self) -> u64 {
        propagation_delay_us(self.length_m, 200_000_000.0)
    }

    fn max_capacity_mbps(&self) -> Option<u64> {
        Some(1)
    }
}
