use std::fmt::{self, Display, Formatter};

use serde::Deserialize;

use super::{SimulatedMedium, propagation_delay_us};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FiberMedium {
    pub mode: FiberMode,
    pub connector: String,
    pub length_m: f64,
}

impl SimulatedMedium for FiberMedium {
    fn validate(&self) -> Result<(), String> {
        if self.length_m <= 0.0 {
            return Err("fiber length must be greater than zero".into());
        }
        let limit = match self.mode {
            FiberMode::SingleMode => 100_000.0,
            FiberMode::MultiMode => 550.0,
        };
        if self.length_m > limit {
            return Err(format!(
                "{} fiber segment is {:.1} m; modeled limit is {:.1} m",
                self.mode, self.length_m, limit
            ));
        }
        if self.connector.trim().is_empty() {
            return Err("fiber connector cannot be empty".into());
        }
        Ok(())
    }

    fn detail(&self) -> String {
        format!(
            "{} / {} / {:.1} m",
            self.mode, self.connector, self.length_m
        )
    }

    fn physical_facts(&self) -> Vec<String> {
        vec![
            format!("{} optical fiber", self.mode),
            format!("{} connector", self.connector),
            format!("{:.1} m physical segment", self.length_m),
        ]
    }

    fn propagation_delay_us(&self) -> u64 {
        propagation_delay_us(self.length_m, 204_000_000.0)
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
