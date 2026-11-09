//! Conservative compressible ideal-gas finite volumes, with optional Newtonian
//! viscosity and Fourier heat conduction. Turbulence, chemical species, porous
//! resistance and equipment maps require separate constitutive extensions.

use crate::transport::{Grid3d, SolverLimits};
use crate::{PhysicsError, Scalar};

mod advance;
mod scales;
mod viscous;
pub use advance::{FlowAdvanceError, FlowAdvanceReport, FlowAdvanceWorkspace, FlowRetryPolicy};
pub use scales::GasScales;
pub use viscous::ViscousTransport;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Conserved {
    pub density: Scalar,
    pub momentum: [Scalar; 3],
    pub total_energy: Scalar,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Primitive {
    pub density: Scalar,
    pub velocity: [Scalar; 3],
    pub pressure: Scalar,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdealGas {
    gamma: Scalar,
}

impl IdealGas {
    pub fn new(gamma: Scalar) -> Result<Self, PhysicsError> {
        if gamma <= Scalar::ONE {
            return Err(PhysicsError::InvalidCoefficient);
        }
        Ok(Self { gamma })
    }
    pub fn conserved(self, state: Primitive) -> Result<Conserved, PhysicsError> {
        if state.density <= Scalar::ZERO || state.pressure <= Scalar::ZERO {
            return Err(PhysicsError::InvalidState);
        }
        let mut kinetic = Scalar::ZERO;
        let mut momentum = [Scalar::ZERO; 3];
        for (axis, value) in momentum.iter_mut().enumerate() {
            *value = state.density.checked_mul(state.velocity[axis])?;
            kinetic = kinetic.checked_add(value.checked_mul(state.velocity[axis])?)?;
        }
        let total_energy = state
            .pressure
            .checked_div(self.gamma.checked_sub(Scalar::ONE)?)?
            .checked_add(kinetic.checked_div(Scalar::integer(2)?)?)?;
        Ok(Conserved {
            density: state.density,
            momentum,
            total_energy,
        })
    }
    pub fn primitive(self, state: Conserved) -> Result<Primitive, PhysicsError> {
        if state.density <= Scalar::ZERO {
            return Err(PhysicsError::InvalidState);
        }
        let mut velocity = [Scalar::ZERO; 3];
        let mut kinetic = Scalar::ZERO;
        for (axis, value) in velocity.iter_mut().enumerate() {
            *value = state.momentum[axis].checked_div(state.density)?;
            kinetic = kinetic.checked_add(state.momentum[axis].checked_mul(*value)?)?;
        }
        let pressure = state
            .total_energy
            .checked_sub(kinetic.checked_div(Scalar::integer(2)?)?)?
            .checked_mul(self.gamma.checked_sub(Scalar::ONE)?)?;
        if pressure <= Scalar::ZERO {
            return Err(PhysicsError::InvalidState);
        }
        Ok(Primitive {
            density: state.density,
            velocity,
            pressure,
        })
    }
    fn acoustic_speed(self, state: Primitive) -> Result<Scalar, PhysicsError> {
        self.gamma
            .checked_mul(state.pressure)?
            .checked_div(state.density)?
            .sqrt()
    }
}

impl Conserved {
    pub fn components(self) -> [Scalar; 5] {
        [
            self.density,
            self.momentum[0],
            self.momentum[1],
            self.momentum[2],
            self.total_energy,
        ]
    }
    pub fn from_components(values: [Scalar; 5]) -> Self {
        Self {
            density: values[0],
            momentum: [values[1], values[2], values[3]],
            total_energy: values[4],
        }
    }
    fn checked_add_scaled(self, rhs: Self, scale: Scalar) -> Result<Self, PhysicsError> {
        let mut components = self.components();
        for (value, rhs) in components.iter_mut().zip(rhs.components()) {
            *value = value.checked_add(rhs.checked_mul(scale)?)?;
        }
        Ok(Self::from_components(components))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlowBoundary {
    AdiabaticSlipWall,
    NoSlipWall {
        velocity: [Scalar; 3],
        temperature: Option<Scalar>,
    },
    Periodic,
    /// Fixed primitive state at the exterior ghost-cell centre.
    Reservoir(Primitive),
}

pub struct FlowWorkspace<'a> {
    pub trial: &'a mut [Conserved],
    pub next: &'a mut [Conserved],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowReport {
    pub iterations: u32,
    pub residual: Scalar,
    pub boundary_exchange: Conserved,
    pub balance_error: Conserved,
}

/// Rusanov face fluxes, backward Euler and damped fixed-point iteration.
/// A failed step changes workspace only. Hosts may retry smaller timesteps
/// deterministically; they must not advance their clock for rejected trials.
#[derive(Clone, Debug)]
pub struct CompressibleFlow {
    grid: Grid3d,
    gas: IdealGas,
    boundaries: [FlowBoundary; 6],
    limits: SolverLimits,
    transport: Option<ViscousTransport>,
}

impl CompressibleFlow {
    pub fn new(
        grid: Grid3d,
        gas: IdealGas,
        boundaries: [FlowBoundary; 6],
        limits: SolverLimits,
    ) -> Result<Self, PhysicsError> {
        Self::build(grid, gas, boundaries, limits, None)
    }

    pub fn new_viscous(
        grid: Grid3d,
        gas: IdealGas,
        boundaries: [FlowBoundary; 6],
        limits: SolverLimits,
        transport: ViscousTransport,
    ) -> Result<Self, PhysicsError> {
        Self::build(grid, gas, boundaries, limits, Some(transport))
    }

    fn build(
        grid: Grid3d,
        gas: IdealGas,
        boundaries: [FlowBoundary; 6],
        limits: SolverLimits,
        transport: Option<ViscousTransport>,
    ) -> Result<Self, PhysicsError> {
        if limits.iterations == 0 || limits.residual <= Scalar::ZERO {
            return Err(PhysicsError::InvalidCoefficient);
        }
        for (face, boundary) in boundaries.into_iter().enumerate() {
            match boundary {
                FlowBoundary::Reservoir(state) => {
                    gas.conserved(state)?;
                }
                FlowBoundary::NoSlipWall {
                    velocity,
                    temperature,
                } => {
                    if transport.is_none()
                        || velocity[face / 2] != Scalar::ZERO
                        || temperature.is_some_and(|t| t <= Scalar::ZERO)
                    {
                        return Err(PhysicsError::InvalidCoefficient);
                    }
                }
                FlowBoundary::Periodic => {
                    if boundaries[face ^ 1] != FlowBoundary::Periodic {
                        return Err(PhysicsError::InvalidCoefficient);
                    }
                }
                FlowBoundary::AdiabaticSlipWall => (),
            }
        }
        Ok(Self {
            grid,
            gas,
            boundaries,
            limits,
            transport,
        })
    }

    pub fn solve(
        &self,
        dt: Scalar,
        state: &mut [Conserved],
        workspace: &mut FlowWorkspace<'_>,
    ) -> Result<FlowReport, PhysicsError> {
        if state.len() != self.grid.count()
            || workspace.trial.len() != state.len()
            || workspace.next.len() != state.len()
        {
            return Err(PhysicsError::BufferLength);
        }
        if dt <= Scalar::ZERO {
            return Err(PhysicsError::InvalidCoefficient);
        }
        for value in state.iter() {
            self.gas.primitive(*value)?;
        }
        workspace.trial.copy_from_slice(state);
        let before = self.inventory(state)?;
        let mut residual = Scalar::ZERO;
        let half = Scalar::ratio(1, 2)?;
        for iteration in 1..=self.limits.iterations {
            let exchange = self.evaluate(dt, state, workspace.trial, workspace.next)?;
            residual = Scalar::ZERO;
            for (next, trial) in workspace.next.iter().zip(workspace.trial.iter()) {
                for (a, b) in next.components().into_iter().zip(trial.components()) {
                    residual = residual.max(a.checked_sub(b)?.abs()?);
                }
            }
            if residual <= self.limits.residual {
                let after = self.inventory(workspace.trial)?;
                let balance_error = after
                    .checked_add_scaled(before, Scalar::integer(-1)?)?
                    .checked_add_scaled(exchange, Scalar::integer(-1)?)?;
                let report = FlowReport {
                    iterations: iteration,
                    residual,
                    boundary_exchange: exchange,
                    balance_error,
                };
                state.copy_from_slice(workspace.trial);
                return Ok(report);
            }
            for (trial, next) in workspace.trial.iter_mut().zip(workspace.next.iter()) {
                let mut values = trial.components();
                for (value, next) in values.iter_mut().zip(next.components()) {
                    *value = value.checked_add(next)?.checked_mul(half)?;
                }
                *trial = Conserved::from_components(values);
                self.gas.primitive(*trial)?;
            }
        }
        Err(PhysicsError::NonConvergence {
            iterations: self.limits.iterations,
            residual,
        })
    }

    pub fn inventory(&self, state: &[Conserved]) -> Result<Conserved, PhysicsError> {
        if state.len() != self.grid.count() {
            return Err(PhysicsError::BufferLength);
        }
        let volume = self.grid.volume()?;
        state.iter().try_fold(Conserved::default(), |total, cell| {
            total.checked_add_scaled(*cell, volume)
        })
    }

    fn evaluate(
        &self,
        dt: Scalar,
        old: &[Conserved],
        trial: &[Conserved],
        next: &mut [Conserved],
    ) -> Result<Conserved, PhysicsError> {
        next.copy_from_slice(old);
        let shape = self.grid.shape();
        let strides = [1, shape[0], shape[0] * shape[1]];
        let spacing = self.grid.spacing();
        let mut exchange = Conserved::default();
        for cell in 0..self.grid.count() {
            for axis in 0..3 {
                let coordinate = (cell / strides[axis]) % shape[axis];
                let scale = dt.checked_div(spacing[axis])?;
                if coordinate + 1 < shape[axis]
                    || self.boundaries[2 * axis + 1] == FlowBoundary::Periodic
                {
                    let neighbor = if coordinate + 1 < shape[axis] {
                        cell + strides[axis]
                    } else {
                        cell - coordinate * strides[axis]
                    };
                    let mut flux = self.flux(trial[cell], trial[neighbor], axis)?;
                    if let Some(transport) = self.transport {
                        flux = flux.checked_add_scaled(
                            self.viscous_face(trial, cell, neighbor, axis, transport)?,
                            Scalar::integer(-1)?,
                        )?;
                    }
                    next[cell] = next[cell].checked_add_scaled(flux, scale.checked_neg()?)?;
                    next[neighbor] = next[neighbor].checked_add_scaled(flux, scale)?;
                } else {
                    let ghost = self.ghost(trial[cell], axis, self.boundaries[2 * axis + 1])?;
                    let mut flux = self.flux(trial[cell], ghost, axis)?;
                    if let Some(transport) = self.transport {
                        flux = flux.checked_add_scaled(
                            self.viscous_boundary(trial, cell, 2 * axis + 1, transport)?,
                            Scalar::integer(-1)?,
                        )?;
                    }
                    next[cell] = next[cell].checked_add_scaled(flux, scale.checked_neg()?)?;
                    exchange = exchange.checked_add_scaled(
                        flux,
                        scale.checked_mul(self.grid.volume()?)?.checked_neg()?,
                    )?;
                }
                if coordinate == 0 && self.boundaries[2 * axis] != FlowBoundary::Periodic {
                    let ghost = self.ghost(trial[cell], axis, self.boundaries[2 * axis])?;
                    let mut flux = self.flux(ghost, trial[cell], axis)?;
                    if let Some(transport) = self.transport {
                        flux = flux.checked_add_scaled(
                            self.viscous_boundary(trial, cell, 2 * axis, transport)?,
                            Scalar::integer(-1)?,
                        )?;
                    }
                    next[cell] = next[cell].checked_add_scaled(flux, scale)?;
                    exchange = exchange
                        .checked_add_scaled(flux, scale.checked_mul(self.grid.volume()?)?)?;
                }
            }
        }
        Ok(exchange)
    }

    fn ghost(
        &self,
        state: Conserved,
        axis: usize,
        boundary: FlowBoundary,
    ) -> Result<Conserved, PhysicsError> {
        match boundary {
            FlowBoundary::Reservoir(state) => self.gas.conserved(state),
            FlowBoundary::AdiabaticSlipWall | FlowBoundary::NoSlipWall { .. } => {
                let mut ghost = state;
                ghost.momentum[axis] = ghost.momentum[axis].checked_neg()?;
                Ok(ghost)
            }
            FlowBoundary::Periodic => Err(PhysicsError::InvalidCoefficient),
        }
    }

    fn flux(
        &self,
        left: Conserved,
        right: Conserved,
        axis: usize,
    ) -> Result<Conserved, PhysicsError> {
        let a = self.gas.primitive(left)?;
        let b = self.gas.primitive(right)?;
        let speed = a.velocity[axis]
            .abs()?
            .checked_add(self.gas.acoustic_speed(a)?)?
            .max(
                b.velocity[axis]
                    .abs()?
                    .checked_add(self.gas.acoustic_speed(b)?)?,
            );
        let f = physical_flux(left, a, axis)?.components();
        let g = physical_flux(right, b, axis)?.components();
        let l = left.components();
        let r = right.components();
        let mut result = [Scalar::ZERO; 5];
        let two = Scalar::integer(2)?;
        for i in 0..5 {
            result[i] = f[i]
                .checked_add(g[i])?
                .checked_sub(speed.checked_mul(r[i].checked_sub(l[i])?)?)?
                .checked_div(two)?;
        }
        Ok(Conserved::from_components(result))
    }
}

fn physical_flux(
    state: Conserved,
    primitive: Primitive,
    axis: usize,
) -> Result<Conserved, PhysicsError> {
    let velocity = primitive.velocity[axis];
    let mut momentum = [Scalar::ZERO; 3];
    for (component, value) in momentum.iter_mut().enumerate() {
        *value = state.momentum[component].checked_mul(velocity)?;
        if component == axis {
            *value = value.checked_add(primitive.pressure)?;
        }
    }
    Ok(Conserved {
        density: state.momentum[axis],
        momentum,
        total_energy: state
            .total_energy
            .checked_add(primitive.pressure)?
            .checked_mul(velocity)?,
    })
}
