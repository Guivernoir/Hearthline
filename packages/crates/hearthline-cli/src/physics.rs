use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use clap::Subcommand;
use hearthline_config::physics::PhysicsProofConfig;
use hearthline_config::physics::capacity::{CapacityModelReference, PhysicsCapacityConfig};
use hearthline_config::physics::geometry::{GeometryReference, SolidGeometryDocument};
use hearthline_config::physics::material::{DryingMaterialDocument, MaterialReference};
use hearthline_project::physics::CompiledPhysicsProof;
use hearthline_project::physics::capacity::{
    CapacitySources, PhysicsCapacityReport, proof_source_digest,
};
use hearthline_project::physics::geometry::geometry_digest;
use hearthline_project::physics::material::material_digest;
use hearthline_sim::physics::population::PooledCapacityComparison;
use hearthline_sim::physics::{PHYSICS_REPLAY_DOCUMENT_LIMIT, PhysicsReplay};

#[derive(Subcommand)]
pub enum PhysicsCommand {
    /// Execute an isolated numerical proof; this does not migrate the factory.
    Prove {
        config: PathBuf,
        #[arg(long)]
        record: Option<PathBuf>,
        #[arg(long)]
        geometry: Option<PathBuf>,
        #[arg(long)]
        material: Option<PathBuf>,
    },
    /// Recompute spatial fields and verify every declared checkpoint exactly.
    Replay { artifact: PathBuf },
    /// Report preallocated proof buffers; not a full-factory memory estimate.
    Budget {
        config: PathBuf,
        #[arg(long)]
        geometry: Option<PathBuf>,
        #[arg(long)]
        material: Option<PathBuf>,
    },
    /// Compute a reference for an independently authored solid geometry document.
    Geometry { document: PathBuf },
    /// Compute a digest reference for a moisture-dependent drying material.
    Material { document: PathBuf },
    /// Compute a typed proof-source reference for a capacity profile.
    ProofReference { document: PathBuf },
    /// Estimate concurrent populations; does not qualify missing factory physics.
    Capacity {
        config: PathBuf,
        #[arg(long)]
        require_fit: bool,
        /// Compare unchanged meshes and workloads with shared profile workers.
        #[arg(long)]
        pooled: bool,
        /// Repeat for each referenced numerical proof source.
        #[arg(long)]
        proof: Vec<PathBuf>,
        #[arg(long)]
        geometry: Vec<PathBuf>,
        #[arg(long)]
        material: Vec<PathBuf>,
    },
}

pub fn execute(command: PhysicsCommand) -> Result<(), Box<dyn Error>> {
    match command {
        PhysicsCommand::Prove {
            config,
            record,
            geometry,
            material,
        } => {
            let config: PhysicsProofConfig = serde_yaml_ng::from_str(&read_bounded(&config)?)?;
            let replay = PhysicsReplay::record_with_resources(
                config,
                read_geometry(geometry)?,
                read_material(material)?,
            )?;
            if let Some(path) = record {
                super::write_atomic(&path, &(serde_json::to_string(&replay)? + "\n"))?;
            }
            println!(
                "{}: {} physical steps, {} us, digest {}; numerical proof only, not plant qualification",
                replay.config.id,
                replay.final_state.step,
                replay.final_state.time_us,
                replay.final_state.digest
            );
        }
        PhysicsCommand::Replay { artifact } => {
            let replay: PhysicsReplay = serde_json::from_str(&read_bounded(&artifact)?)?;
            replay.verify()?;
            println!(
                "verified physics replay {}: {}",
                replay.config.id, replay.final_state.digest
            );
        }
        PhysicsCommand::Budget {
            config,
            geometry,
            material,
        } => {
            let config: PhysicsProofConfig = serde_yaml_ng::from_str(&read_bounded(&config)?)?;
            let compiled = CompiledPhysicsProof::compile_with_resources(
                config,
                read_geometry(geometry)?,
                read_material(material)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&compiled.memory())?);
        }
        PhysicsCommand::Geometry { document } => {
            let document = SolidGeometryDocument::from_yaml(&read_bounded(&document)?)?;
            let reference = GeometryReference {
                sha256: geometry_digest(&document)?,
                id: document.id,
            };
            println!("{}", serde_json::to_string_pretty(&reference)?);
        }
        PhysicsCommand::Material { document } => {
            let document = DryingMaterialDocument::from_yaml(&read_bounded(&document)?)?;
            let reference = MaterialReference {
                sha256: material_digest(&document)?,
                id: document.id,
            };
            println!("{}", serde_json::to_string_pretty(&reference)?);
        }
        PhysicsCommand::ProofReference { document } => {
            let config: PhysicsProofConfig = serde_yaml_ng::from_str(&read_bounded(&document)?)?;
            let reference = CapacityModelReference {
                sha256: proof_source_digest(&config)?,
                id: config.id,
            };
            println!("{}", serde_json::to_string_pretty(&reference)?);
        }
        PhysicsCommand::Capacity {
            config,
            require_fit,
            pooled,
            proof,
            geometry,
            material,
        } => {
            let config = PhysicsCapacityConfig::from_yaml(&read_bounded(&config)?)?;
            let mut remaining = PHYSICS_REPLAY_DOCUMENT_LIMIT;
            let mut read = |path: PathBuf| -> Result<String, Box<dyn Error>> {
                let text = read_bounded(&path)?;
                remaining = remaining
                    .checked_sub(text.len() as u64)
                    .ok_or("capacity source documents exceed 16 MiB in total")?;
                Ok(text)
            };
            let sources = CapacitySources {
                proofs: proof
                    .into_iter()
                    .map(|path| Ok(serde_yaml_ng::from_str(&read(path)?)?))
                    .collect::<Result<_, Box<dyn Error>>>()?,
                geometries: geometry
                    .into_iter()
                    .map(|path| Ok(SolidGeometryDocument::from_yaml(&read(path)?)?))
                    .collect::<Result<_, Box<dyn Error>>>()?,
                materials: material
                    .into_iter()
                    .map(|path| Ok(DryingMaterialDocument::from_yaml(&read(path)?)?))
                    .collect::<Result<_, Box<dyn Error>>>()?,
            };
            if pooled {
                let report = PooledCapacityComparison::compile_with_sources(config, sources)?;
                println!("{}", serde_json::to_string_pretty(&report)?);
                if require_fit {
                    report.require_fit()?;
                }
            } else {
                let report = PhysicsCapacityReport::compile_with_sources(config, sources)?;
                println!("{}", serde_json::to_string_pretty(&report)?);
                if require_fit {
                    report.require_fit()?;
                }
            }
        }
    }
    Ok(())
}

fn read_geometry(path: Option<PathBuf>) -> Result<Option<SolidGeometryDocument>, Box<dyn Error>> {
    path.map(|path| Ok(SolidGeometryDocument::from_yaml(&read_bounded(&path)?)?))
        .transpose()
}

fn read_material(path: Option<PathBuf>) -> Result<Option<DryingMaterialDocument>, Box<dyn Error>> {
    path.map(|path| Ok(DryingMaterialDocument::from_yaml(&read_bounded(&path)?)?))
        .transpose()
}

fn read_bounded(path: &Path) -> Result<String, Box<dyn Error>> {
    let mut result = String::new();
    File::open(path)?
        .take(PHYSICS_REPLAY_DOCUMENT_LIMIT + 1)
        .read_to_string(&mut result)?;
    if result.len() as u64 > PHYSICS_REPLAY_DOCUMENT_LIMIT {
        return Err("physics document exceeds 16 MiB".into());
    }
    Ok(result)
}
