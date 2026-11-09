use hearthline_config::physics::{MechanicsInitialGuess, SpecimenMechanicsConfig, SpecimenSupport};
use hearthline_physics::Scalar;
use hearthline_physics::mechanics::{IsotropicElasticity, Tetrahedron, Vector3};
use hearthline_physics::transport::{Grid3d, SolverLimits};

use super::geometry::CompiledSolidDomain;
use super::{error, nonnegative, positive};

mod assembly;
pub(super) use assembly::storage_bound;

pub(super) const RECONSTRUCTION_CONTRACT: &str = "hearthline-elastic-reconstruction/1:zero-start:csr-ssor:matrix-free-jacobi:full-equilibrium:exact-projections";

/// Six positively oriented tetrahedra per FV voxel. X-fast node/element
/// numbering and the common body diagonal give conforming shared faces.
#[derive(Clone, Debug)]
pub struct CompiledSpecimenMechanics {
    positions: Vec<Vector3>,
    elements: Vec<Tetrahedron>,
    materials: Vec<IsotropicElasticity>,
    constraints: Vec<Option<Scalar>>,
    length_scale: Scalar,
    stress_scale: Scalar,
    moisture_expansion: Scalar,
    thermal_expansion: Scalar,
    limits: SolverLimits,
    stiffness: Option<assembly::CompiledStiffness>,
    initial_guess: MechanicsInitialGuess,
}

impl CompiledSpecimenMechanics {
    pub(super) fn compile(
        config: &SpecimenMechanicsConfig,
        grid: Grid3d,
        domain: Option<&CompiledSolidDomain>,
    ) -> Result<Self, String> {
        let length_scale = positive(&config.length_scale_m)?;
        let stress_scale = positive(&config.stress_scale_pa)?;
        let young = positive(&config.young_modulus_pa)?
            .checked_div(stress_scale)
            .map_err(error)?;
        let material =
            IsotropicElasticity::new(young, config.poisson_ratio.parse().map_err(error)?)
                .map_err(error)?;
        let limits = SolverLimits {
            iterations: config.maximum_iterations,
            residual: positive(&config.residual_tolerance)?,
        };
        if limits.iterations == 0 {
            return Err("mechanical iteration count must be positive".into());
        }
        let moisture_expansion = nonnegative(&config.moisture_expansion_per_kg_kg)?;
        let thermal_expansion = nonnegative(&config.thermal_expansion_per_k)?;
        let shape = grid.shape();
        let node_shape = shape.map(|v| v + 1);
        let (nodes, element_count) = match domain {
            Some(domain) => (
                domain.plan().nodes as usize,
                domain
                    .lattice_ids()
                    .len()
                    .checked_mul(6)
                    .ok_or("mechanical element count overflow")?,
            ),
            None => counts(grid)?,
        };
        let sparse_nodes = domain.map(|domain| domain.plan().node_ids()).transpose()?;
        let mut positions = reserved(nodes)?;
        let spacing = grid.spacing();
        for node in 0..nodes {
            let lattice = sparse_nodes.as_ref().map_or(node, |ids| ids[node] as usize);
            let [x, y, z] = [
                lattice % node_shape[0],
                (lattice / node_shape[0]) % node_shape[1],
                lattice / (node_shape[0] * node_shape[1]),
            ];
            let mut position = [Scalar::ZERO; 3];
            for (axis, coordinate) in [x, y, z].into_iter().enumerate() {
                position[axis] =
                    Scalar::integer(i64::try_from(coordinate).map_err(|e| e.to_string())?)
                        .and_then(|v| v.checked_mul(spacing[axis]))
                        .and_then(|v| v.checked_div(length_scale))
                        .map_err(error)?;
            }
            positions.push(position);
        }
        let node_id = |x, y, z| -> Result<usize, String> {
            let lattice = x + node_shape[0] * (y + node_shape[1] * z);
            sparse_nodes.as_ref().map_or(Ok(lattice), |ids| {
                ids.binary_search(&(lattice as u32))
                    .map_err(|_| "solid element references an inactive node".into())
            })
        };
        let mut elements = reserved(element_count)?;
        for cell in 0..element_count / 6 {
            let lattice = domain.map_or(cell, |domain| domain.lattice_ids()[cell] as usize);
            let [x, y, z] = [
                lattice % shape[0],
                (lattice / shape[0]) % shape[1],
                lattice / (shape[0] * shape[1]),
            ];
            let mut corners = [0; 8];
            for (i, corner) in corners.iter_mut().enumerate() {
                *corner = node_id(x + i % 2, y + (i / 2) % 2, z + i / 4)?;
            }
            for local in [
                [0, 1, 3, 7],
                [0, 3, 2, 7],
                [0, 2, 6, 7],
                [0, 6, 4, 7],
                [0, 4, 5, 7],
                [0, 5, 1, 7],
            ] {
                elements.push(
                    Tetrahedron::compile(local.map(|i| corners[i]), &positions).map_err(error)?,
                );
            }
        }
        let mut materials = reserved(element_count)?;
        materials.resize(element_count, material);
        let dofs = nodes
            .checked_mul(3)
            .ok_or("mechanical DOF count overflow")?;
        let mut constraints = reserved(dofs)?;
        constraints.resize(dofs, None);
        match config.support {
            SpecimenSupport::Free => {
                let x_corner = node_id(shape[0], 0, 0)?;
                let y_corner = node_id(0, shape[1], 0)?;
                for dof in [
                    0,
                    1,
                    2,
                    3 * x_corner + 1,
                    3 * x_corner + 2,
                    3 * y_corner + 2,
                ] {
                    constraints[dof] = Some(Scalar::ZERO);
                }
            }
            SpecimenSupport::FixedXFace => {
                for z in 0..node_shape[2] {
                    for y in 0..node_shape[1] {
                        let node = node_id(0, y, z)?;
                        constraints[3 * node..3 * node + 3].fill(Some(Scalar::ZERO));
                    }
                }
            }
        }
        let stiffness = domain
            .map(|_| assembly::CompiledStiffness::compile(nodes, &elements, &materials))
            .transpose()?;
        Ok(Self {
            positions,
            elements,
            materials,
            constraints,
            length_scale,
            stress_scale,
            moisture_expansion,
            thermal_expansion,
            limits,
            stiffness,
            initial_guess: config.initial_guess,
        })
    }

    pub fn positions(&self) -> &[Vector3] {
        &self.positions
    }
    pub fn elements(&self) -> &[Tetrahedron] {
        &self.elements
    }
    pub fn materials(&self) -> &[IsotropicElasticity] {
        &self.materials
    }
    pub fn constraints(&self) -> &[Option<Scalar>] {
        &self.constraints
    }
    pub const fn length_scale(&self) -> Scalar {
        self.length_scale
    }
    pub const fn stress_scale(&self) -> Scalar {
        self.stress_scale
    }
    pub const fn moisture_expansion(&self) -> Scalar {
        self.moisture_expansion
    }
    pub const fn thermal_expansion(&self) -> Scalar {
        self.thermal_expansion
    }
    pub const fn limits(&self) -> SolverLimits {
        self.limits
    }
    pub const fn reconstructible(&self) -> bool {
        matches!(self.initial_guess, MechanicsInitialGuess::Zero)
    }
    pub fn stiffness(
        &self,
    ) -> Result<
        Option<hearthline_physics::mechanics::StiffnessMatrix<'_>>,
        hearthline_physics::PhysicsError,
    > {
        self.stiffness
            .as_ref()
            .map(|matrix| matrix.view())
            .transpose()
    }
    pub(super) fn stiffness_bytes(&self) -> u64 {
        self.stiffness.as_ref().map_or(0, |matrix| matrix.bytes())
    }
}

fn counts(grid: Grid3d) -> Result<(usize, usize), String> {
    let nodes = grid
        .shape()
        .into_iter()
        .try_fold(1_usize, |count, n| count.checked_mul(n.checked_add(1)?))
        .ok_or("mechanical node count overflow")?;
    let elements = grid
        .count()
        .checked_mul(6)
        .ok_or("mechanical element count overflow")?;
    Ok((nodes, elements))
}

fn reserved<T>(count: usize) -> Result<Vec<T>, String> {
    let mut buffer = Vec::new();
    buffer.try_reserve_exact(count).map_err(|e| e.to_string())?;
    Ok(buffer)
}
