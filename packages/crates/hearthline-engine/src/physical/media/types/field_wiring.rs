use hearthline_model::Text;
use serde::Deserialize;

use super::{
    MediaError, MediaFacts, MediaText, SimulatedMedium, error, facts, message, propagation_delay_us,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldWiringMedium {
    pub signal: Text<64>,
    pub length_m: f64,
}

impl SimulatedMedium for FieldWiringMedium {
    fn validate(&self) -> Result<(), MediaError> {
        if self.signal.trim().is_empty() {
            return Err("field signal cannot be empty".into());
        }
        if self.length_m <= 0.0 {
            return Err("field-wiring length must be greater than zero".into());
        }
        if self.length_m > 500.0 {
            return Err(error(format_args!(
                "field-wiring length {:.1} m exceeds 500 m",
                self.length_m
            )));
        }
        Ok(())
    }

    fn detail(&self) -> MediaText {
        message(format_args!("{} / {:.1} m", self.signal, self.length_m))
    }

    fn physical_facts(&self) -> MediaFacts {
        facts([
            Text::from(self.signal.as_str()),
            message(format_args!("{:.1} m field segment", self.length_m)),
            "Protocol-specific electrical limits require a later typed profile".into(),
        ])
    }

    fn propagation_delay_us(&self) -> u64 {
        propagation_delay_us(self.length_m, 200_000_000.0)
    }

    fn max_capacity_mbps(&self) -> Option<u64> {
        Some(100)
    }
}
