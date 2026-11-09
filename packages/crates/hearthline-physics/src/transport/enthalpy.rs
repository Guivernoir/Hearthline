use crate::material::caloric::CaloricLaw;
use crate::{PhysicsError, Scalar};

use super::{Boundary, Conductance, SolveReport, TransportGrid};

mod water;
pub use water::{WET_ENTHALPY_CONTRACT, WaterTransport};

pub const ENTHALPY_TRANSPORT_CONTRACT: &str =
    "hearthline-enthalpy-transport/1:backward-euler:bracketed-nonlinear-gs";

#[derive(Clone, Copy, Debug)]
pub struct EnthalpyLimits {
    pub sweeps: u32,
    pub local_iterations: u32,
    /// Absolute cell energy-equation tolerance, not a temperature tolerance.
    pub residual: Scalar,
}

pub struct EnthalpyWorkspace<'a> {
    pub trial: &'a mut [Scalar],
    pub old_enthalpy: &'a mut [Scalar],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnthalpyReport {
    pub transport: SolveReport,
    pub local_iterations: u64,
}

/// m * (h(T_new)-h(T_old)) = dt * (net conductive heat + sources).
/// Each nonlinear Gauss-Seidel row uses a bracketed scalar Newton solve.
/// The final residual uses the original energy equation and immutable old h.
#[derive(Clone, Copy)]
pub struct EnthalpySystem<'a> {
    grid: TransportGrid<'a>,
    mass: &'a [Scalar],
    law: CaloricLaw<'a>,
    conductance: Conductance<'a>,
    boundary: &'a [Boundary],
    limits: EnthalpyLimits,
    water: Option<WaterTransport<'a>>,
}

impl<'a> EnthalpySystem<'a> {
    pub fn new(
        grid: TransportGrid<'a>,
        mass: &'a [Scalar],
        law: CaloricLaw<'a>,
        conductance: [Scalar; 3],
        boundary: &'a [Boundary],
        limits: EnthalpyLimits,
    ) -> Result<Self, PhysicsError> {
        Self::with_conductance(
            grid,
            mass,
            law,
            Conductance::Uniform(conductance),
            boundary,
            limits,
        )
    }

    pub fn heterogeneous(
        grid: TransportGrid<'a>,
        mass: &'a [Scalar],
        law: CaloricLaw<'a>,
        conductance: &'a [[Scalar; 3]],
        boundary: &'a [Boundary],
        limits: EnthalpyLimits,
    ) -> Result<Self, PhysicsError> {
        Self::with_conductance(
            grid,
            mass,
            law,
            Conductance::Cells(conductance),
            boundary,
            limits,
        )
    }

    fn with_conductance(
        grid: TransportGrid<'a>,
        mass: &'a [Scalar],
        law: CaloricLaw<'a>,
        conductance: Conductance<'a>,
        boundary: &'a [Boundary],
        limits: EnthalpyLimits,
    ) -> Result<Self, PhysicsError> {
        if mass.len() != grid.count() || boundary.len() != grid.count() {
            return Err(PhysicsError::BufferLength);
        }
        if mass.iter().any(|v| *v <= Scalar::ZERO)
            || boundary.iter().any(|b| b.conductance < Scalar::ZERO)
            || limits.sweeps == 0
            || limits.local_iterations == 0
            || limits.residual <= Scalar::ZERO
        {
            return Err(PhysicsError::InvalidCoefficient);
        }
        conductance.validate(grid.count())?;
        Ok(Self {
            grid,
            mass,
            law,
            conductance,
            boundary,
            limits,
            water: None,
        })
    }

    pub fn with_water(mut self, water: WaterTransport<'a>) -> Result<Self, PhysicsError> {
        water.validate(self.grid.count(), self.law.domain())?;
        self.water = Some(water);
        Ok(self)
    }

    pub fn inventory(&self, temperature: &[Scalar]) -> Result<Scalar, PhysicsError> {
        self.inventory_at(temperature, false)
    }

    pub fn previous_inventory(&self, temperature: &[Scalar]) -> Result<Scalar, PhysicsError> {
        self.inventory_at(temperature, true)
    }

    fn inventory_at(&self, temperature: &[Scalar], previous: bool) -> Result<Scalar, PhysicsError> {
        if temperature.len() != self.grid.count() {
            return Err(PhysicsError::BufferLength);
        }
        temperature.iter().zip(self.mass).enumerate().try_fold(
            Scalar::ZERO,
            |sum, (cell, (&t, &mass))| {
                let mut sum = sum.checked_add(mass.checked_mul(self.law.enthalpy(t)?)?)?;
                if let Some(water) = self.water {
                    sum = sum.checked_add(water.stored_energy(cell, mass, t, previous)?)?;
                }
                Ok(sum)
            },
        )
    }

    pub fn solve(
        &self,
        dt: Scalar,
        temperature: &mut [Scalar],
        sources: &[Scalar],
        work: &mut EnthalpyWorkspace<'_>,
    ) -> Result<EnthalpyReport, PhysicsError> {
        let n = self.grid.count();
        if [
            temperature.len(),
            sources.len(),
            work.trial.len(),
            work.old_enthalpy.len(),
        ]
        .iter()
        .any(|v| *v != n)
        {
            return Err(PhysicsError::BufferLength);
        }
        if dt <= Scalar::ZERO {
            return Err(PhysicsError::InvalidCoefficient);
        }
        work.trial.copy_from_slice(temperature);
        for (&t, h) in temperature.iter().zip(work.old_enthalpy.iter_mut()) {
            *h = self.law.enthalpy(t)?;
        }
        let mut local_iterations = 0_u64;
        let mut residual = Scalar::ZERO;
        for sweep in 1..=self.limits.sweeps {
            let mut changed = false;
            for (cell, &source) in sources.iter().enumerate() {
                let row = self.row(
                    cell,
                    dt,
                    [work.old_enthalpy[cell], temperature[cell]],
                    source,
                    work.trial,
                )?;
                let (next, iterations) = row.solve(self.law, work.trial[cell], self.limits)?;
                local_iterations = local_iterations
                    .checked_add(u64::from(iterations))
                    .ok_or(PhysicsError::Overflow)?;
                changed |= next != work.trial[cell];
                work.trial[cell] = next;
            }
            residual = Scalar::ZERO;
            for (cell, &source) in sources.iter().enumerate() {
                let row = self.row(
                    cell,
                    dt,
                    [work.old_enthalpy[cell], temperature[cell]],
                    source,
                    work.trial,
                )?;
                residual = residual.max(row.residual(self.law, work.trial[cell])?.abs()?);
            }
            if residual <= self.limits.residual {
                let mut exchange = Scalar::ZERO;
                for (cell, &source) in sources.iter().enumerate() {
                    exchange = exchange.checked_add(scaled_increment(dt, source)?)?;
                    if let Some(water) = self.water {
                        exchange = exchange.checked_add(water.boundary_exchange(
                            cell,
                            dt,
                            work.trial[cell],
                        )?)?;
                    }
                    if self.boundary[cell].conductance > Scalar::ZERO {
                        exchange = exchange.checked_add(
                            scaled_increment(dt, self.boundary[cell].conductance)?.checked_mul(
                                self.boundary[cell].ambient.checked_sub(work.trial[cell])?,
                            )?,
                        )?;
                    }
                }
                let balance_error = self
                    .inventory(work.trial)?
                    .checked_sub(self.previous_inventory(temperature)?)?
                    .checked_sub(exchange)?;
                temperature.copy_from_slice(work.trial);
                return Ok(EnthalpyReport {
                    transport: SolveReport {
                        iterations: sweep,
                        residual,
                        exchange,
                        balance_error,
                    },
                    local_iterations,
                });
            }
            if !changed {
                return Err(PhysicsError::InsufficientResolution);
            }
        }
        Err(PhysicsError::NonConvergence {
            iterations: self.limits.sweeps,
            residual,
        })
    }

    fn row(
        &self,
        cell: usize,
        dt: Scalar,
        old: [Scalar; 2],
        source: Scalar,
        trial: &[Scalar],
    ) -> Result<Row<'a>, PhysicsError> {
        let mut row = Row {
            mass: self.mass[cell],
            old_enthalpy: old[0],
            source: scaled_increment(dt, source)?,
            faces: [(Scalar::ZERO, Scalar::ZERO); 7],
            water: self
                .water
                .map(|water| water.row(self.grid, cell, self.mass[cell], dt, old[1], trial))
                .transpose()?,
        };
        row.faces[0] = (
            scaled_increment(dt, self.boundary[cell].conductance)?,
            self.boundary[cell].ambient,
        );
        for (face, neighbor) in self.grid.neighbors(cell).into_iter().enumerate() {
            if let Some((neighbor, axis)) = neighbor {
                row.faces[face + 1] = (
                    scaled_increment(dt, self.conductance.face(cell, neighbor, axis)?)?,
                    trial[neighbor],
                );
            }
        }
        Ok(row)
    }
}

struct Row<'a> {
    mass: Scalar,
    old_enthalpy: Scalar,
    source: Scalar,
    faces: [(Scalar, Scalar); 7],
    water: Option<water::WaterRow<'a>>,
}

impl Row<'_> {
    fn residual(&self, law: CaloricLaw<'_>, temperature: Scalar) -> Result<Scalar, PhysicsError> {
        let mut residual = self
            .mass
            .checked_mul(law.enthalpy(temperature)?.checked_sub(self.old_enthalpy)?)?
            .checked_sub(self.source)?;
        if let Some(water) = &self.water {
            residual = residual.checked_add(water.residual(temperature)?)?;
        }
        for &(coefficient, neighbor) in &self.faces {
            if coefficient != Scalar::ZERO {
                residual = residual
                    .checked_add(coefficient.checked_mul(temperature.checked_sub(neighbor)?)?)?;
            }
        }
        Ok(residual)
    }

    fn solve(
        &self,
        law: CaloricLaw<'_>,
        initial: Scalar,
        limits: EnthalpyLimits,
    ) -> Result<(Scalar, u32), PhysicsError> {
        let [mut low, mut high] = law.domain();
        let tolerance = Scalar::from_raw((limits.residual.raw() / 4).max(1));
        let low_residual = self.residual(law, low)?;
        let high_residual = self.residual(law, high)?;
        if low_residual > tolerance || high_residual < tolerance.checked_neg()? {
            return Err(PhysicsError::InvalidState);
        }
        let mut current = initial;
        let mut residual = Scalar::ZERO;
        for iteration in 1..=limits.local_iterations {
            let signed = self.residual(law, current)?;
            residual = signed.abs()?;
            if residual <= tolerance {
                return Ok((current, iteration));
            }
            if signed < Scalar::ZERO {
                low = current;
            } else {
                high = current;
            }
            if high
                .raw()
                .checked_sub(low.raw())
                .ok_or(PhysicsError::Overflow)?
                <= 1
            {
                for endpoint in [low, high] {
                    if self.residual(law, endpoint)?.abs()? <= tolerance {
                        return Ok((endpoint, iteration));
                    }
                }
                return Err(PhysicsError::InsufficientResolution);
            }
            let mut capacity = self.mass.checked_mul(law.heat_capacity(current)?)?;
            if let Some(water) = &self.water {
                capacity = capacity.checked_add(water.capacity(current)?)?;
            }
            if capacity == Scalar::ZERO {
                return Err(PhysicsError::InsufficientResolution);
            }
            let diagonal = self
                .faces
                .iter()
                .try_fold(capacity, |sum, face| sum.checked_add(face.0))?;
            let candidate = current.checked_sub(signed.checked_div(diagonal)?)?;
            current = if candidate > low && candidate < high && candidate != current {
                candidate
            } else {
                Scalar::from_raw(
                    low.raw()
                        .checked_add((high.raw() - low.raw()) / 2)
                        .ok_or(PhysicsError::Overflow)?,
                )
            };
        }
        Err(PhysicsError::NonConvergence {
            iterations: limits.local_iterations,
            residual,
        })
    }
}

fn scaled_increment(dt: Scalar, value: Scalar) -> Result<Scalar, PhysicsError> {
    let result = dt.checked_mul(value)?;
    if value != Scalar::ZERO && result == Scalar::ZERO {
        Err(PhysicsError::InsufficientResolution)
    } else {
        Ok(result)
    }
}
