//! Standalone connected-flow verification sources, not factory appliance files.
//! Decimal quantities are strings so YAML never rounds through binary floating point.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

mod bounded;

pub const CONNECTED_GAS_SOURCE_SCHEMA: &str = "0.1.0";
pub const CONNECTED_GAS_SOURCE_LIMIT: usize = 1_048_576;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConnectedGasDocument {
    #[serde(deserialize_with = "bounded::text")]
    pub schema_version: String,
    #[serde(deserialize_with = "bounded::text")]
    pub id: String,
    pub allocation_budget_bytes: u64,
    pub reference: GasReferenceConfig,
    pub gas: IdealGasConfig,
    pub integration: GasIntegrationConfig,
    pub regions: Vec<GasRegionConfig>,
    pub connections: Vec<GasConnectionConfig>,
}

impl ConnectedGasDocument {
    pub fn from_yaml(source: &str) -> Result<Self, String> {
        if source.len() > CONNECTED_GAS_SOURCE_LIMIT {
            return Err("connected gas source exceeds 1 MiB".into());
        }
        bounded::validate(source, CONNECTED_GAS_SOURCE_LIMIT)?;
        serde_yaml_ng::from_str(source).map_err(|error| error.to_string())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GasReferenceConfig {
    #[serde(deserialize_with = "bounded::text")]
    pub length_m: String,
    #[serde(deserialize_with = "bounded::text")]
    pub velocity_m_s: String,
    #[serde(deserialize_with = "bounded::text")]
    pub density_kg_m3: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IdealGasConfig {
    #[serde(deserialize_with = "bounded::text")]
    pub gas_constant_j_kg_k: String,
    #[serde(deserialize_with = "bounded::text")]
    pub heat_capacity_ratio: String,
    #[serde(deserialize_with = "bounded::text")]
    pub dynamic_viscosity_pa_s: String,
    #[serde(deserialize_with = "bounded::text")]
    pub thermal_conductivity_w_m_k: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GasIntegrationConfig {
    pub timestep_us: u64,
    pub steps: u32,
    pub maximum_iterations: u32,
    /// Dimensionless, normalized solver residual. It is not a pressure error in Pa.
    #[serde(deserialize_with = "bounded::text")]
    pub residual_tolerance: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retries: Option<GasRetryConfig>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GasRetryConfig {
    pub maximum_substeps: u32,
    /// Physical microseconds, unlike the internal nondimensional retry duration.
    pub minimum_timestep_us: u64,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GasRegionIdentity {
    #[serde(deserialize_with = "bounded::text")]
    pub site: String,
    #[serde(deserialize_with = "bounded::text")]
    pub cell: String,
    #[serde(deserialize_with = "bounded::text")]
    pub instance: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GasRegionConfig {
    pub identity: GasRegionIdentity,
    pub shape: [u32; 3],
    #[serde(deserialize_with = "bounded::texts")]
    pub spacing_m: [String; 3],
    pub boundaries: GasBoundarySet,
    pub initial: GasPrimitiveConfig,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GasPrimitiveConfig {
    #[serde(deserialize_with = "bounded::text")]
    pub density_kg_m3: String,
    #[serde(deserialize_with = "bounded::text")]
    pub pressure_absolute_pa: String,
    #[serde(deserialize_with = "bounded::texts")]
    pub velocity_m_s: [String; 3],
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GasBoundarySet {
    pub x_minus: GasBoundaryConfig,
    pub x_plus: GasBoundaryConfig,
    pub y_minus: GasBoundaryConfig,
    pub y_plus: GasBoundaryConfig,
    pub z_minus: GasBoundaryConfig,
    pub z_plus: GasBoundaryConfig,
}

impl GasBoundarySet {
    pub fn faces(&self) -> [&GasBoundaryConfig; 6] {
        [
            &self.x_minus,
            &self.x_plus,
            &self.y_minus,
            &self.y_plus,
            &self.z_minus,
            &self.z_plus,
        ]
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum GasBoundaryConfig {
    Coupled,
    AdiabaticSlipWall,
    NoSlipWall {
        #[serde(deserialize_with = "bounded::texts")]
        velocity_m_s: [String; 3],
        /// Omit for an adiabatic wall; otherwise this is a maintained temperature.
        #[serde(default, deserialize_with = "bounded::optional_text")]
        temperature_k: Option<String>,
    },
    /// An explicitly infinite external boundary, never a finite receiver/compressor.
    Reservoir {
        state: GasPrimitiveConfig,
    },
}

#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum GasFace {
    XMinus,
    XPlus,
    YMinus,
    YPlus,
    ZMinus,
    ZPlus,
}

impl GasFace {
    pub const fn index(self) -> u8 {
        match self {
            Self::XMinus => 0,
            Self::XPlus => 1,
            Self::YMinus => 2,
            Self::YPlus => 3,
            Self::ZMinus => 4,
            Self::ZPlus => 5,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GasEndpointConfig {
    pub region: GasRegionIdentity,
    pub face: GasFace,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GasConnectionConfig {
    #[serde(deserialize_with = "bounded::text")]
    pub id: String,
    pub first: GasEndpointConfig,
    pub second: GasEndpointConfig,
}
