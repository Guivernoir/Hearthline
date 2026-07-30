use serde::Deserialize;

use super::{SimulatedMedium, propagation_delay_us};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldWiringMedium {
    pub signal: String,
    pub length_m: f64,
}

impl SimulatedMedium for FieldWiringMedium {
    fn validate(&self) -> Result<(), String> {
        if self.signal.trim().is_empty() {
            return Err("field signal cannot be empty".into());
        }
        if self.length_m <= 0.0 {
            return Err("field-wiring length must be greater than zero".into());
        }
        if self.length_m > 500.0 {
            return Err(format!(
                "field-wiring segment is {:.1} m; modeled generic limit is 500 m",
                self.length_m
            ));
        }
        Ok(())
    }

    fn detail(&self) -> String {
        format!("{} / {:.1} m", self.signal, self.length_m)
    }

    fn physical_facts(&self) -> Vec<String> {
        vec![
            self.signal.clone(),
            format!("{:.1} m field segment", self.length_m),
            "Protocol-specific electrical limits require a later typed profile".into(),
        ]
    }

    fn propagation_delay_us(&self) -> u64 {
        propagation_delay_us(self.length_m, 200_000_000.0)
    }

    fn max_capacity_mbps(&self) -> Option<u64> {
        Some(100)
    }
}
