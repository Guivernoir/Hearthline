use crate::{PhysicsError, Scalar};

/// Isothermal ideal-gas storage for the receiver verification case. Pressure
/// is absolute; temperature is kelvin; mass is kg; volume is m^3.
/// This proof deliberately does not model compression work or dryer behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IsothermalReceiver {
    storage_kg_pa: Scalar,
}

impl IsothermalReceiver {
    pub fn new(
        volume_m3: Scalar,
        temperature_k: Scalar,
        gas_constant_j_kg_k: Scalar,
    ) -> Result<Self, PhysicsError> {
        if volume_m3 <= Scalar::ZERO
            || temperature_k <= Scalar::ZERO
            || gas_constant_j_kg_k <= Scalar::ZERO
        {
            return Err(PhysicsError::InvalidCoefficient);
        }
        let storage_kg_pa =
            volume_m3.checked_div(gas_constant_j_kg_k.checked_mul(temperature_k)?)?;
        if storage_kg_pa <= Scalar::ZERO {
            return Err(PhysicsError::InvalidCoefficient);
        }
        Ok(Self { storage_kg_pa })
    }

    pub const fn storage_kg_pa(self) -> Scalar {
        self.storage_kg_pa
    }

    pub fn mass_kg(self, pressure_pa: Scalar) -> Result<Scalar, PhysicsError> {
        if pressure_pa <= Scalar::ZERO {
            return Err(PhysicsError::InvalidState);
        }
        self.storage_kg_pa.checked_mul(pressure_pa)
    }

    pub fn pressure_pa(self, mass_kg: Scalar) -> Result<Scalar, PhysicsError> {
        if mass_kg <= Scalar::ZERO {
            return Err(PhysicsError::InvalidState);
        }
        mass_kg.checked_div(self.storage_kg_pa)
    }
}
