use hearthline_config::physics::connected::{
    ConnectedGasDocument, GasBoundaryConfig, GasConnectionConfig, GasPrimitiveConfig,
    GasRegionConfig, GasRegionIdentity,
};

use crate::physics::schedule::PhysicsInstanceIdentity;

/// Source DTO allocations and the adapter's face-validation table coexist with
/// the mesh compiler. Mutable simulation fields are allocated after they drop.
pub(super) fn source_overhead(document: &ConnectedGasDocument) -> Result<u64, String> {
    let mut bytes = size_of::<ConnectedGasDocument>();
    let mut add = |n: usize| -> Result<(), String> {
        bytes = bytes
            .checked_add(n)
            .ok_or("connected source memory overflow")?;
        Ok(())
    };
    add(document
        .regions
        .capacity()
        .checked_mul(size_of::<GasRegionConfig>())
        .ok_or("source memory overflow")?)?;
    add(document
        .connections
        .capacity()
        .checked_mul(size_of::<GasConnectionConfig>())
        .ok_or("source memory overflow")?)?;
    add(document
        .regions
        .len()
        .checked_mul(6 * size_of::<((PhysicsInstanceIdentity, u8), bool)>())
        .ok_or("source face memory overflow")?)?;
    for text in [
        &document.schema_version,
        &document.id,
        &document.reference.length_m,
        &document.reference.velocity_m_s,
        &document.reference.density_kg_m3,
        &document.gas.gas_constant_j_kg_k,
        &document.gas.heat_capacity_ratio,
        &document.gas.dynamic_viscosity_pa_s,
        &document.gas.thermal_conductivity_w_m_k,
        &document.integration.residual_tolerance,
    ] {
        add(text.capacity())?;
    }
    for region in &document.regions {
        add(identity_bytes(&region.identity))?;
        for text in &region.spacing_m {
            add(text.capacity())?;
        }
        add(primitive_bytes(&region.initial))?;
        for boundary in region.boundaries.faces() {
            match boundary {
                GasBoundaryConfig::NoSlipWall {
                    velocity_m_s,
                    temperature_k,
                } => {
                    for text in velocity_m_s {
                        add(text.capacity())?;
                    }
                    add(temperature_k.as_ref().map_or(0, String::capacity))?;
                }
                GasBoundaryConfig::Reservoir { state } => add(primitive_bytes(state))?,
                _ => {}
            }
        }
    }
    for connection in &document.connections {
        add(connection.id.capacity())?;
        add(identity_bytes(&connection.first.region))?;
        add(identity_bytes(&connection.second.region))?;
    }
    u64::try_from(bytes).map_err(|e| e.to_string())
}
fn identity_bytes(identity: &GasRegionIdentity) -> usize {
    identity.site.capacity() + identity.cell.capacity() + identity.instance.capacity()
}
fn primitive_bytes(state: &GasPrimitiveConfig) -> usize {
    state.density_kg_m3.capacity()
        + state.pressure_absolute_pa.capacity()
        + state
            .velocity_m_s
            .iter()
            .map(String::capacity)
            .sum::<usize>()
}
