use crate::material::caloric::CaloricLaw;
use crate::{PhysicsError, Scalar};

use super::super::{Boundary, Conductance, TransportGrid};
use super::scaled_increment;

pub const WET_ENTHALPY_CONTRACT: &str =
    "hearthline-wet-enthalpy/1:solid-liquid-storage:implicit-upwind-sensible-flux";

/// Completed moisture-stage coefficients and fields. Moisture is liquid mass
/// per dry mass; boundary rates are positive into the material. The caller must
/// account for latent energy separately when that boundary represents vapor.
/// This is not a vapor-storage, sorption or pressure-work model.
#[derive(Clone, Copy)]
pub struct WaterTransport<'a> {
    pub liquid: CaloricLaw<'a>,
    pub previous_moisture: &'a [Scalar],
    pub current_moisture: &'a [Scalar],
    pub conductance: &'a [[Scalar; 3]],
    pub boundary: &'a [Boundary],
    pub inlet_temperature: Scalar,
}

impl<'a> WaterTransport<'a> {
    pub(super) fn validate(self, cells: usize, domain: [Scalar; 2]) -> Result<(), PhysicsError> {
        if [
            self.previous_moisture.len(),
            self.current_moisture.len(),
            self.boundary.len(),
        ]
        .iter()
        .any(|&len| len != cells)
        {
            return Err(PhysicsError::BufferLength);
        }
        if self.liquid.domain() != domain
            || self
                .previous_moisture
                .iter()
                .chain(self.current_moisture)
                .any(|&m| m < Scalar::ZERO)
            || self
                .boundary
                .iter()
                .any(|b| b.conductance < Scalar::ZERO || b.ambient < Scalar::ZERO)
        {
            return Err(PhysicsError::InvalidCoefficient);
        }
        self.liquid.enthalpy(self.inlet_temperature)?;
        Conductance::Cells(self.conductance).validate(cells)
    }

    pub(super) fn stored_energy(
        self,
        cell: usize,
        mass: Scalar,
        temperature: Scalar,
        previous: bool,
    ) -> Result<Scalar, PhysicsError> {
        let moisture = if previous {
            self.previous_moisture[cell]
        } else {
            self.current_moisture[cell]
        };
        water_mass(mass, moisture)?.checked_mul(self.liquid.enthalpy(temperature)?)
    }

    pub(super) fn boundary_exchange(
        self,
        cell: usize,
        dt: Scalar,
        temperature: Scalar,
    ) -> Result<Scalar, PhysicsError> {
        let transferred = self.boundary_mass(cell, dt)?;
        if transferred == Scalar::ZERO {
            return Ok(Scalar::ZERO);
        }
        let upstream = if transferred > Scalar::ZERO {
            self.inlet_temperature
        } else {
            temperature
        };
        transferred.checked_mul(self.liquid.enthalpy(upstream)?)
    }

    fn boundary_mass(self, cell: usize, dt: Scalar) -> Result<Scalar, PhysicsError> {
        scaled_increment(dt, self.boundary[cell].rate(self.current_moisture[cell])?)
    }

    pub(super) fn row(
        self,
        grid: TransportGrid<'_>,
        cell: usize,
        mass: Scalar,
        dt: Scalar,
        old_temperature: Scalar,
        trial: &[Scalar],
    ) -> Result<WaterRow<'a>, PhysicsError> {
        let mut row = WaterRow {
            liquid: self.liquid,
            old_energy: self.stored_energy(cell, mass, old_temperature, true)?,
            current_water_mass: water_mass(mass, self.current_moisture[cell])?,
            outgoing_mass: [Scalar::ZERO; 7],
            incoming_energy: Scalar::ZERO,
        };
        let transfer = self.boundary_mass(cell, dt)?;
        row.add_transfer(0, transfer, self.inlet_temperature)?;
        for (face, neighbor) in grid.neighbors(cell).into_iter().enumerate() {
            let Some((neighbor, axis)) = neighbor else {
                continue;
            };
            let rate = Conductance::Cells(self.conductance)
                .face(cell, neighbor, axis)?
                .checked_mul(
                    self.current_moisture[neighbor].checked_sub(self.current_moisture[cell])?,
                )?;
            row.add_transfer(face + 1, scaled_increment(dt, rate)?, trial[neighbor])?;
        }
        Ok(row)
    }
}

fn water_mass(dry_mass: Scalar, moisture: Scalar) -> Result<Scalar, PhysicsError> {
    let mass = dry_mass.checked_mul(moisture)?;
    if moisture > Scalar::ZERO && mass == Scalar::ZERO {
        return Err(PhysicsError::InsufficientResolution);
    }
    Ok(mass)
}

pub(super) struct WaterRow<'a> {
    liquid: CaloricLaw<'a>,
    old_energy: Scalar,
    current_water_mass: Scalar,
    outgoing_mass: [Scalar; 7],
    incoming_energy: Scalar,
}

impl WaterRow<'_> {
    fn add_transfer(
        &mut self,
        face: usize,
        mass: Scalar,
        upstream_temperature: Scalar,
    ) -> Result<(), PhysicsError> {
        if mass > Scalar::ZERO {
            self.incoming_energy = self
                .incoming_energy
                .checked_add(mass.checked_mul(self.liquid.enthalpy(upstream_temperature)?)?)?;
        } else {
            self.outgoing_mass[face] = mass.checked_neg()?;
        }
        Ok(())
    }

    pub(super) fn residual(&self, temperature: Scalar) -> Result<Scalar, PhysicsError> {
        let h = self.liquid.enthalpy(temperature)?;
        let stored = self
            .current_water_mass
            .checked_mul(h)?
            .checked_sub(self.old_energy)?;
        self.outgoing_mass
            .iter()
            .try_fold(stored, |sum, &mass| sum.checked_add(mass.checked_mul(h)?))?
            .checked_sub(self.incoming_energy)
    }

    pub(super) fn capacity(&self, temperature: Scalar) -> Result<Scalar, PhysicsError> {
        self.outgoing_mass
            .iter()
            .try_fold(self.current_water_mass, |sum, &mass| sum.checked_add(mass))?
            .checked_mul(self.liquid.heat_capacity(temperature)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::caloric::CaloricKnot;

    #[test]
    fn face_energy_is_antisymmetric_even_when_lumping_would_round_differently() {
        let mut knots = [CaloricKnot::default(); 2];
        let capacity = Scalar::ratio(3, 2).unwrap();
        let liquid = CaloricLaw::compile(
            &[
                [Scalar::ZERO, capacity],
                [Scalar::integer(2).unwrap(), capacity],
            ],
            Scalar::ZERO,
            &mut knots,
        )
        .unwrap();
        let empty = || WaterRow {
            liquid,
            old_energy: Scalar::ZERO,
            current_water_mass: Scalar::ZERO,
            outgoing_mass: [Scalar::ZERO; 7],
            incoming_energy: Scalar::ZERO,
        };
        let mut donor = empty();
        let mut receiver = empty();
        let transferred = Scalar::from_raw(1);
        for face in 1..=2 {
            donor
                .add_transfer(face, transferred.checked_neg().unwrap(), Scalar::ONE)
                .unwrap();
            receiver
                .add_transfer(face, transferred, Scalar::ONE)
                .unwrap();
        }
        // Each face transfers round_even(1.5) = 2 raw energy units.
        // Multiplying the summed mass instead would give 3, not 4.
        let outgoing = donor.residual(Scalar::ONE).unwrap();
        let incoming = receiver.residual(Scalar::ONE).unwrap();
        assert_eq!(outgoing, Scalar::from_raw(4));
        assert_eq!(incoming, outgoing.checked_neg().unwrap());
        assert_ne!(
            outgoing,
            transferred
                .checked_add(transferred)
                .unwrap()
                .checked_mul(liquid.enthalpy(Scalar::ONE).unwrap())
                .unwrap()
        );
    }
}
