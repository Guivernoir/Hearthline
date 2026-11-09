use crate::{PhysicsError, Scalar};

/// Reference scales derived from L, U, rho and the ideal-gas constant.
/// The normalized gas law is p = rho T, energy scale is rho U^2, and time
/// scale is L/U. Every conversion is checked; no SI coefficient is clamped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GasScales {
    length: Scalar,
    velocity: Scalar,
    density: Scalar,
    pressure: Scalar,
    temperature: Scalar,
    time: Scalar,
    viscosity: Scalar,
    conductivity: Scalar,
}

impl GasScales {
    pub fn new(
        length: Scalar,
        velocity: Scalar,
        density: Scalar,
        gas_constant: Scalar,
    ) -> Result<Self, PhysicsError> {
        if [length, velocity, density, gas_constant]
            .iter()
            .any(|v| *v <= Scalar::ZERO)
        {
            return Err(PhysicsError::InvalidCoefficient);
        }
        let squared_velocity = velocity.checked_mul(velocity)?;
        let pressure = density.checked_mul(squared_velocity)?;
        let temperature = squared_velocity.checked_div(gas_constant)?;
        let time = length.checked_div(velocity)?;
        let viscosity = density.checked_mul(velocity)?.checked_mul(length)?;
        let conductivity = viscosity.checked_mul(gas_constant)?;
        if [pressure, temperature, time, viscosity, conductivity]
            .iter()
            .any(|v| *v <= Scalar::ZERO)
        {
            return Err(PhysicsError::InvalidCoefficient);
        }
        Ok(Self {
            length,
            velocity,
            density,
            pressure,
            temperature,
            time,
            viscosity,
            conductivity,
        })
    }

    pub fn length_to_solver(self, value_m: Scalar) -> Result<Scalar, PhysicsError> {
        value_m.checked_div(self.length)
    }
    pub fn density_to_solver(self, value_kg_m3: Scalar) -> Result<Scalar, PhysicsError> {
        value_kg_m3.checked_div(self.density)
    }
    pub fn pressure_to_solver(self, value_pa: Scalar) -> Result<Scalar, PhysicsError> {
        value_pa.checked_div(self.pressure)
    }
    pub fn temperature_to_solver(self, value_k: Scalar) -> Result<Scalar, PhysicsError> {
        value_k.checked_div(self.temperature)
    }
    pub fn time_to_solver(self, value_s: Scalar) -> Result<Scalar, PhysicsError> {
        value_s.checked_div(self.time)
    }
    pub fn viscosity_to_solver(self, value_pa_s: Scalar) -> Result<Scalar, PhysicsError> {
        value_pa_s.checked_div(self.viscosity)
    }
    pub fn conductivity_to_solver(self, value_w_m_k: Scalar) -> Result<Scalar, PhysicsError> {
        value_w_m_k.checked_div(self.conductivity)
    }
    pub fn pressure_to_si(self, value: Scalar) -> Result<Scalar, PhysicsError> {
        value.checked_mul(self.pressure)
    }
    pub fn temperature_to_si(self, value: Scalar) -> Result<Scalar, PhysicsError> {
        value.checked_mul(self.temperature)
    }
    pub fn velocity_to_si(self, value: Scalar) -> Result<Scalar, PhysicsError> {
        value.checked_mul(self.velocity)
    }
    pub fn density_to_si(self, value: Scalar) -> Result<Scalar, PhysicsError> {
        value.checked_mul(self.density)
    }
    pub const fn length(self) -> Scalar {
        self.length
    }
    pub const fn pressure(self) -> Scalar {
        self.pressure
    }
    pub const fn temperature(self) -> Scalar {
        self.temperature
    }
    pub const fn time(self) -> Scalar {
        self.time
    }
}
