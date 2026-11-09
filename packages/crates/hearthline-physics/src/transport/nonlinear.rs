use crate::material::PropertyTable;
use crate::{PhysicsError, Scalar};

use super::{
    Boundary, SolveReport, SolverLimits, TransportGrid, TransportSystem, surface_conductance,
};

pub const NONLINEAR_DIFFUSION_CONTRACT: &str =
    "hearthline-diffusion/1:harmonic:picard:surface-resistance";

#[derive(Clone, Copy, Debug)]
pub struct NonlinearLimits {
    pub linear: SolverLimits,
    pub iterations: u32,
    pub residual: Scalar,
    pub relaxation: Scalar,
}

/// The same environment at every exposed face, including sparse-domain cavities.
/// Per-axis film conductance is h * face area, before half-cell resistance.
#[derive(Clone, Copy, Debug)]
pub enum SurfaceBoundary {
    Insulated,
    FixedValue(Scalar),
    Film {
        conductance: [Scalar; 3],
        ambient: Scalar,
    },
}

pub struct NonlinearWorkspace<'a> {
    pub trial: &'a mut [Scalar],
    pub linear: &'a mut [Scalar],
    pub cell_conductance: &'a mut [[Scalar; 3]],
    pub boundary: &'a mut [Boundary],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NonlinearReport {
    pub iterations: u32,
    pub linear_sweeps: u64,
    pub transport: SolveReport,
}

/// C du/dt = div(k(u) grad u) + source on orthogonal control volumes.
/// Storage is time-independent; state-dependent heat capacity needs a conserved
/// enthalpy formulation, not replacement of C at each Picard iteration.
pub struct NonlinearDiffusion<'a> {
    grid: TransportGrid<'a>,
    storage: &'a [Scalar],
    reference: [Scalar; 3],
    multiplier: PropertyTable<'a>,
    surface: SurfaceBoundary,
    limits: NonlinearLimits,
}

impl<'a> NonlinearDiffusion<'a> {
    pub fn new(
        grid: TransportGrid<'a>,
        storage: &'a [Scalar],
        reference: [Scalar; 3],
        multiplier: PropertyTable<'a>,
        surface: SurfaceBoundary,
        limits: NonlinearLimits,
    ) -> Result<Self, PhysicsError> {
        if storage.len() != grid.count() {
            return Err(PhysicsError::BufferLength);
        }
        if storage.iter().any(|v| *v <= Scalar::ZERO)
            || reference.iter().any(|v| *v < Scalar::ZERO)
            || limits.iterations == 0
            || limits.linear.iterations == 0
            || limits.linear.residual <= Scalar::ZERO
            || limits.residual < limits.linear.residual
            || limits.relaxation <= Scalar::ZERO
            || limits.relaxation > Scalar::ONE
        {
            return Err(PhysicsError::InvalidCoefficient);
        }
        if let SurfaceBoundary::Film { conductance, .. } = surface
            && conductance.iter().any(|v| *v < Scalar::ZERO)
        {
            return Err(PhysicsError::InvalidCoefficient);
        }
        Ok(Self {
            grid,
            storage,
            reference,
            multiplier,
            surface,
            limits,
        })
    }

    pub fn solve(
        &self,
        dt: Scalar,
        state: &mut [Scalar],
        sources: &[Scalar],
        work: &mut NonlinearWorkspace<'_>,
    ) -> Result<NonlinearReport, PhysicsError> {
        let n = self.grid.count();
        if [
            state.len(),
            sources.len(),
            work.trial.len(),
            work.linear.len(),
            work.cell_conductance.len(),
            work.boundary.len(),
        ]
        .iter()
        .any(|v| *v != n)
        {
            return Err(PhysicsError::BufferLength);
        }
        if dt <= Scalar::ZERO {
            return Err(PhysicsError::InvalidCoefficient);
        }
        work.trial.copy_from_slice(state);
        let mut linear_sweeps = 0_u64;
        let mut residual = Scalar::ZERO;
        self.coefficients(work.trial, work.cell_conductance, work.boundary)?;
        for iteration in 1..=self.limits.iterations {
            let report = self
                .system(work.cell_conductance, work.boundary)?
                .solve_trial(dt, state, work.trial, sources, work.linear)?;
            linear_sweeps = linear_sweeps
                .checked_add(u64::from(report.iterations))
                .ok_or(PhysicsError::Overflow)?;
            for (trial, &linear) in work.trial.iter_mut().zip(work.linear.iter()) {
                *trial = trial.checked_add(
                    self.limits
                        .relaxation
                        .checked_mul(linear.checked_sub(*trial)?)?,
                )?;
            }
            // Rebuild coefficients at the candidate before checking the original
            // nonlinear equation. A small iterate change alone is not convergence.
            self.coefficients(work.trial, work.cell_conductance, work.boundary)?;
            let system = self.system(work.cell_conductance, work.boundary)?;
            residual = system.residual(dt, state, sources, work.trial)?;
            if residual <= self.limits.residual {
                let transport =
                    system.report(dt, state, sources, work.trial, iteration, residual)?;
                state.copy_from_slice(work.trial);
                return Ok(NonlinearReport {
                    iterations: iteration,
                    linear_sweeps,
                    transport,
                });
            }
        }
        Err(PhysicsError::NonConvergence {
            iterations: self.limits.iterations,
            residual,
        })
    }

    fn system<'b>(
        &'b self,
        coefficients: &'b [[Scalar; 3]],
        boundary: &'b [Boundary],
    ) -> Result<TransportSystem<'b>, PhysicsError> {
        TransportSystem::heterogeneous(
            self.grid,
            self.storage,
            coefficients,
            boundary,
            self.limits.linear,
        )
    }

    fn coefficients(
        &self,
        state: &[Scalar],
        output: &mut [[Scalar; 3]],
        boundary: &mut [Boundary],
    ) -> Result<(), PhysicsError> {
        for cell in 0..self.grid.count() {
            let multiplier = self.multiplier.value(state[cell])?;
            for (coefficient, &reference) in output[cell].iter_mut().zip(&self.reference) {
                *coefficient = reference.checked_mul(multiplier)?;
                if reference > Scalar::ZERO
                    && multiplier > Scalar::ZERO
                    && *coefficient == Scalar::ZERO
                {
                    return Err(PhysicsError::InsufficientResolution);
                }
            }
            boundary[cell] = Boundary::default();
            for (face, neighbor) in self.grid.neighbors(cell).into_iter().enumerate() {
                if neighbor.is_some() {
                    continue;
                }
                let axis = face / 2;
                let (conductance, ambient) = match self.surface {
                    SurfaceBoundary::Insulated => (Scalar::ZERO, Scalar::ZERO),
                    SurfaceBoundary::FixedValue(ambient) => (
                        output[cell][axis].checked_mul(Scalar::integer(2)?)?,
                        ambient,
                    ),
                    SurfaceBoundary::Film {
                        conductance,
                        ambient,
                    } => (
                        surface_conductance(
                            conductance[axis],
                            output[cell][axis].checked_mul(Scalar::integer(2)?)?,
                        )?,
                        ambient,
                    ),
                };
                boundary[cell].conductance = boundary[cell].conductance.checked_add(conductance)?;
                boundary[cell].ambient = ambient;
            }
        }
        Ok(())
    }
}
