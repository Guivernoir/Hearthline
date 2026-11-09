use std::collections::{BTreeMap, BTreeSet};

use hearthline_config::physics::capacity::{CapacityKernel, CapacityProfile};
use hearthline_config::physics::geometry::SolidGeometryDocument;
use hearthline_config::physics::material::DryingMaterialDocument;
use hearthline_config::physics::{
    PHYSICS_PROOF_SCHEMA_VERSION, PhysicsProofCase, PhysicsProofConfig,
};
use sha2::{Digest, Sha256};

use super::{ProfileStorage, valid_id};
use crate::physics::CompiledPhysicsProof;

/// Explicit documents, independent of traversal order or filesystem paths.
#[derive(Clone, Debug, Default)]
pub struct CapacitySources {
    pub proofs: Vec<PhysicsProofConfig>,
    pub geometries: Vec<SolidGeometryDocument>,
    pub materials: Vec<DryingMaterialDocument>,
}

/// Typed source identity; the report separately records the compiled model
/// digest, including the numerical contracts used by the runtime.
pub fn proof_source_digest(config: &PhysicsProofConfig) -> Result<String, String> {
    if config.schema_version != PHYSICS_PROOF_SCHEMA_VERSION {
        return Err("unsupported physics proof schema".into());
    }
    valid_id(&config.id)?;
    Ok(crate::source::sha256_hex(&Sha256::digest(
        serde_json::to_vec(config).map_err(|e| e.to_string())?,
    )))
}

pub(super) struct Resolver {
    proofs: BTreeMap<String, PhysicsProofConfig>,
    geometries: BTreeMap<String, SolidGeometryDocument>,
    materials: BTreeMap<String, DryingMaterialDocument>,
    used_proofs: BTreeSet<String>,
    used_geometries: BTreeSet<String>,
    used_materials: BTreeSet<String>,
}

impl Resolver {
    pub fn new(sources: CapacitySources) -> Result<Self, String> {
        Ok(Self {
            proofs: indexed(sources.proofs, |p| &p.id)?,
            geometries: indexed(sources.geometries, |p| &p.id)?,
            materials: indexed(sources.materials, |p| &p.id)?,
            used_proofs: BTreeSet::new(),
            used_geometries: BTreeSet::new(),
            used_materials: BTreeSet::new(),
        })
    }

    pub fn resolve(
        &mut self,
        profile: CapacityProfile,
        host_budget: u64,
    ) -> Result<ProfileStorage, String> {
        let Some(reference) = &profile.model else {
            return Ok(ProfileStorage {
                layout: crate::physics::PhysicsStorageLayout::compile(
                    profile.shape,
                    profile.kernel,
                )?,
                profile,
                compiled_model_digest: None,
                compiler_scratch_bytes: 0,
            });
        };
        let proof = self
            .proofs
            .get(&reference.id)
            .ok_or_else(|| format!("missing physics capacity proof {}", reference.id))?;
        if proof_source_digest(proof)? != reference.sha256 {
            return Err(format!("capacity proof {} digest mismatch", reference.id));
        }
        let kernel = match proof.case {
            PhysicsProofCase::DryingSpecimen {
                mechanics: Some(_), ..
            } => CapacityKernel::DryingElasticity,
            PhysicsProofCase::CompressibleReceiver { .. } => CapacityKernel::CompressibleFlow,
            _ => CapacityKernel::ScalarTransport,
        };
        if profile.shape != proof.shape || profile.kernel != kernel {
            return Err(format!(
                "capacity profile {} shape or kernel differs from its proof",
                profile.id
            ));
        }
        let geometry = proof
            .geometry
            .as_ref()
            .map(|reference| {
                self.used_geometries.insert(reference.id.clone());
                self.geometries
                    .get(&reference.id)
                    .cloned()
                    .ok_or_else(|| format!("missing capacity geometry {}", reference.id))
            })
            .transpose()?;
        let material = proof
            .material
            .as_ref()
            .map(|settings| {
                self.used_materials.insert(settings.reference.id.clone());
                self.materials
                    .get(&settings.reference.id)
                    .cloned()
                    .ok_or_else(|| format!("missing capacity material {}", settings.reference.id))
            })
            .transpose()?;
        // Compilation validates every property/reference and measures the actual
        // sparse stiffness layout. No mutable spatial fields are constructed.
        let model = CompiledPhysicsProof::compile_with_resources_and_budget(
            proof.clone(),
            geometry,
            material,
            host_budget,
        )?;
        self.used_proofs.insert(reference.id.clone());
        Ok(ProfileStorage {
            layout: model.storage_layout(),
            compiled_model_digest: Some(model.digest().into()),
            compiler_scratch_bytes: model.memory().compiler_scratch_bytes,
            profile,
        })
    }

    pub fn finish(self) -> Result<(), String> {
        if self.used_proofs.len() != self.proofs.len()
            || self.used_geometries.len() != self.geometries.len()
            || self.used_materials.len() != self.materials.len()
        {
            return Err("unused physics capacity source document".into());
        }
        Ok(())
    }
}

fn indexed<T>(documents: Vec<T>, id: impl Fn(&T) -> &str) -> Result<BTreeMap<String, T>, String> {
    let mut result = BTreeMap::new();
    for document in documents {
        let key = id(&document).to_owned();
        valid_id(&key)?;
        if result.insert(key.clone(), document).is_some() {
            return Err(format!("duplicate physics capacity source {key}"));
        }
    }
    Ok(result)
}
