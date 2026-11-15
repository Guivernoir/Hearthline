use super::{CellInventory, ConnectedFlow, Conserved, FaceLink, FlowBoundary, ViscousTransport};
use crate::{PhysicsError, Scalar};

#[cfg(test)]
#[path = "viscous_tests.rs"]
mod tests;

fn interpolate(a: Scalar, b: Scalar, left: Scalar, right: Scalar) -> Result<Scalar, PhysicsError> {
    let numerator = (i128::from(a.raw()) * i128::from(right.raw()))
        .checked_add(i128::from(b.raw()) * i128::from(left.raw()))
        .ok_or(PhysicsError::Overflow)?;
    Scalar::from_raw_ratio(numerator, i128::from(left.raw()) + i128::from(right.raw()))
}

impl ConnectedFlow<'_> {
    pub(super) fn viscous_flux(
        &self,
        state: &[CellInventory],
        cell: usize,
        face: usize,
        transport: ViscousTransport,
    ) -> Result<Conserved, PhysicsError> {
        let axis = face / 2;
        let center = self.primitive(state, cell)?;
        let width = self.mesh.cells[cell].spacing()[axis];
        let two = Scalar::integer(2)?;
        match self.mesh.cells[cell].faces[face] {
            FaceLink::Cell(peer) => {
                let peer = peer as usize;
                let other = self.primitive(state, peer)?;
                let other_width = self.mesh.cells[peer].spacing()[axis];
                let distance = width.checked_add(other_width)?.checked_div(two)?;
                let left = self.gradient(state, cell)?;
                let right = self.gradient(state, peer)?;
                let mut gradient = [[Scalar::ZERO; 3]; 3];
                let mut velocity = [Scalar::ZERO; 3];
                for i in 0..3 {
                    velocity[i] =
                        interpolate(center.velocity[i], other.velocity[i], width, other_width)?;
                    for j in 0..3 {
                        gradient[i][j] = if j == axis {
                            other.velocity[i]
                                .checked_sub(center.velocity[i])?
                                .checked_div(distance)?
                        } else {
                            interpolate(left[i][j], right[i][j], width, other_width)?
                        };
                    }
                }
                let temperature_gradient = transport
                    .temperature(other)?
                    .checked_sub(transport.temperature(center)?)?
                    .checked_div(distance)?;
                transport.flux(gradient, velocity, temperature_gradient, axis)
            }
            FaceLink::Boundary(id) => {
                let boundary = self.mesh.boundaries[id as usize];
                let mut distance = width.checked_div(two)?;
                let (velocity, temperature) = match boundary {
                    FlowBoundary::NoSlipWall {
                        velocity,
                        temperature,
                    } => (velocity, temperature),
                    FlowBoundary::AdiabaticSlipWall => {
                        let mut velocity = center.velocity;
                        velocity[axis] = Scalar::ZERO;
                        (velocity, None)
                    }
                    FlowBoundary::Reservoir(primitive) => {
                        distance = width;
                        (primitive.velocity, Some(transport.temperature(primitive)?))
                    }
                    FlowBoundary::Periodic => return Err(PhysicsError::InvalidGeometry),
                };
                let mut gradient = if matches!(boundary, FlowBoundary::NoSlipWall { .. }) {
                    [[Scalar::ZERO; 3]; 3]
                } else {
                    self.gradient(state, cell)?
                };
                let sign = Scalar::integer(if face.is_multiple_of(2) { -1 } else { 1 })?;
                for i in 0..3 {
                    gradient[i][axis] = velocity[i]
                        .checked_sub(center.velocity[i])?
                        .checked_mul(sign)?
                        .checked_div(distance)?;
                }
                let temperature_gradient = temperature.map_or(Ok(Scalar::ZERO), |wall| {
                    wall.checked_sub(transport.temperature(center)?)?
                        .checked_mul(sign)?
                        .checked_div(distance)
                })?;
                let mut flux = transport.flux(gradient, velocity, temperature_gradient, axis)?;
                if boundary == FlowBoundary::AdiabaticSlipWall {
                    for i in 0..3 {
                        if i != axis {
                            flux.momentum[i] = Scalar::ZERO;
                        }
                    }
                    flux.total_energy = Scalar::ZERO;
                }
                Ok(flux)
            }
        }
    }

    fn gradient(
        &self,
        state: &[CellInventory],
        cell: usize,
    ) -> Result<[[Scalar; 3]; 3], PhysicsError> {
        let center = self.primitive(state, cell)?;
        let mut gradient = [[Scalar::ZERO; 3]; 3];
        let two = Scalar::integer(2)?;
        for axis in 0..3 {
            let width = self.mesh.cells[cell].spacing()[axis];
            let mut velocities = [[Scalar::ZERO; 3]; 2];
            let mut distances = [width; 2];
            for side in 0..2 {
                velocities[side] = match self.mesh.cells[cell].faces[2 * axis + side] {
                    FaceLink::Cell(peer) => {
                        distances[side] = width
                            .checked_add(self.mesh.cells[peer as usize].spacing()[axis])?
                            .checked_div(two)?;
                        self.primitive(state, peer as usize)?.velocity
                    }
                    FaceLink::Boundary(id) => match self.mesh.boundaries[id as usize] {
                        FlowBoundary::Reservoir(state) => state.velocity,
                        FlowBoundary::AdiabaticSlipWall => {
                            let mut ghost = center.velocity;
                            ghost[axis] = ghost[axis].checked_neg()?;
                            ghost
                        }
                        FlowBoundary::NoSlipWall { velocity, .. } => {
                            let mut ghost = [Scalar::ZERO; 3];
                            for i in 0..3 {
                                ghost[i] = velocity[i]
                                    .checked_mul(two)?
                                    .checked_sub(center.velocity[i])?;
                            }
                            ghost
                        }
                        FlowBoundary::Periodic => return Err(PhysicsError::InvalidGeometry),
                    },
                };
            }
            for i in 0..3 {
                let backward = center.velocity[i]
                    .checked_sub(velocities[0][i])?
                    .checked_div(distances[0])?;
                let forward = velocities[1][i]
                    .checked_sub(center.velocity[i])?
                    .checked_div(distances[1])?;
                gradient[i][axis] = interpolate(backward, forward, distances[0], distances[1])?;
            }
        }
        Ok(gradient)
    }
}
