use super::{
    MediaError, MediaFacts, MediaText, SimulatedMedium, distance_text, error, facts, message,
    propagation_delay_us,
};
use hearthline_model::{Position, Text};

#[derive(Clone, Debug)]
pub struct FieldWiringMedium {
    pub signal: Text<64>,
    pub length: Position,
}

impl SimulatedMedium for FieldWiringMedium {
    fn validate(&self) -> Result<(), MediaError> {
        if self.signal.trim().is_empty() {
            return Err("field signal cannot be empty".into());
        }
        if self.length <= Position::ZERO {
            return Err("field-wiring length must be greater than zero".into());
        }
        if self.length > Position::from_raw(500_000_000) {
            return Err(error(format_args!(
                "field-wiring length {:.1} m exceeds 500 m",
                distance_text(self.length)
            )));
        }
        Ok(())
    }

    fn detail(&self) -> MediaText {
        message(format_args!(
            "{} / {} m",
            self.signal,
            distance_text(self.length)
        ))
    }

    fn physical_facts(&self) -> MediaFacts {
        facts([
            Text::from(self.signal.as_str()),
            message(format_args!(
                "{} m field segment",
                distance_text(self.length)
            )),
            "Protocol-specific electrical limits require a later typed profile".into(),
        ])
    }

    fn propagation_delay_us(&self) -> u64 {
        propagation_delay_us(self.length, 200_000_000)
    }

    fn max_capacity_mbps(&self) -> Option<u64> {
        Some(100)
    }
}
