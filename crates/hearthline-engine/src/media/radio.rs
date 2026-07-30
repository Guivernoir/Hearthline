use serde::Deserialize;

use super::{SimulatedMedium, propagation_delay_us};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadioMedium {
    pub standard: String,
    pub ssid: String,
    pub security: String,
    pub distance_m: f64,
}

impl SimulatedMedium for RadioMedium {
    fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("radio standard", &self.standard),
            ("SSID", &self.ssid),
            ("wireless security", &self.security),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{field} cannot be empty"));
            }
        }
        if self.distance_m <= 0.0 {
            return Err("radio distance must be greater than zero".into());
        }
        if self.distance_m > 300.0 {
            return Err(format!(
                "radio path is {:.1} m; modeled WLAN range is limited to 300 m",
                self.distance_m
            ));
        }
        Ok(())
    }

    fn detail(&self) -> String {
        format!(
            "{} / {} / {} / {:.1} m",
            self.standard, self.ssid, self.security, self.distance_m
        )
    }

    fn physical_facts(&self) -> Vec<String> {
        vec![
            self.standard.clone(),
            format!("{} security", self.security),
            format!("{:.1} m modeled radio path", self.distance_m),
            "Interference and stochastic fading are not yet modeled".into(),
        ]
    }

    fn propagation_delay_us(&self) -> u64 {
        propagation_delay_us(self.distance_m, 299_792_458.0)
    }

    fn max_capacity_mbps(&self) -> Option<u64> {
        None
    }
}
