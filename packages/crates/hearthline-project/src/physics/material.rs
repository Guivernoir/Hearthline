use hearthline_config::physics::material::{
    DRYING_MATERIAL_SCHEMA, DryingMaterialConfig, DryingMaterialDocument, MaterialProvenance,
};
use hearthline_model::ComponentId;
use hearthline_physics::material::PropertyTable;
use hearthline_physics::transport::SolverLimits;
use hearthline_physics::transport::nonlinear::NonlinearLimits;
use hearthline_physics::{PhysicsError, Scalar};
use sha2::{Digest, Sha256};

use super::{CompiledProofCase, error, nonnegative, positive};

mod caloric;
pub use caloric::CompiledCaloricMaterial;

#[derive(Clone, Debug)]
pub struct CompiledDryingMaterial {
    document: DryingMaterialDocument,
    knots: Vec<[Scalar; 2]>,
    temperature_range: [Scalar; 2],
    limits: NonlinearLimits,
    caloric: Option<CompiledCaloricMaterial>,
}

impl CompiledDryingMaterial {
    pub(super) fn compile(
        config: &DryingMaterialConfig,
        document: DryingMaterialDocument,
        case: CompiledProofCase,
        linear: SolverLimits,
    ) -> Result<Self, String> {
        if document.schema_version != DRYING_MATERIAL_SCHEMA {
            return Err("unsupported drying material schema".into());
        }
        ComponentId::new(&document.id).map_err(|e| e.to_string())?;
        if config.reference.id != document.id
            || config.reference.sha256 != material_digest(&document)?
        {
            return Err("drying material reference or digest mismatch".into());
        }
        match &document.provenance {
            MaterialProvenance::SyntheticVerification { description }
                if !description.trim().is_empty() => {}
            MaterialProvenance::PublicMeasurement { source, locator }
                if public_reference(source) && !locator.trim().is_empty() => {}
            _ => return Err(
                "material provenance needs a description or a public HTTPS source and data locator"
                    .into(),
            ),
        }
        let mut knots = Vec::new();
        knots
            .try_reserve_exact(document.diffusivity_multiplier_by_moisture.len())
            .map_err(|e| format!("material table allocation: {e}"))?;
        for [moisture, multiplier] in &document.diffusivity_multiplier_by_moisture {
            knots.push([nonnegative(moisture)?, nonnegative(multiplier)?]);
        }
        PropertyTable::new(&knots).map_err(error)?;
        let temperature_range = [
            positive(&document.temperature_range_k[0])?,
            positive(&document.temperature_range_k[1])?,
        ];
        if temperature_range[0] >= temperature_range[1] {
            return Err("material temperature validity interval must increase".into());
        }
        let limits = NonlinearLimits {
            linear,
            iterations: config.maximum_iterations,
            residual: positive(&config.residual_tolerance)?,
            relaxation: positive(&config.relaxation)?,
        };
        if limits.iterations == 0
            || limits.residual < linear.residual
            || limits.relaxation > Scalar::ONE
        {
            return Err("invalid material nonlinear iteration limits".into());
        }
        let CompiledProofCase::Drying {
            moisture,
            equilibrium,
            temperature,
            ambient,
            diffusion,
            heat_capacity,
            ..
        } = case
        else {
            return Err("drying material is only supported by a drying proof".into());
        };
        if diffusion != positive(&document.reference_diffusivity_m2_s)? {
            return Err("material reference diffusivity disagrees with the proof profile".into());
        }
        if document
            .liquid_heat_capacity_j_kg_k_by_temperature
            .is_some()
            && document.dry_heat_capacity_j_kg_k_by_temperature.is_none()
        {
            return Err("liquid caloric law requires the dry-solid heat-capacity table".into());
        }
        let caloric = match (
            &config.caloric_solver,
            &document.dry_heat_capacity_j_kg_k_by_temperature,
        ) {
            (None, None) => None,
            (Some(solver), Some(capacities)) => Some(CompiledCaloricMaterial::compile(
                solver,
                capacities,
                document
                    .liquid_heat_capacity_j_kg_k_by_temperature
                    .as_deref(),
                temperature_range,
                temperature,
                heat_capacity,
            )?),
            _ => {
                return Err(
                    "caloric material requires both a heat-capacity table and solver settings"
                        .into(),
                );
            }
        };
        let compiled = Self {
            document,
            knots,
            temperature_range,
            limits,
            caloric,
        };
        compiled
            .validate_state(moisture, temperature)
            .map_err(error)?;
        compiled
            .validate_state(equilibrium, ambient)
            .map_err(error)?;
        Ok(compiled)
    }

    pub fn document(&self) -> &DryingMaterialDocument {
        &self.document
    }

    pub fn multiplier(&self) -> Result<PropertyTable<'_>, PhysicsError> {
        PropertyTable::new(&self.knots)
    }

    pub const fn limits(&self) -> NonlinearLimits {
        self.limits
    }

    pub fn caloric(&self) -> Option<&CompiledCaloricMaterial> {
        self.caloric.as_ref()
    }

    pub fn validate_state(
        &self,
        moisture: Scalar,
        temperature: Scalar,
    ) -> Result<(), PhysicsError> {
        if temperature < self.temperature_range[0] || temperature > self.temperature_range[1] {
            return Err(PhysicsError::InvalidState);
        }
        if moisture < self.knots[0][0] || moisture > self.knots[self.knots.len() - 1][0] {
            return Err(PhysicsError::InvalidState);
        }
        Ok(())
    }
}

fn public_reference(source: &str) -> bool {
    url::Url::parse(source).is_ok_and(|url| {
        url.scheme() == "https"
            && url.has_host()
            && url.username().is_empty()
            && url.password().is_none()
    })
}

pub fn material_digest(document: &DryingMaterialDocument) -> Result<String, String> {
    Ok(crate::source::sha256_hex(&Sha256::digest(
        serde_json::to_vec(document).map_err(|e| e.to_string())?,
    )))
}
