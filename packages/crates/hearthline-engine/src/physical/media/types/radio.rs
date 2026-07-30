use hearthline_model::Text;
use serde::Deserialize;

use super::{
    MediaError, MediaFacts, MediaText, SimulatedMedium, error, facts, message, propagation_delay_us,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadioMedium {
    pub standard: Text<32>,
    pub ssid: Text<64>,
    pub security: Text<64>,
    pub distance_m: f64,
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
        if self.distance_m <= 0.0 {
            return Err("radio distance must be greater than zero".into());
        }
        if self.distance_m > 300.0 {
            return Err(error(format_args!(
                "radio path {:.1} m exceeds 300 m",
                self.distance_m
            )));
        }
        Ok(())
    }

    fn detail(&self) -> MediaText {
        message(format_args!(
            "{} / {} / {} / {:.1} m",
            self.standard, self.ssid, self.security, self.distance_m
        ))
    }

    fn physical_facts(&self) -> MediaFacts {
        facts([
            Text::from(self.standard.as_str()),
            message(format_args!("{} security", self.security)),
            message(format_args!("{:.1} m modeled radio path", self.distance_m)),
            "Interference and stochastic fading are not yet modeled".into(),
        ])
    }

    fn propagation_delay_us(&self) -> u64 {
        propagation_delay_us(self.distance_m, 299_792_458.0)
    }

    fn max_capacity_mbps(&self) -> Option<u64> {
        None
    }
}
