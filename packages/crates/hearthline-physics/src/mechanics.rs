//! Small-strain isotropic tetrahedral finite elements. Inputs use a consistent
//! nondimensional length/stress scale, chosen before numerical compilation.
//! No fracture, plasticity, contact, or finite-strain claims are made here.

use crate::transport::SolverLimits;
use crate::{PhysicsError, Scalar};

mod sparse;
pub use sparse::StiffnessMatrix;

pub const NORMALIZED_JACOBI_CONTRACT: &str =
    "hearthline-mechanical-preconditioner/1:max-free-diagonal:wide-quotient";

pub const SYMMETRIC_SWEEP_CONTRACT: &str =
    "hearthline-mechanical-preconditioner/2:ssor-omega-one:max-free-diagonal:wide-row";

pub type Vector3 = [Scalar; 3];
pub type Tensor3 = [[Scalar; 3]; 3];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Tetrahedron {
    nodes: [usize; 4],
    gradients: [Vector3; 4],
    volume: Scalar,
}

impl Tetrahedron {
    pub fn compile(nodes: [usize; 4], positions: &[Vector3]) -> Result<Self, PhysicsError> {
        if nodes.iter().any(|n| *n >= positions.len()) {
            return Err(PhysicsError::InvalidGeometry);
        }
        let mut columns = [[Scalar::ZERO; 3]; 3];
        for column in 0..3 {
            for axis in 0..3 {
                columns[column][axis] =
                    positions[nodes[column + 1]][axis].checked_sub(positions[nodes[0]][axis])?;
            }
        }
        let cross_products = [
            cross(columns[1], columns[2])?,
            cross(columns[2], columns[0])?,
            cross(columns[0], columns[1])?,
        ];
        let determinant = dot(columns[0], cross_products[0])?;
        if determinant <= Scalar::ZERO {
            return Err(PhysicsError::InvalidGeometry);
        }
        let volume = determinant.checked_div(Scalar::integer(6)?)?;
        if volume <= Scalar::ZERO {
            return Err(PhysicsError::InvalidGeometry);
        }
        let mut gradients = [[Scalar::ZERO; 3]; 4];
        for node in 0..3 {
            for axis in 0..3 {
                gradients[node + 1][axis] = cross_products[node][axis].checked_div(determinant)?;
                gradients[0][axis] = gradients[0][axis].checked_sub(gradients[node + 1][axis])?;
            }
        }
        Ok(Self {
            nodes,
            gradients,
            volume,
        })
    }
    pub const fn nodes(self) -> [usize; 4] {
        self.nodes
    }
    pub const fn volume(self) -> Scalar {
        self.volume
    }

    /// Integral of B^T C B for one pair of local displacement DOFs. Gradient
    /// products are formed first to preserve exact symmetry after quantization.
    pub fn stiffness(
        self,
        material: IsotropicElasticity,
        row: usize,
        column: usize,
    ) -> Result<Scalar, PhysicsError> {
        if row >= 12 || column >= 12 {
            return Err(PhysicsError::BufferLength);
        }
        let a = self.gradients[row / 3];
        let b = self.gradients[column / 3];
        let i = row % 3;
        let j = column % 3;
        let shear =
            a[j].checked_mul(b[i])?
                .checked_add(if i == j { dot(a, b)? } else { Scalar::ZERO })?;
        material
            .lambda
            .checked_mul(a[i].checked_mul(b[j])?)?
            .checked_add(material.mu.checked_mul(shear)?)?
            .checked_mul(self.volume)
    }

    pub fn strain(&self, displacement: &[Scalar]) -> Result<Tensor3, PhysicsError> {
        if self.nodes.iter().any(|node| {
            node.checked_mul(3)
                .and_then(|i| i.checked_add(2))
                .is_none_or(|i| i >= displacement.len())
        }) {
            return Err(PhysicsError::BufferLength);
        }
        let mut gradient = [[Scalar::ZERO; 3]; 3];
        for (node, shape_gradient) in self.nodes.iter().zip(self.gradients) {
            for component in 0..3 {
                for axis in 0..3 {
                    gradient[component][axis] = gradient[component][axis].checked_add(
                        displacement[node * 3 + component].checked_mul(shape_gradient[axis])?,
                    )?;
                }
            }
        }
        let mut strain = [[Scalar::ZERO; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                strain[i][j] = gradient[i][j]
                    .checked_add(gradient[j][i])?
                    .checked_div(Scalar::integer(2)?)?;
            }
        }
        Ok(strain)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IsotropicElasticity {
    lambda: Scalar,
    mu: Scalar,
}

impl IsotropicElasticity {
    pub fn new(young: Scalar, poisson: Scalar) -> Result<Self, PhysicsError> {
        if young <= Scalar::ZERO
            || poisson <= Scalar::integer(-1)?
            || poisson >= Scalar::ratio(1, 2)?
        {
            return Err(PhysicsError::InvalidCoefficient);
        }
        let one_plus = Scalar::ONE.checked_add(poisson)?;
        let two = Scalar::integer(2)?;
        let mu = young.checked_div(two.checked_mul(one_plus)?)?;
        let lambda = young.checked_mul(poisson)?.checked_div(
            one_plus.checked_mul(Scalar::ONE.checked_sub(two.checked_mul(poisson)?)?)?,
        )?;
        if mu <= Scalar::ZERO {
            return Err(PhysicsError::InvalidCoefficient);
        }
        Ok(Self { lambda, mu })
    }

    pub fn stress(
        self,
        strain: Tensor3,
        isotropic_eigenstrain: Scalar,
    ) -> Result<Tensor3, PhysicsError> {
        let mut elastic = strain;
        for (i, row) in elastic.iter_mut().enumerate() {
            row[i] = row[i].checked_sub(isotropic_eigenstrain)?;
        }
        let trace = elastic[0][0]
            .checked_add(elastic[1][1])?
            .checked_add(elastic[2][2])?;
        let mut stress = [[Scalar::ZERO; 3]; 3];
        let two_mu = self.mu.checked_mul(Scalar::integer(2)?)?;
        for i in 0..3 {
            for j in 0..3 {
                stress[i][j] = two_mu.checked_mul(elastic[i][j])?;
                if i == j {
                    stress[i][j] = stress[i][j].checked_add(self.lambda.checked_mul(trace)?)?;
                }
            }
        }
        Ok(stress)
    }
}

pub struct MechanicsWorkspace<'a> {
    pub trial: &'a mut [Scalar],
    pub residual: &'a mut [Scalar],
    pub preconditioned: &'a mut [Scalar],
    pub direction: &'a mut [Scalar],
    pub product: &'a mut [Scalar],
    pub diagonal: &'a mut [Scalar],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MechanicsReport {
    pub iterations: u32,
    pub residual: Scalar,
}

/// Preconditioned conjugate gradients, matrix-free or compiled CSR.
/// The host supplies
/// one material and eigenstrain per element and constraints per displacement
/// degree of freedom. Trial displacement publishes only on convergence.
pub struct MechanicalSystem<'a> {
    elements: &'a [Tetrahedron],
    materials: &'a [IsotropicElasticity],
    constraints: &'a [Option<Scalar>],
    limits: SolverLimits,
    stiffness: Option<StiffnessMatrix<'a>>,
    normalized_preconditioner: bool,
    symmetric_sweep: bool,
}

#[derive(Clone, Copy)]
enum Force<'a> {
    Assembled(&'a [Scalar]),
    Eigenstrain {
        external: &'a [Scalar],
        eigenstrain: &'a [Scalar],
    },
}

impl Force<'_> {
    fn write(
        self,
        system: &MechanicalSystem<'_>,
        output: &mut [Scalar],
    ) -> Result<(), PhysicsError> {
        match self {
            Self::Assembled(values) => {
                output.copy_from_slice(values);
                Ok(())
            }
            Self::Eigenstrain {
                external,
                eigenstrain,
            } => system.load(external, eigenstrain, output),
        }
    }
}

impl<'a> MechanicalSystem<'a> {
    pub fn new(
        elements: &'a [Tetrahedron],
        materials: &'a [IsotropicElasticity],
        constraints: &'a [Option<Scalar>],
        limits: SolverLimits,
    ) -> Result<Self, PhysicsError> {
        if elements.is_empty()
            || materials.len() != elements.len()
            || constraints.is_empty()
            || !constraints.len().is_multiple_of(3)
            || elements
                .iter()
                .flat_map(|e| e.nodes)
                .any(|n| n >= constraints.len() / 3)
        {
            return Err(PhysicsError::BufferLength);
        }
        if limits.iterations == 0 || limits.residual <= Scalar::ZERO {
            return Err(PhysicsError::InvalidCoefficient);
        }
        Ok(Self {
            elements,
            materials,
            constraints,
            limits,
            stiffness: None,
            normalized_preconditioner: false,
            symmetric_sweep: false,
        })
    }

    pub fn with_stiffness(mut self, stiffness: StiffnessMatrix<'a>) -> Result<Self, PhysicsError> {
        if stiffness.dimension() != self.constraints.len() {
            return Err(PhysicsError::BufferLength);
        }
        self.stiffness = Some(stiffness);
        Ok(self)
    }

    /// A constant positive scaling preserves the exact-arithmetic CG path.
    /// It keeps Jacobi search vectors near force rather than displacement
    /// magnitudes, avoiding unnecessary fixed-point loss for stiff systems.
    pub fn with_normalized_preconditioner(mut self) -> Self {
        self.normalized_preconditioner = true;
        self
    }

    /// SSOR with omega=1, natural ordering and one forward/backward sweep.
    /// Unlike a one-way sweep, this preconditioner is symmetric for CG.
    pub fn with_symmetric_sweep(mut self) -> Result<Self, PhysicsError> {
        if self.stiffness.is_none() {
            return Err(PhysicsError::InvalidCoefficient);
        }
        self.symmetric_sweep = true;
        self.normalized_preconditioner = true;
        Ok(self)
    }

    pub fn apply(&self, displacement: &[Scalar], force: &mut [Scalar]) -> Result<(), PhysicsError> {
        if displacement.len() != self.constraints.len() || force.len() != displacement.len() {
            return Err(PhysicsError::BufferLength);
        }
        if let Some(stiffness) = self.stiffness {
            return stiffness.apply(displacement, force);
        }
        force.fill(Scalar::ZERO);
        for (element, material) in self.elements.iter().zip(self.materials) {
            let stress = material.stress(element.strain(displacement)?, Scalar::ZERO)?;
            for (node, gradient) in element.nodes.iter().zip(element.gradients) {
                for axis in 0..3 {
                    force[3 * node + axis] = force[3 * node + axis]
                        .checked_add(element.volume.checked_mul(dot(stress[axis], gradient)?)?)?;
                }
            }
        }
        Ok(())
    }

    fn load(
        &self,
        external: &[Scalar],
        eigenstrain: &[Scalar],
        force: &mut [Scalar],
    ) -> Result<(), PhysicsError> {
        force.copy_from_slice(external);
        for ((element, material), eigenstrain) in
            self.elements.iter().zip(self.materials).zip(eigenstrain)
        {
            let stress = material.stress([[Scalar::ZERO; 3]; 3], eigenstrain.checked_neg()?)?;
            for (node, gradient) in element.nodes.iter().zip(element.gradients) {
                for axis in 0..3 {
                    force[3 * node + axis] = force[3 * node + axis]
                        .checked_add(element.volume.checked_mul(dot(stress[axis], gradient)?)?)?;
                }
            }
        }
        Ok(())
    }

    fn diagonal(&self, diagonal: &mut [Scalar]) -> Result<(), PhysicsError> {
        if let Some(stiffness) = self.stiffness {
            return stiffness.diagonal(diagonal);
        }
        diagonal.fill(Scalar::ZERO);
        for (element, material) in self.elements.iter().zip(self.materials) {
            for (node, gradient) in element.nodes.iter().zip(element.gradients) {
                let squared = dot(gradient, gradient)?;
                for axis in 0..3 {
                    let stiffness = material
                        .lambda
                        .checked_add(material.mu)?
                        .checked_mul(gradient[axis].checked_mul(gradient[axis])?)?
                        .checked_add(material.mu.checked_mul(squared)?)?
                        .checked_mul(element.volume)?;
                    diagonal[3 * node + axis] = diagonal[3 * node + axis].checked_add(stiffness)?;
                }
            }
        }
        if diagonal
            .iter()
            .zip(self.constraints)
            .any(|(d, c)| c.is_none() && *d <= Scalar::ZERO)
        {
            return Err(PhysicsError::NonPositiveDefinite);
        }
        Ok(())
    }

    pub fn solve(
        &self,
        displacement: &mut [Scalar],
        external: &[Scalar],
        eigenstrain: &[Scalar],
        work: &mut MechanicsWorkspace<'_>,
    ) -> Result<MechanicsReport, PhysicsError> {
        if eigenstrain.len() != self.elements.len() || external.len() != self.constraints.len() {
            return Err(PhysicsError::BufferLength);
        }
        self.solve_load(
            displacement,
            Force::Eigenstrain {
                external,
                eigenstrain,
            },
            work,
        )
    }

    /// Assemble the complete, displacement-independent right-hand side once.
    /// The caller may retain it only while loads and eigenstrains are unchanged.
    pub fn assemble_load(
        &self,
        external: &[Scalar],
        eigenstrain: &[Scalar],
        output: &mut [Scalar],
    ) -> Result<(), PhysicsError> {
        if eigenstrain.len() != self.elements.len()
            || external.len() != self.constraints.len()
            || output.len() != external.len()
        {
            return Err(PhysicsError::BufferLength);
        }
        self.load(external, eigenstrain, output)
    }

    /// Solve against a complete force vector, including any eigenstrain load.
    pub fn solve_loaded(
        &self,
        displacement: &mut [Scalar],
        load: &[Scalar],
        work: &mut MechanicsWorkspace<'_>,
    ) -> Result<MechanicsReport, PhysicsError> {
        if load.len() != self.constraints.len() {
            return Err(PhysicsError::BufferLength);
        }
        self.solve_load(displacement, Force::Assembled(load), work)
    }

    fn solve_load(
        &self,
        displacement: &mut [Scalar],
        load: Force<'_>,
        work: &mut MechanicsWorkspace<'_>,
    ) -> Result<MechanicsReport, PhysicsError> {
        let n = self.constraints.len();
        if [
            displacement.len(),
            work.trial.len(),
            work.residual.len(),
            work.preconditioned.len(),
            work.direction.len(),
            work.product.len(),
            work.diagonal.len(),
        ]
        .iter()
        .any(|len| *len != n)
        {
            return Err(PhysicsError::BufferLength);
        }
        self.diagonal(work.diagonal)?;
        work.trial.copy_from_slice(displacement);
        for (trial, constraint) in work.trial.iter_mut().zip(self.constraints) {
            if let Some(value) = constraint {
                *trial = *value;
            }
        }
        self.apply(work.trial, work.product)?;
        load.write(self, work.residual)?;
        for i in 0..n {
            work.residual[i] = if self.constraints[i].is_some() {
                Scalar::ZERO
            } else {
                work.residual[i].checked_sub(work.product[i])?
            };
        }
        let mut residual = norm(work.residual)?;
        if residual <= self.limits.residual {
            displacement.copy_from_slice(work.trial);
            return Ok(MechanicsReport {
                iterations: 0,
                residual,
            });
        }
        self.precondition(work.residual, work.diagonal, work.preconditioned)?;
        work.direction.copy_from_slice(work.preconditioned);
        let mut rz = dot_wide(work.residual, work.preconditioned)?;
        for iteration in 1..=self.limits.iterations {
            self.apply(work.direction, work.product)?;
            for (value, constraint) in work.product.iter_mut().zip(self.constraints) {
                if constraint.is_some() {
                    *value = Scalar::ZERO;
                }
            }
            let denominator = dot_wide(work.direction, work.product)?;
            if denominator <= 0 || rz <= 0 {
                return Err(PhysicsError::NonPositiveDefinite);
            }
            let alpha = Scalar::ratio_wide(rz, denominator)?;
            for i in 0..n {
                work.trial[i] = work.trial[i].checked_add(alpha.checked_mul(work.direction[i])?)?;
            }
            let mut restarted = false;
            if self.stiffness.is_some() {
                for i in 0..n {
                    work.residual[i] =
                        work.residual[i].checked_sub(alpha.checked_mul(work.product[i])?)?;
                }
                // Recurrence is inexpensive, but only the true assembled force
                // residual can accept a solve. Replace drift and restart CG if
                // quantization prevents the recurrence's apparent convergence.
                if norm(work.residual)? <= self.limits.residual
                    || iteration == self.limits.iterations
                {
                    self.true_residual(load, work)?;
                    restarted = true;
                }
            } else {
                self.true_residual(load, work)?;
            }
            residual = norm(work.residual)?;
            if residual <= self.limits.residual {
                displacement.copy_from_slice(work.trial);
                return Ok(MechanicsReport {
                    iterations: iteration,
                    residual,
                });
            }
            self.precondition(work.residual, work.diagonal, work.preconditioned)?;
            let next_rz = dot_wide(work.residual, work.preconditioned)?;
            let beta = if restarted {
                Scalar::ZERO
            } else {
                Scalar::ratio_wide(next_rz, rz)?
            };
            for i in 0..n {
                work.direction[i] =
                    work.preconditioned[i].checked_add(beta.checked_mul(work.direction[i])?)?;
            }
            rz = next_rz;
        }
        Err(PhysicsError::NonConvergence {
            iterations: self.limits.iterations,
            residual,
        })
    }

    fn true_residual(
        &self,
        load: Force<'_>,
        work: &mut MechanicsWorkspace<'_>,
    ) -> Result<(), PhysicsError> {
        self.apply(work.trial, work.product)?;
        load.write(self, work.residual)?;
        for i in 0..self.constraints.len() {
            work.residual[i] = if self.constraints[i].is_some() {
                Scalar::ZERO
            } else {
                work.residual[i].checked_sub(work.product[i])?
            };
        }
        Ok(())
    }

    fn precondition(
        &self,
        residual: &[Scalar],
        diagonal: &[Scalar],
        output: &mut [Scalar],
    ) -> Result<(), PhysicsError> {
        let scale = if self.normalized_preconditioner {
            diagonal
                .iter()
                .zip(self.constraints)
                .filter_map(|(&d, constraint)| constraint.is_none().then_some(d))
                .max()
                .unwrap_or(Scalar::ONE)
        } else {
            Scalar::ONE
        };
        if self.symmetric_sweep {
            return self
                .stiffness
                .ok_or(PhysicsError::InvalidCoefficient)?
                .symmetric_sweep(residual, diagonal, self.constraints, scale, output);
        }
        for i in 0..output.len() {
            output[i] = if self.constraints[i].is_some() {
                Scalar::ZERO
            } else if self.normalized_preconditioner {
                residual[i].checked_mul_div(scale, diagonal[i])?
            } else {
                residual[i].checked_div(diagonal[i])?
            };
        }
        Ok(())
    }
}

fn dot(a: Vector3, b: Vector3) -> Result<Scalar, PhysicsError> {
    let mut value = Scalar::ZERO;
    for axis in 0..3 {
        value = value.checked_add(a[axis].checked_mul(b[axis])?)?;
    }
    Ok(value)
}
fn cross(a: Vector3, b: Vector3) -> Result<Vector3, PhysicsError> {
    Ok([
        a[1].checked_mul(b[2])?
            .checked_sub(a[2].checked_mul(b[1])?)?,
        a[2].checked_mul(b[0])?
            .checked_sub(a[0].checked_mul(b[2])?)?,
        a[0].checked_mul(b[1])?
            .checked_sub(a[1].checked_mul(b[0])?)?,
    ])
}
fn dot_wide(a: &[Scalar], b: &[Scalar]) -> Result<i128, PhysicsError> {
    a.iter().zip(b).try_fold(0_i128, |sum, (a, b)| {
        sum.checked_add(i128::from(a.raw()) * i128::from(b.raw()))
            .ok_or(PhysicsError::Overflow)
    })
}
fn norm(field: &[Scalar]) -> Result<Scalar, PhysicsError> {
    field
        .iter()
        .try_fold(Scalar::ZERO, |norm, value| Ok(norm.max(value.abs()?)))
}
