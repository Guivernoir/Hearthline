//! Joint implicit integration of conforming, face-connected gas domains.
//! Extensive state makes each rounded internal transfer equal and opposite.

mod mesh;
mod viscous;
pub use mesh::{FaceLink, FlowCell, FlowMesh};

use super::{Conserved, FlowBoundary, IdealGas, Primitive, ViscousTransport};
use crate::transport::SolverLimits;
use crate::{PhysicsError, Scalar};

pub const CONNECTED_FLOW_CONTRACT: &str = "connected-gas/1:extensive-q9:implicit-rusanov";

/// Nondimensional integrals over one cell, not density or SI sensor readings.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CellInventory {
    pub mass: Scalar,
    pub momentum: [Scalar; 3],
    pub energy: Scalar,
}

impl CellInventory {
    pub fn from_density(density: Conserved, volume: Scalar) -> Result<Self, PhysicsError> {
        if volume <= Scalar::ZERO {
            return Err(PhysicsError::InvalidGeometry);
        }
        let mut values = density.components();
        for value in &mut values {
            *value = value.checked_mul(volume)?;
        }
        Ok(Self::from_components(values))
    }
    pub fn density(self, volume: Scalar) -> Result<Conserved, PhysicsError> {
        if volume <= Scalar::ZERO {
            return Err(PhysicsError::InvalidGeometry);
        }
        let mut values = self.components();
        for value in &mut values {
            *value = value.checked_div(volume)?;
        }
        Ok(Conserved::from_components(values))
    }
    pub const fn components(self) -> [Scalar; 5] {
        [
            self.mass,
            self.momentum[0],
            self.momentum[1],
            self.momentum[2],
            self.energy,
        ]
    }
    pub const fn from_components(v: [Scalar; 5]) -> Self {
        Self {
            mass: v[0],
            momentum: [v[1], v[2], v[3]],
            energy: v[4],
        }
    }
    fn add(self, rhs: Self, subtract: bool) -> Result<Self, PhysicsError> {
        let mut values = self.components();
        for (value, rhs) in values.iter_mut().zip(rhs.components()) {
            *value = if subtract {
                value.checked_sub(rhs)?
            } else {
                value.checked_add(rhs)?
            };
        }
        Ok(Self::from_components(values))
    }
}

pub struct ConnectedWorkspace<'a> {
    pub trial: &'a mut [CellInventory],
    pub next: &'a mut [CellInventory],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectedReport {
    pub iterations: u32,
    pub evaluations: u64,
    /// Maximum density-form backward-Euler equation defect at accepted fields.
    pub residual: Scalar,
    pub boundary_exchange: CellInventory,
    pub balance_error: CellInventory,
}

#[derive(Clone, Copy, Debug)]
pub struct ConnectedFlow<'a> {
    mesh: FlowMesh<'a>,
    gas: IdealGas,
    limits: SolverLimits,
    transport: Option<ViscousTransport>,
}

impl<'a> ConnectedFlow<'a> {
    pub fn new(
        mesh: FlowMesh<'a>,
        gas: IdealGas,
        limits: SolverLimits,
        transport: Option<ViscousTransport>,
    ) -> Result<Self, PhysicsError> {
        if limits.iterations == 0 || limits.residual <= Scalar::ZERO {
            return Err(PhysicsError::InvalidCoefficient);
        }
        for cell in mesh.cells {
            for (face, link) in cell.faces.iter().enumerate() {
                if let FaceLink::Boundary(id) = *link {
                    match mesh.boundaries[id as usize] {
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
                        FlowBoundary::AdiabaticSlipWall => (),
                        FlowBoundary::Periodic => return Err(PhysicsError::InvalidGeometry),
                    }
                }
            }
        }
        Ok(Self {
            mesh,
            gas,
            limits,
            transport,
        })
    }

    pub fn inventory(&self, state: &[CellInventory]) -> Result<CellInventory, PhysicsError> {
        if state.len() != self.mesh.cells.len() {
            return Err(PhysicsError::BufferLength);
        }
        let mut totals = [0_i128; 5];
        for cell in state {
            for (total, value) in totals.iter_mut().zip(cell.components()) {
                *total = total
                    .checked_add(i128::from(value.raw()))
                    .ok_or(PhysicsError::Overflow)?;
            }
        }
        let mut values = [Scalar::ZERO; 5];
        for (value, total) in values.iter_mut().zip(totals) {
            *value = Scalar::from_raw(i64::try_from(total).map_err(|_| PhysicsError::Overflow)?);
        }
        Ok(CellInventory::from_components(values))
    }

    pub fn solve(
        &self,
        dt: Scalar,
        state: &mut [CellInventory],
        work: &mut ConnectedWorkspace<'_>,
    ) -> Result<ConnectedReport, PhysicsError> {
        if state.len() != self.mesh.cells.len()
            || work.trial.len() != state.len()
            || work.next.len() != state.len()
        {
            return Err(PhysicsError::BufferLength);
        }
        if dt <= Scalar::ZERO {
            return Err(PhysicsError::InvalidCoefficient);
        }
        self.validate_state(state)?;
        let before = self.inventory(state)?;
        work.trial.copy_from_slice(state);
        let mut residual = Scalar::ZERO;
        let mut evaluations = 0_u64;
        for iteration in 1..=self.limits.iterations {
            let exchange = self.evaluate(dt, state, work.trial, work.next)?;
            evaluations += 1;
            self.validate_state(work.next)?;
            residual = self.residual(work.next, work.trial)?;
            if residual <= self.limits.residual {
                // Test the equation at the conservative candidate, not merely
                // the change between iterates. Never publish a relaxed state.
                self.evaluate(dt, state, work.next, work.trial)?;
                evaluations += 1;
                residual = self.residual(work.next, work.trial)?;
                if residual <= self.limits.residual {
                    let balance_error = self
                        .inventory(work.next)?
                        .add(before, true)?
                        .add(exchange, true)?;
                    if balance_error != CellInventory::default() {
                        return Err(PhysicsError::InvalidState);
                    }
                    state.copy_from_slice(work.next);
                    return Ok(ConnectedReport {
                        iterations: iteration,
                        evaluations,
                        residual,
                        boundary_exchange: exchange,
                        balance_error,
                    });
                }
            }
            for (trial, next) in work.trial.iter_mut().zip(work.next.iter()) {
                let mut values = trial.components();
                for (value, next) in values.iter_mut().zip(next.components()) {
                    *value = Scalar::from_raw_ratio(
                        i128::from(value.raw()) + i128::from(next.raw()),
                        2,
                    )?;
                }
                *trial = CellInventory::from_components(values);
            }
            self.validate_state(work.trial)?;
        }
        Err(PhysicsError::NonConvergence {
            iterations: self.limits.iterations,
            residual,
        })
    }

    fn density(&self, state: &[CellInventory], cell: usize) -> Result<Conserved, PhysicsError> {
        state[cell].density(self.mesh.cells[cell].volume())
    }
    fn primitive(&self, state: &[CellInventory], cell: usize) -> Result<Primitive, PhysicsError> {
        self.gas.primitive(self.density(state, cell)?)
    }
    fn validate_state(&self, state: &[CellInventory]) -> Result<(), PhysicsError> {
        for cell in 0..state.len() {
            self.primitive(state, cell)?;
        }
        Ok(())
    }
    fn residual(&self, a: &[CellInventory], b: &[CellInventory]) -> Result<Scalar, PhysicsError> {
        let mut residual = Scalar::ZERO;
        for (cell, (a, b)) in self.mesh.cells.iter().zip(a.iter().zip(b)) {
            for (a, b) in a.components().into_iter().zip(b.components()) {
                residual = residual.max(a.checked_sub(b)?.abs()?.checked_div(cell.volume())?);
            }
        }
        Ok(residual)
    }

    fn evaluate(
        &self,
        dt: Scalar,
        old: &[CellInventory],
        trial: &[CellInventory],
        next: &mut [CellInventory],
    ) -> Result<CellInventory, PhysicsError> {
        next.copy_from_slice(old);
        let mut exchange = CellInventory::default();
        for (index, cell) in self.mesh.cells.iter().enumerate() {
            for (face, link) in cell.faces.iter().enumerate() {
                let positive = face % 2 == 1;
                if matches!(link, FaceLink::Cell(_)) && !positive {
                    continue;
                }
                let axis = face / 2;
                let center = self.density(trial, index)?;
                let peer = match *link {
                    FaceLink::Cell(peer) => self.density(trial, peer as usize)?,
                    FaceLink::Boundary(id) => match self.mesh.boundaries[id as usize] {
                        FlowBoundary::Reservoir(state) => self.gas.conserved(state)?,
                        FlowBoundary::AdiabaticSlipWall | FlowBoundary::NoSlipWall { .. } => {
                            let mut ghost = center;
                            ghost.momentum[axis] = ghost.momentum[axis].checked_neg()?;
                            ghost
                        }
                        FlowBoundary::Periodic => return Err(PhysicsError::InvalidGeometry),
                    },
                };
                let mut flux = if positive {
                    self.gas.flux(center, peer, axis)?
                } else {
                    self.gas.flux(peer, center, axis)?
                };
                if let Some(transport) = self.transport {
                    flux = flux.checked_add_scaled(
                        self.viscous_flux(trial, index, face, transport)?,
                        Scalar::integer(-1)?,
                    )?;
                }
                let factor = dt.checked_mul(cell.area(axis)?)?;
                if factor == Scalar::ZERO {
                    return Err(PhysicsError::InsufficientResolution);
                }
                let transfer = CellInventory::from_density(flux, factor)?;
                next[index] = next[index].add(transfer, positive)?;
                match *link {
                    FaceLink::Cell(peer) => {
                        next[peer as usize] = next[peer as usize].add(transfer, false)?;
                    }
                    FaceLink::Boundary(_) => {
                        exchange = exchange.add(transfer, positive)?;
                    }
                }
            }
        }
        Ok(exchange)
    }
}
