use std::mem::size_of;

use hearthline_config::physics::capacity::CapacityKernel;
use hearthline_config::physics::material::DryingMaterialDocument;
use hearthline_physics::Scalar;
use hearthline_physics::flow::Conserved;
use hearthline_physics::material::caloric::CaloricKnot;
use hearthline_physics::mechanics::{IsotropicElasticity, Tetrahedron, Vector3};
use hearthline_physics::transport::Boundary;
use serde::Serialize;

/// Exact numerical-array payloads for the implemented proof runtime. This does
/// not include container metadata, allocator rounding or serialized JSON.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PhysicsStorageLayout {
    pub cells: u64,
    pub nodes: u64,
    pub elements: u64,
    /// Independent variables that must persist between shared-workspace visits.
    pub state_bytes: u64,
    /// Full observable snapshot, including exactly reconstructible projections.
    pub checkpoint_field_bytes: u64,
    pub scratch_and_coefficients_bytes: u64,
    pub immutable_geometry_bytes: u64,
    pub immutable_material_bytes: u64,
}

impl PhysicsStorageLayout {
    pub fn compile(shape: [u32; 3], kernel: CapacityKernel) -> Result<Self, String> {
        if shape.contains(&0) {
            return Err("physics storage shape must have three positive dimensions".into());
        }
        let cells = shape
            .into_iter()
            .try_fold(1_u64, |n, axis| multiply(n, u64::from(axis)))?;
        let (nodes, elements) = if kernel == CapacityKernel::DryingElasticity {
            (
                shape
                    .into_iter()
                    .try_fold(1_u64, |n, axis| multiply(n, u64::from(axis) + 1))?,
                multiply(cells, 6)?,
            )
        } else {
            (0, 0)
        };
        Self::for_mesh(cells, nodes, elements, kernel)
    }

    pub(super) fn for_mesh(
        cells: u64,
        nodes: u64,
        elements: u64,
        kernel: CapacityKernel,
    ) -> Result<Self, String> {
        let (state_bytes, checkpoint_field_bytes, scratch_and_coefficients_bytes) = match kernel {
            CapacityKernel::CompressibleFlow => (
                bytes::<Conserved>(cells, 1)?,
                add(bytes::<Conserved>(cells, 1)?, bytes::<Scalar>(cells, 2)?)?,
                add(bytes::<Conserved>(cells, 4)?, bytes::<Scalar>(cells, 4)?)?,
            ),
            _ => (
                bytes::<Scalar>(cells, 2)?,
                bytes::<Scalar>(cells, 2)?,
                add(bytes::<Scalar>(cells, 6)?, bytes::<Boundary>(cells, 2)?)?,
            ),
        };
        let mut layout = Self {
            cells,
            nodes,
            elements,
            state_bytes,
            checkpoint_field_bytes,
            scratch_and_coefficients_bytes,
            immutable_geometry_bytes: 0,
            immutable_material_bytes: 0,
        };
        if kernel == CapacityKernel::DryingElasticity {
            let dofs = multiply(layout.nodes, 3)?;
            layout.state_bytes = add(layout.state_bytes, bytes::<Scalar>(dofs, 1)?)?;
            layout.checkpoint_field_bytes = add(
                layout.checkpoint_field_bytes,
                add(
                    bytes::<Scalar>(dofs, 1)?,
                    bytes::<Scalar>(layout.elements, 6)?,
                )?,
            )?;
            layout.scratch_and_coefficients_bytes = add(
                layout.scratch_and_coefficients_bytes,
                add(
                    bytes::<Scalar>(dofs, 8)?,
                    bytes::<Scalar>(layout.elements, 13)?,
                )?,
            )?;
            layout.immutable_geometry_bytes = add(
                add(
                    bytes::<Vector3>(layout.nodes, 1)?,
                    bytes::<Tetrahedron>(layout.elements, 1)?,
                )?,
                add(
                    bytes::<IsotropicElasticity>(layout.elements, 1)?,
                    bytes::<Option<Scalar>>(dofs, 1)?,
                )?,
            )?;
        }
        Ok(layout)
    }

    pub fn instance_buffer_bytes(self) -> Result<u64, String> {
        add(self.state_bytes, self.scratch_and_coefficients_bytes)
    }

    pub(super) fn with_reconstructible_elasticity(mut self) -> Result<Self, String> {
        // The shared worker still needs displacement. Only the persistent
        // per-product copy disappears; complete snapshots remain unchanged.
        let displacement = bytes::<Scalar>(self.nodes, 3)?;
        self.state_bytes = self
            .state_bytes
            .checked_sub(displacement)
            .ok_or("invalid reconstructible elasticity layout")?;
        self.scratch_and_coefficients_bytes =
            add(self.scratch_and_coefficients_bytes, displacement)?;
        Ok(self)
    }

    pub fn immutable_bytes(self) -> Result<u64, String> {
        add(self.immutable_geometry_bytes, self.immutable_material_bytes)
    }

    pub(super) fn with_material(
        mut self,
        document: &DryingMaterialDocument,
    ) -> Result<Self, String> {
        // One Picard trial plus three full-distance conductances per cell.
        // The existing moisture boundary vector is reused by the variable law.
        self.scratch_and_coefficients_bytes = add(
            self.scratch_and_coefficients_bytes,
            bytes::<Scalar>(self.cells, 4)?,
        )?;
        self.immutable_material_bytes = add(
            bytes::<[Scalar; 2]>(document.diffusivity_multiplier_by_moisture.len() as u64, 1)?,
            bytes::<CaloricKnot>(
                document
                    .dry_heat_capacity_j_kg_k_by_temperature
                    .as_ref()
                    .map_or(0, Vec::len) as u64,
                1,
            )?,
        )?;
        self.immutable_material_bytes = add(
            self.immutable_material_bytes,
            bytes::<CaloricKnot>(
                document
                    .liquid_heat_capacity_j_kg_k_by_temperature
                    .as_ref()
                    .map_or(0, Vec::len) as u64,
                1,
            )?,
        )?;
        Ok(self)
    }
}

pub(super) fn add(left: u64, right: u64) -> Result<u64, String> {
    left.checked_add(right)
        .ok_or_else(|| "physics resource demand overflow".into())
}

pub(super) fn multiply(left: u64, right: u64) -> Result<u64, String> {
    left.checked_mul(right)
        .ok_or_else(|| "physics resource demand overflow".into())
}

fn bytes<T>(count: u64, copies: u64) -> Result<u64, String> {
    multiply(multiply(count, copies)?, size_of::<T>() as u64)
}
