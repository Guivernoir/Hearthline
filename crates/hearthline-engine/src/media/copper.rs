use std::fmt::{self, Display, Formatter};

use serde::Deserialize;

use super::{SimulatedMedium, propagation_delay_us};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopperMedium {
    pub wiring: CopperWiring,
    pub category: CopperCategory,
    pub length_m: f64,
}

impl SimulatedMedium for CopperMedium {
    fn validate(&self) -> Result<(), String> {
        if self.length_m <= 0.0 {
            return Err("copper length must be greater than zero".into());
        }
        if self.length_m > 100.0 {
            return Err(format!(
                "{} copper segment is {:.1} m; modeled balanced Ethernet copper is limited to 100 m",
                self.category, self.length_m
            ));
        }
        Ok(())
    }

    fn detail(&self) -> String {
        format!("{} {} / {:.1} m", self.category, self.wiring, self.length_m)
    }

    fn physical_facts(&self) -> Vec<String> {
        vec![
            format!("Balanced copper {}", self.category),
            format!("{} pinout", self.wiring),
            format!("{:.1} m physical segment", self.length_m),
            "100 m modeled segment limit".into(),
        ]
    }

    fn propagation_delay_us(&self) -> u64 {
        propagation_delay_us(self.length_m, 200_000_000.0)
    }

    fn max_capacity_mbps(&self) -> Option<u64> {
        Some(match self.category {
            CopperCategory::Cat5e | CopperCategory::Cat6 => 1_000,
            CopperCategory::Cat6a => 10_000,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CopperWiring {
    StraightThrough,
    Crossover,
}

impl Display for CopperWiring {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::StraightThrough => formatter.write_str("straight-through"),
            Self::Crossover => formatter.write_str("crossover"),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CopperCategory {
    Cat5e,
    Cat6,
    Cat6a,
}

impl Display for CopperCategory {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Cat5e => "Cat5e",
            Self::Cat6 => "Cat6",
            Self::Cat6a => "Cat6A",
        };
        formatter.write_str(value)
    }
}
