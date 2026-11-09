use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const DRYING_MATERIAL_SCHEMA: &str = "0.1.0";

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialReference {
    pub id: String,
    /// SHA-256 of the typed document, independent of YAML key order and comments.
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DryingMaterialConfig {
    pub reference: MaterialReference,
    pub maximum_iterations: u32,
    pub residual_tolerance: String,
    pub relaxation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caloric_solver: Option<CaloricSolverConfig>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaloricSolverConfig {
    pub maximum_sweeps: u32,
    pub maximum_local_iterations: u32,
    pub residual_tolerance_j: String,
}

/// Partial constitutive contract. Optional caloric laws store dry-solid and
/// liquid sensible energy; vapor storage, sorption and phase equilibria are absent.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DryingMaterialDocument {
    pub schema_version: String,
    pub id: String,
    pub provenance: MaterialProvenance,
    pub reference_diffusivity_m2_s: String,
    /// Closed interval; the law must not be extrapolated beyond it.
    pub temperature_range_k: [String; 2],
    /// [water mass / dry solid mass, dimensionless diffusivity multiplier].
    pub diffusivity_multiplier_by_moisture: Vec<[String; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dry_heat_capacity_j_kg_k_by_temperature: Option<Vec<[String; 2]>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liquid_heat_capacity_j_kg_k_by_temperature: Option<Vec<[String; 2]>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum MaterialProvenance {
    SyntheticVerification {
        description: String,
    },
    /// Traceability, not a claim that the application has validated this law.
    PublicMeasurement {
        source: String,
        locator: String,
    },
}

impl DryingMaterialDocument {
    pub fn from_yaml(source: &str) -> Result<Self, serde_yaml_ng::Error> {
        serde_yaml_ng::from_str(source)
    }
}
