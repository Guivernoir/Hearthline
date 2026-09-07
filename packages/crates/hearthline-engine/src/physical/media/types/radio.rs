use super::{
    MediaError, MediaFacts, MediaText, SimulatedMedium, distance_text, error, facts, message,
    propagation_delay_us,
};
use hearthline_model::{Position, Text};

#[derive(Clone, Debug)]
pub struct RadioMedium {
    pub standard: Text<32>,
    pub ssid: Text<64>,
    pub security: Text<64>,
    pub distance: Position,
}

impl SimulatedMedium for RadioMedium {
    fn validate(&self) -> Result<(), MediaError> {
        for (field, value) in [
            ("radio standard", self.standard.as_str()),
            ("SSID", self.ssid.as_str()),
            ("wireless security", self.security.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(error(format_args!("{field} cannot be empty")));
            }
        }
        if self.distance <= Position::ZERO {
            return Err("radio distance must be greater than zero".into());
        }
        if self.distance > Position::from_raw(300_000_000) {
            return Err(error(format_args!(
                "radio path {:.1} m exceeds 300 m",
                distance_text(self.distance)
            )));
        }
        Ok(())
    }

    fn detail(&self) -> MediaText {
        message(format_args!(
            "{} / {} / {} / {} m",
            self.standard,
            self.ssid,
            self.security,
            distance_text(self.distance)
        ))
    }

    fn physical_facts(&self) -> MediaFacts {
        facts([
            Text::from(self.standard.as_str()),
            message(format_args!("{} security", self.security)),
            message(format_args!(
                "{} m modeled radio path",
                distance_text(self.distance)
            )),
            "Interference and stochastic fading are not yet modeled".into(),
        ])
    }

    fn propagation_delay_us(&self) -> u64 {
        propagation_delay_us(self.distance, 299_792_458)
    }

    fn max_capacity_mbps(&self) -> Option<u64> {
        None
    }
}
