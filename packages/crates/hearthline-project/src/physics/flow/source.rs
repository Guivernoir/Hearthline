//! Typed SI source adapter for an isolated verification island. The production
//! appliance/connection compiler will reuse the same lower-level gas compiler.

use hearthline_config::physics::connected::{
    CONNECTED_GAS_SOURCE_SCHEMA, ConnectedGasDocument, GasBoundaryConfig, GasPrimitiveConfig,
    GasRegionIdentity,
};
use hearthline_model::ComponentId;
use hearthline_physics::Scalar;
use hearthline_physics::flow::{FlowBoundary, Primitive};
use hearthline_physics::transport::SolverLimits;

use super::connected::{
    CompiledConnectedGas, ConnectedGasProfile, GasReference, GasRegion, GasRetryConfig,
    RegionInitialGas,
};
use super::mesh::{FlowPatch, FlowPatchConnection};
use crate::physics::schedule::PhysicsInstanceIdentity;

mod memory;

impl CompiledConnectedGas {
    pub fn compile_document(
        document: &ConnectedGasDocument,
        host_budget: u64,
    ) -> Result<Self, String> {
        if document.schema_version != CONNECTED_GAS_SOURCE_SCHEMA {
            return Err("unsupported connected gas source schema".into());
        }
        ComponentId::new(&document.id).map_err(|e| e.to_string())?;
        if document.allocation_budget_bytes > host_budget {
            return Err("connected gas source exceeds host allocation policy".into());
        }
        let source_overhead = memory::source_overhead(document)?;
        if u128::from(source_overhead) * 4 > u128::from(document.allocation_budget_bytes) * 3 {
            return Err("connected source metadata exceeds budget with 25% reserve".into());
        }
        let mut coupled = Vec::with_capacity(
            document
                .regions
                .len()
                .checked_mul(6)
                .ok_or("face count overflow")?,
        );
        let mut regions = Vec::with_capacity(document.regions.len());
        let mut initial = Vec::with_capacity(document.regions.len());
        for region in &document.regions {
            let id = identity(&region.identity)?;
            let faces = region.boundaries.faces();
            let mut boundaries = [FlowBoundary::AdiabaticSlipWall; 6];
            for (face, source) in faces.into_iter().enumerate() {
                if matches!(source, GasBoundaryConfig::Coupled) {
                    coupled.push(((id.clone(), face as u8), false));
                }
                boundaries[face] = boundary(source)
                    .map_err(|error| format!("region {} face {face}: {error}", id.instance))?;
            }
            let state = primitive(&region.initial)
                .map_err(|e| format!("region {} initial state: {e}", id.instance))?;
            initial.push(RegionInitialGas {
                identity: id.clone(),
                density_kg_m3: state.density,
                pressure_pa: state.pressure,
                velocity_m_s: state.velocity,
            });
            regions.push(GasRegion {
                identity: id,
                shape: region.shape.map(|n| n as usize),
                spacing_m: vector(&region.spacing_m)?,
                boundaries,
            });
        }
        coupled.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        if coupled.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err("duplicate coupled region face".into());
        }
        let mut connections = Vec::with_capacity(document.connections.len());
        for connection in &document.connections {
            let first = FlowPatch {
                region: identity(&connection.first.region)?,
                face: connection.first.face.index(),
            };
            let second = FlowPatch {
                region: identity(&connection.second.region)?,
                face: connection.second.face.index(),
            };
            for endpoint in [&first, &second] {
                let key = (endpoint.region.clone(), endpoint.face);
                match coupled.binary_search_by(|entry| entry.0.cmp(&key)) {
                    Ok(index) if !coupled[index].1 => coupled[index].1 = true,
                    _ => {
                        return Err(format!(
                            "connection {} requires an unused coupled face at {}/{}/{}:{}",
                            connection.id,
                            endpoint.region.site,
                            endpoint.region.cell,
                            endpoint.region.instance,
                            endpoint.face
                        ));
                    }
                }
            }
            connections.push(FlowPatchConnection {
                id: ComponentId::new(&connection.id).map_err(|e| e.to_string())?,
                first,
                second,
            });
        }
        if coupled.iter().any(|entry| !entry.1) {
            return Err("connected gas source has an unlinked coupled face".into());
        }
        let settings = &document.integration;
        let profile = ConnectedGasProfile {
            reference: GasReference {
                length_m: quantity(&document.reference.length_m)?,
                velocity_m_s: quantity(&document.reference.velocity_m_s)?,
                density_kg_m3: quantity(&document.reference.density_kg_m3)?,
                gas_constant_j_kg_k: quantity(&document.gas.gas_constant_j_kg_k)?,
            },
            heat_capacity_ratio: quantity(&document.gas.heat_capacity_ratio)?,
            dynamic_viscosity_pa_s: quantity(&document.gas.dynamic_viscosity_pa_s)?,
            thermal_conductivity_w_m_k: quantity(&document.gas.thermal_conductivity_w_m_k)?,
            timestep_us: settings.timestep_us,
            steps: settings.steps,
            limits: SolverLimits {
                iterations: settings.maximum_iterations,
                residual: quantity(&settings.residual_tolerance)?,
            },
        };
        Self::compile_inner(
            regions,
            connections,
            initial,
            profile,
            document.allocation_budget_bytes,
            settings.retries.map(|retries| GasRetryConfig {
                maximum_substeps: retries.maximum_substeps,
                minimum_timestep_us: retries.minimum_timestep_us,
            }),
            source_overhead,
        )
    }
}

fn identity(source: &GasRegionIdentity) -> Result<PhysicsInstanceIdentity, String> {
    PhysicsInstanceIdentity::new(&source.site, &source.cell, &source.instance)
}
fn quantity(source: &str) -> Result<Scalar, String> {
    source
        .parse::<Scalar>()
        .map_err(|e| format!("invalid physical decimal {source:?}: {e}"))
}
fn vector(source: &[String; 3]) -> Result<[Scalar; 3], String> {
    Ok([
        quantity(&source[0])?,
        quantity(&source[1])?,
        quantity(&source[2])?,
    ])
}
fn primitive(source: &GasPrimitiveConfig) -> Result<Primitive, String> {
    Ok(Primitive {
        density: quantity(&source.density_kg_m3)?,
        pressure: quantity(&source.pressure_absolute_pa)?,
        velocity: vector(&source.velocity_m_s)?,
    })
}
fn boundary(source: &GasBoundaryConfig) -> Result<FlowBoundary, String> {
    Ok(match source {
        GasBoundaryConfig::Coupled | GasBoundaryConfig::AdiabaticSlipWall => {
            FlowBoundary::AdiabaticSlipWall
        }
        GasBoundaryConfig::NoSlipWall {
            velocity_m_s,
            temperature_k,
        } => FlowBoundary::NoSlipWall {
            velocity: vector(velocity_m_s)?,
            temperature: temperature_k.as_deref().map(quantity).transpose()?,
        },
        GasBoundaryConfig::Reservoir { state } => FlowBoundary::Reservoir(primitive(state)?),
    })
}
