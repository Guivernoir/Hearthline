use crate::{PhysicsError, Scalar};

mod conductance;
pub mod enthalpy;
pub mod nonlinear;
mod topology;
use conductance::Conductance;
pub use conductance::{harmonic_conductance, surface_conductance};
pub use topology::{CellNeighbors, EXPOSED_FACE, SparseGrid3d, TransportGrid};

/// Cell-centered Cartesian finite volumes, x fastest, then y, then z.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Grid3d {
    shape: [usize; 3],
    spacing: [Scalar; 3],
    count: usize,
}

impl Grid3d {
    pub fn new(shape: [usize; 3], spacing: [Scalar; 3]) -> Result<Self, PhysicsError> {
        if shape.contains(&0) || spacing.iter().any(|s| *s <= Scalar::ZERO) {
            return Err(PhysicsError::InvalidGeometry);
        }
        let count = shape
            .iter()
            .try_fold(1_usize, |a, b| a.checked_mul(*b))
            .ok_or(PhysicsError::InvalidGeometry)?;
        let grid = Self {
            shape,
            spacing,
            count,
        };
        if grid.volume()? <= Scalar::ZERO {
            return Err(PhysicsError::InvalidGeometry);
        }
        Ok(grid)
    }

    pub const fn count(self) -> usize {
        self.count
    }
    pub const fn shape(self) -> [usize; 3] {
        self.shape
    }
    pub const fn spacing(self) -> [Scalar; 3] {
        self.spacing
    }
    pub fn volume(self) -> Result<Scalar, PhysicsError> {
        self.spacing[0]
            .checked_mul(self.spacing[1])?
            .checked_mul(self.spacing[2])
    }

    pub fn conductance(self, coefficient: Scalar) -> Result<[Scalar; 3], PhysicsError> {
        if coefficient < Scalar::ZERO {
            return Err(PhysicsError::InvalidCoefficient);
        }
        let volume = self.volume()?;
        let mut result = [Scalar::ZERO; 3];
        for (axis, value) in result.iter_mut().enumerate() {
            *value = coefficient.checked_mul(
                volume
                    .checked_div(self.spacing[axis])?
                    .checked_div(self.spacing[axis])?,
            )?;
            if coefficient > Scalar::ZERO && *value == Scalar::ZERO {
                return Err(PhysicsError::InvalidCoefficient);
            }
        }
        Ok(result)
    }

    pub fn face_neighbor(self, cell: usize, face: usize) -> Result<Option<usize>, PhysicsError> {
        if cell >= self.count || face >= 6 {
            return Err(PhysicsError::InvalidGeometry);
        }
        Ok(self.neighbors(cell)[face].map(|(neighbor, _)| neighbor))
    }

    fn neighbors(self, cell: usize) -> [Option<(usize, usize)>; 6] {
        let strides = [1, self.shape[0], self.shape[0] * self.shape[1]];
        let coordinates = [
            cell % self.shape[0],
            (cell / self.shape[0]) % self.shape[1],
            cell / strides[2],
        ];
        let mut result = [None; 6];
        for axis in 0..3 {
            if coordinates[axis] > 0 {
                result[2 * axis] = Some((cell - strides[axis], axis));
            }
            if coordinates[axis] + 1 < self.shape[axis] {
                result[2 * axis + 1] = Some((cell + strides[axis], axis));
            }
        }
        result
    }
}

/// A lumped boundary exchange per cell. Multiple exposed faces are aggregated
/// by the compiler; h * area and the ambient value have equation-specific units.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Boundary {
    pub conductance: Scalar,
    pub ambient: Scalar,
}

impl Boundary {
    pub fn rate(self, value: Scalar) -> Result<Scalar, PhysicsError> {
        self.conductance
            .checked_mul(self.ambient.checked_sub(value)?)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SolverLimits {
    pub iterations: u32,
    pub residual: Scalar,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SolveReport {
    pub iterations: u32,
    pub residual: Scalar,
    /// Positive into the domain, including declared sources.
    pub exchange: Scalar,
    /// Change in stored quantity minus exchange. Never silently discarded.
    pub balance_error: Scalar,
}

/// C du/dt = sum G (u_neighbor - u) + H (ambient - u) + source.
/// Backward Euler with lexicographic Gauss-Seidel and an absolute equation
/// residual. This is scalar transport, not a Navier-Stokes solver.
pub struct TransportSystem<'a> {
    grid: TransportGrid<'a>,
    storage: &'a [Scalar],
    conductance: Conductance<'a>,
    boundary: &'a [Boundary],
    limits: SolverLimits,
}

impl<'a> TransportSystem<'a> {
    pub fn new(
        grid: Grid3d,
        storage: &'a [Scalar],
        conductance: [Scalar; 3],
        boundary: &'a [Boundary],
        limits: SolverLimits,
    ) -> Result<Self, PhysicsError> {
        Self::on_grid(
            TransportGrid::Dense(grid),
            storage,
            conductance,
            boundary,
            limits,
        )
    }

    pub fn on_grid(
        grid: TransportGrid<'a>,
        storage: &'a [Scalar],
        conductance: [Scalar; 3],
        boundary: &'a [Boundary],
        limits: SolverLimits,
    ) -> Result<Self, PhysicsError> {
        Self::with_conductance(
            grid,
            storage,
            Conductance::Uniform(conductance),
            boundary,
            limits,
        )
    }

    /// Per-cell full-distance conductance for each principal direction, not a
    /// raw material diffusivity. A harmonic face value conserves interface flux.
    pub fn heterogeneous(
        grid: TransportGrid<'a>,
        storage: &'a [Scalar],
        cell_conductance: &'a [[Scalar; 3]],
        boundary: &'a [Boundary],
        limits: SolverLimits,
    ) -> Result<Self, PhysicsError> {
        Self::with_conductance(
            grid,
            storage,
            Conductance::Cells(cell_conductance),
            boundary,
            limits,
        )
    }

    fn with_conductance(
        grid: TransportGrid<'a>,
        storage: &'a [Scalar],
        conductance: Conductance<'a>,
        boundary: &'a [Boundary],
        limits: SolverLimits,
    ) -> Result<Self, PhysicsError> {
        if storage.len() != grid.count() || boundary.len() != grid.count() {
            return Err(PhysicsError::BufferLength);
        }
        if storage.iter().any(|s| *s <= Scalar::ZERO)
            || boundary.iter().any(|b| b.conductance < Scalar::ZERO)
            || limits.iterations == 0
            || limits.residual <= Scalar::ZERO
        {
            return Err(PhysicsError::InvalidCoefficient);
        }
        conductance.validate(grid.count())?;
        Ok(Self {
            grid,
            storage,
            conductance,
            boundary,
            limits,
        })
    }

    pub fn inventory(&self, state: &[Scalar]) -> Result<Scalar, PhysicsError> {
        if state.len() != self.grid.count() {
            return Err(PhysicsError::BufferLength);
        }
        state
            .iter()
            .zip(self.storage)
            .try_fold(Scalar::ZERO, |total, (u, c)| {
                total.checked_add(u.checked_mul(*c)?)
            })
    }

    pub fn boundary_rates(
        &self,
        state: &[Scalar],
        output: &mut [Scalar],
    ) -> Result<(), PhysicsError> {
        if state.len() != self.grid.count() || output.len() != self.grid.count() {
            return Err(PhysicsError::BufferLength);
        }
        for ((value, boundary), rate) in state.iter().zip(self.boundary).zip(output.iter_mut()) {
            *rate = boundary.rate(*value)?;
        }
        Ok(())
    }

    pub fn solve(
        &self,
        dt: Scalar,
        state: &mut [Scalar],
        sources: &[Scalar],
        work: &mut [Scalar],
    ) -> Result<SolveReport, PhysicsError> {
        let report = self.solve_trial(dt, state, state, sources, work)?;
        state.copy_from_slice(work);
        Ok(report)
    }

    /// Solve from an immutable previous-time field using a separate initial
    /// iterate. Only caller-owned scratch changes, including on failure.
    pub fn solve_trial(
        &self,
        dt: Scalar,
        old: &[Scalar],
        initial: &[Scalar],
        sources: &[Scalar],
        work: &mut [Scalar],
    ) -> Result<SolveReport, PhysicsError> {
        let n = self.grid.count();
        if old.len() != n || initial.len() != n || sources.len() != n || work.len() != n {
            return Err(PhysicsError::BufferLength);
        }
        if dt <= Scalar::ZERO {
            return Err(PhysicsError::InvalidCoefficient);
        }
        work.copy_from_slice(initial);
        let mut residual = Scalar::ZERO;
        for iteration in 1..=self.limits.iterations {
            let mut changed = false;
            for cell in 0..n {
                let (diagonal, rhs) = self.equation(cell, dt, old, sources, work)?;
                let next = rhs.checked_div(diagonal)?;
                changed |= next != work[cell];
                work[cell] = next;
            }
            residual = self.residual(dt, old, sources, work)?;
            if residual <= self.limits.residual {
                return self.report(dt, old, sources, work, iteration, residual);
            }
            if !changed {
                return Err(PhysicsError::InsufficientResolution);
            }
        }
        Err(PhysicsError::NonConvergence {
            iterations: self.limits.iterations,
            residual,
        })
    }

    pub fn residual(
        &self,
        dt: Scalar,
        old: &[Scalar],
        sources: &[Scalar],
        trial: &[Scalar],
    ) -> Result<Scalar, PhysicsError> {
        if [old.len(), sources.len(), trial.len()]
            .iter()
            .any(|n| *n != self.grid.count())
        {
            return Err(PhysicsError::BufferLength);
        }
        if dt <= Scalar::ZERO {
            return Err(PhysicsError::InvalidCoefficient);
        }
        let mut residual = Scalar::ZERO;
        for cell in 0..self.grid.count() {
            let (diagonal, rhs) = self.equation(cell, dt, old, sources, trial)?;
            residual = residual.max(diagonal.checked_mul(trial[cell])?.checked_sub(rhs)?.abs()?);
        }
        Ok(residual)
    }

    fn report(
        &self,
        dt: Scalar,
        old: &[Scalar],
        sources: &[Scalar],
        trial: &[Scalar],
        iterations: u32,
        residual: Scalar,
    ) -> Result<SolveReport, PhysicsError> {
        let mut exchange = Scalar::ZERO;
        for cell in 0..self.grid.count() {
            let flux = self.boundary[cell]
                .conductance
                .checked_mul(self.boundary[cell].ambient.checked_sub(trial[cell])?)?
                .checked_add(sources[cell])?;
            exchange = exchange.checked_add(dt.checked_mul(flux)?)?;
        }
        let balance_error = self
            .inventory(trial)?
            .checked_sub(self.inventory(old)?)?
            .checked_sub(exchange)?;
        Ok(SolveReport {
            iterations,
            residual,
            exchange,
            balance_error,
        })
    }

    fn equation(
        &self,
        cell: usize,
        dt: Scalar,
        old: &[Scalar],
        sources: &[Scalar],
        trial: &[Scalar],
    ) -> Result<(Scalar, Scalar), PhysicsError> {
        let boundary = self.boundary[cell];
        let boundary_dt = dt.checked_mul(boundary.conductance)?;
        if boundary.conductance > Scalar::ZERO && boundary_dt == Scalar::ZERO {
            return Err(PhysicsError::InsufficientResolution);
        }
        let mut diagonal = self.storage[cell].checked_add(boundary_dt)?;
        let mut rhs = self.storage[cell]
            .checked_mul(old[cell])?
            .checked_add(boundary_dt.checked_mul(boundary.ambient)?)?
            .checked_add(dt.checked_mul(sources[cell])?)?;
        for (neighbor, axis) in self.grid.neighbors(cell).into_iter().flatten() {
            let conductance = self.conductance.face(cell, neighbor, axis)?;
            let coefficient = dt.checked_mul(conductance)?;
            if conductance > Scalar::ZERO && coefficient == Scalar::ZERO {
                return Err(PhysicsError::InsufficientResolution);
            }
            diagonal = diagonal.checked_add(coefficient)?;
            rhs = rhs.checked_add(coefficient.checked_mul(trial[neighbor])?)?;
        }
        Ok((diagonal, rhs))
    }
}
