use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod capacity;
pub mod geometry;
pub mod material;

pub const PHYSICS_PROOF_SCHEMA_VERSION: &str = "0.1.0";

/// Standalone numerical verification inputs. These are not factory recipes.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PhysicsProofConfig {
    pub schema_version: String,
    pub id: String,
    pub shape: [u32; 3],
    pub spacing_m: [String; 3],
    pub timestep_s: String,
    pub steps: u32,
    pub maximum_iterations: u32,
    pub residual_tolerance: String,
    pub allocation_budget_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<geometry::GeometryReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<material::DryingMaterialConfig>,
    pub case: PhysicsProofCase,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum PhysicsProofCase {
    CompressibleReceiver {
        flow: Box<CompressibleReceiverConfig>,
    },
    IsothermalReceiver {
        temperature_k: String,
        gas_constant_j_kg_k: String,
        initial_pressure_pa: String,
        receiver_pressure_pa: String,
        conductance_kg_pa_s: String,
    },
    DryingSpecimen {
        initial_moisture_kg_kg: String,
        equilibrium_moisture_kg_kg: String,
        initial_temperature_k: String,
        ambient_temperature_k: String,
        dry_density_kg_m3: String,
        moisture_diffusivity_m2_s: String,
        surface_transfer_m_s: String,
        heat_capacity_j_kg_k: String,
        thermal_conductivity_w_m_k: String,
        surface_heat_transfer_w_m2_k: String,
        latent_heat_j_kg: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mechanics: Option<Box<SpecimenMechanicsConfig>>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompressibleReceiverConfig {
    pub length_scale_m: String,
    pub velocity_scale_m_s: String,
    pub density_scale_kg_m3: String,
    pub gas_constant_j_kg_k: String,
    pub heat_capacity_ratio: String,
    pub initial_temperature_k: String,
    pub upstream_pressure_pa: String,
    pub receiver_pressure_pa: String,
    pub receiver_start_x: u32,
    pub dynamic_viscosity_pa_s: String,
    pub thermal_conductivity_w_m_k: String,
    pub wall_temperature_k: Option<String>,
    pub maximum_substeps: u32,
    /// Minimum retry step in nondimensional solver microseconds.
    pub minimum_solver_step_us: u64,
}

/// Linear isotropic verification law, not a calibrated ceramic constitutive law.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpecimenMechanicsConfig {
    pub length_scale_m: String,
    pub stress_scale_pa: String,
    pub young_modulus_pa: String,
    pub poisson_ratio: String,
    pub moisture_expansion_per_kg_kg: String,
    pub thermal_expansion_per_k: String,
    pub support: SpecimenSupport,
    pub maximum_iterations: u32,
    pub residual_tolerance: String,
    /// Previous-step guesses retain displacement history. Zero-start solves
    /// make this history-free elastic law exactly reconstructible.
    #[serde(
        default,
        skip_serializing_if = "MechanicsInitialGuess::is_previous_step"
    )]
    pub initial_guess: MechanicsInitialGuess,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MechanicsInitialGuess {
    #[default]
    PreviousStep,
    Zero,
}

impl MechanicsInitialGuess {
    pub const fn is_previous_step(&self) -> bool {
        matches!(self, Self::PreviousStep)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SpecimenSupport {
    /// Remove rigid-body modes without restricting uniform free expansion.
    Free,
    FixedXFace,
}
