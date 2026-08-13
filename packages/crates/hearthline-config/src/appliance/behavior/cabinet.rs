use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UtilityMediumConfig {
    CeramicSlip,
    CompressedAir,
    Water,
    Vacuum,
    Drain,
    HydraulicOil,
}

impl std::fmt::Display for UtilityMediumConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::CeramicSlip => "ceramic-slip",
            Self::CompressedAir => "compressed-air",
            Self::Water => "water",
            Self::Vacuum => "vacuum",
            Self::Drain => "drain",
            Self::HydraulicOil => "hydraulic-oil",
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MouldControlCabinetConfig {
    pub target: String,
    pub enclosure_rating: String,
    pub control_voltage_vdc: u16,
    pub safety_relay: String,
    pub utility_cabinet: String,
    pub modules: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MouldUtilityCircuitConfig {
    pub id: String,
    pub label: String,
    pub medium: UtilityMediumConfig,
    pub source: String,
    #[serde(default)]
    pub nominal_pressure: Option<f64>,
    pub command_states: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MouldUtilityCabinetConfig {
    pub target: String,
    pub enclosure_rating: String,
    pub control_voltage_vdc: u16,
    pub remote_io: String,
    pub isolation_state: String,
    pub circuits: Vec<MouldUtilityCircuitConfig>,
}
