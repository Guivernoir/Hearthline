use super::{CompressibleFlow, Conserved, FlowBoundary, Primitive};
use crate::{PhysicsError, Scalar};

/// Constant-property Newtonian/Stokes stress and Fourier heat conduction.
/// Coefficients and R must use the same nondimensional reference system as
/// density, velocity, energy, mesh and time. No bulk-viscosity law is assumed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViscousTransport {
    viscosity: Scalar,
    conductivity: Scalar,
    gas_constant: Scalar,
}

impl ViscousTransport {
    pub fn new(
        viscosity: Scalar,
        conductivity: Scalar,
        gas_constant: Scalar,
    ) -> Result<Self, PhysicsError> {
        if viscosity < Scalar::ZERO || conductivity < Scalar::ZERO || gas_constant <= Scalar::ZERO {
            return Err(PhysicsError::InvalidCoefficient);
        }
        Ok(Self {
            viscosity,
            conductivity,
            gas_constant,
        })
    }

    pub fn temperature(self, state: Primitive) -> Result<Scalar, PhysicsError> {
        if state.density <= Scalar::ZERO || state.pressure <= Scalar::ZERO {
            return Err(PhysicsError::InvalidState);
        }
        let temperature = state
            .pressure
            .checked_div(state.density.checked_mul(self.gas_constant)?)?;
        if temperature <= Scalar::ZERO {
            return Err(PhysicsError::InvalidState);
        }
        Ok(temperature)
    }

    /// grad[i][j] = d velocity_i / d coordinate_j; flux points in +axis.
    pub fn flux(
        self,
        gradient: [[Scalar; 3]; 3],
        velocity: [Scalar; 3],
        temperature_gradient: Scalar,
        axis: usize,
    ) -> Result<Conserved, PhysicsError> {
        if axis >= 3 {
            return Err(PhysicsError::InvalidGeometry);
        }
        let trace = gradient[0][0]
            .checked_add(gradient[1][1])?
            .checked_add(gradient[2][2])?;
        let volumetric = trace.checked_mul(Scalar::ratio(2, 3)?)?;
        let mut flux = Conserved::default();
        for i in 0..3 {
            let mut rate = gradient[i][axis].checked_add(gradient[axis][i])?;
            if i == axis {
                rate = rate.checked_sub(volumetric)?;
            }
            flux.momentum[i] = self.viscosity.checked_mul(rate)?;
            flux.total_energy = flux
                .total_energy
                .checked_add(flux.momentum[i].checked_mul(velocity[i])?)?;
        }
        flux.total_energy = flux
            .total_energy
            .checked_add(self.conductivity.checked_mul(temperature_gradient)?)?;
        Ok(flux)
    }
}

impl CompressibleFlow {
    pub(super) fn viscous_face(
        &self,
        fields: &[Conserved],
        left: usize,
        right: usize,
        axis: usize,
        transport: ViscousTransport,
    ) -> Result<Conserved, PhysicsError> {
        let a = self.gas.primitive(fields[left])?;
        let b = self.gas.primitive(fields[right])?;
        let ga = self.velocity_gradient(fields, left)?;
        let gb = self.velocity_gradient(fields, right)?;
        let two = Scalar::integer(2)?;
        let dx = self.grid.spacing()[axis];
        let mut gradient = [[Scalar::ZERO; 3]; 3];
        let mut velocity = [Scalar::ZERO; 3];
        for component in 0..3 {
            velocity[component] = a.velocity[component]
                .checked_add(b.velocity[component])?
                .checked_div(two)?;
            for direction in 0..3 {
                gradient[component][direction] = if direction == axis {
                    b.velocity[component]
                        .checked_sub(a.velocity[component])?
                        .checked_div(dx)?
                } else {
                    ga[component][direction]
                        .checked_add(gb[component][direction])?
                        .checked_div(two)?
                };
            }
        }
        let temperature_gradient = transport
            .temperature(b)?
            .checked_sub(transport.temperature(a)?)?
            .checked_div(dx)?;
        transport.flux(gradient, velocity, temperature_gradient, axis)
    }

    pub(super) fn viscous_boundary(
        &self,
        fields: &[Conserved],
        cell: usize,
        face: usize,
        transport: ViscousTransport,
    ) -> Result<Conserved, PhysicsError> {
        let axis = face / 2;
        let sign = Scalar::integer(if face.is_multiple_of(2) { -1 } else { 1 })?;
        let center = self.gas.primitive(fields[cell])?;
        let temperature = transport.temperature(center)?;
        let mut distance = self.grid.spacing()[axis].checked_div(Scalar::integer(2)?)?;
        let (velocity, wall_temperature) = match self.boundaries[face] {
            FlowBoundary::NoSlipWall {
                velocity,
                temperature,
            } => (velocity, temperature),
            FlowBoundary::AdiabaticSlipWall => {
                let mut velocity = center.velocity;
                velocity[axis] = Scalar::ZERO;
                (velocity, None)
            }
            FlowBoundary::Reservoir(state) => {
                distance = self.grid.spacing()[axis];
                (state.velocity, Some(transport.temperature(state)?))
            }
            FlowBoundary::Periodic => return Err(PhysicsError::InvalidCoefficient),
        };
        let mut gradient = match self.boundaries[face] {
            FlowBoundary::NoSlipWall { .. } => [[Scalar::ZERO; 3]; 3],
            _ => self.velocity_gradient(fields, cell)?,
        };
        for component in 0..3 {
            gradient[component][axis] = velocity[component]
                .checked_sub(center.velocity[component])?
                .checked_mul(sign)?
                .checked_div(distance)?;
        }
        let temperature_gradient = wall_temperature.map_or(Ok(Scalar::ZERO), |wall| {
            wall.checked_sub(temperature)?
                .checked_mul(sign)?
                .checked_div(distance)
        })?;
        let mut flux = transport.flux(gradient, velocity, temperature_gradient, axis)?;
        if self.boundaries[face] == FlowBoundary::AdiabaticSlipWall {
            for (component, momentum) in flux.momentum.iter_mut().enumerate() {
                if component != axis {
                    *momentum = Scalar::ZERO;
                }
            }
            flux.total_energy = Scalar::ZERO;
        }
        Ok(flux)
    }

    fn velocity_gradient(
        &self,
        fields: &[Conserved],
        cell: usize,
    ) -> Result<[[Scalar; 3]; 3], PhysicsError> {
        let shape = self.grid.shape();
        let strides = [1, shape[0], shape[0] * shape[1]];
        let center = self.gas.primitive(fields[cell])?;
        let mut gradient = [[Scalar::ZERO; 3]; 3];
        for axis in 0..3 {
            let coordinate = (cell / strides[axis]) % shape[axis];
            let mut velocities = [[Scalar::ZERO; 3]; 2];
            for (side, velocity) in velocities.iter_mut().enumerate() {
                *velocity = if side == 0 && coordinate > 0 {
                    self.gas.primitive(fields[cell - strides[axis]])?.velocity
                } else if side == 1 && coordinate + 1 < shape[axis] {
                    self.gas.primitive(fields[cell + strides[axis]])?.velocity
                } else {
                    match self.boundaries[2 * axis + side] {
                        FlowBoundary::Periodic => {
                            let neighbor = if side == 0 {
                                cell + (shape[axis] - 1) * strides[axis]
                            } else {
                                cell - (shape[axis] - 1) * strides[axis]
                            };
                            self.gas.primitive(fields[neighbor])?.velocity
                        }
                        FlowBoundary::Reservoir(state) => state.velocity,
                        FlowBoundary::AdiabaticSlipWall => {
                            let mut velocity = center.velocity;
                            velocity[axis] = velocity[axis].checked_neg()?;
                            velocity
                        }
                        FlowBoundary::NoSlipWall { velocity, .. } => {
                            let mut ghost = [Scalar::ZERO; 3];
                            for i in 0..3 {
                                ghost[i] = velocity[i]
                                    .checked_mul(Scalar::integer(2)?)?
                                    .checked_sub(center.velocity[i])?;
                            }
                            ghost
                        }
                    }
                };
            }
            let distance = self.grid.spacing()[axis].checked_mul(Scalar::integer(2)?)?;
            for component in 0..3 {
                gradient[component][axis] = velocities[1][component]
                    .checked_sub(velocities[0][component])?
                    .checked_div(distance)?;
            }
        }
        Ok(gradient)
    }
}
