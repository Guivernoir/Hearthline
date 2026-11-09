use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const SOLID_GEOMETRY_SCHEMA: &str = "0.1.0";

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GeometryReference {
    pub id: String,
    /// SHA-256 of the normalized, typed document, not its YAML formatting.
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SolidGeometryDocument {
    pub schema_version: String,
    pub id: String,
    pub solid: SolidGeometry,
}

impl SolidGeometryDocument {
    pub fn from_yaml(source: &str) -> Result<Self, serde_yaml_ng::Error> {
        serde_yaml_ng::from_str(source)
    }
}

/// Orthogonal, exactly mesh-aligned solids. Curved/cut-cell geometry requires a
/// separate discretization contract; it is not silently voxelized here.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SolidGeometry {
    OpenBasin {
        outer_extent_m: [String; 3],
        wall_thickness_m: [String; 2],
        floor_thickness_m: String,
    },
}
