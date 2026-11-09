use crate::{PhysicsError, Scalar};

pub const CALORIC_CONTRACT: &str = "hearthline-caloric/1:linear-cp:integrated-enthalpy";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CaloricKnot {
    pub temperature: Scalar,
    pub heat_capacity: Scalar,
    pub enthalpy: Scalar,
}

/// Positive piecewise-linear specific heat capacity with a single, compiled
/// enthalpy primitive. Use consistent temperature, mass and energy references.
/// Pressure work, composition changes and phase transitions are not included.
#[derive(Clone, Copy, Debug)]
pub struct CaloricLaw<'a> {
    knots: &'a [CaloricKnot],
}

impl<'a> CaloricLaw<'a> {
    pub fn compile(
        capacities: &[[Scalar; 2]],
        reference_enthalpy: Scalar,
        output: &'a mut [CaloricKnot],
    ) -> Result<Self, PhysicsError> {
        if capacities.len() != output.len() {
            return Err(PhysicsError::BufferLength);
        }
        super::PropertyTable::new(capacities)?;
        if capacities.iter().any(|knot| knot[1] <= Scalar::ZERO) {
            return Err(PhysicsError::InvalidCoefficient);
        }
        let mut enthalpy = reference_enthalpy;
        for (index, &[temperature, heat_capacity]) in capacities.iter().enumerate() {
            if index > 0 {
                enthalpy = enthalpy.checked_add(area(
                    temperature.checked_sub(capacities[index - 1][0])?,
                    capacities[index - 1][1],
                    heat_capacity,
                )?)?;
            }
            output[index] = CaloricKnot {
                temperature,
                heat_capacity,
                enthalpy,
            };
        }
        Self::new(output)
    }

    pub fn new(knots: &'a [CaloricKnot]) -> Result<Self, PhysicsError> {
        if knots.len() < 2 || knots.iter().any(|knot| knot.heat_capacity <= Scalar::ZERO) {
            return Err(PhysicsError::InvalidCoefficient);
        }
        for pair in knots.windows(2) {
            let width = pair[1].temperature.checked_sub(pair[0].temperature)?;
            if width <= Scalar::ZERO {
                return Err(PhysicsError::InvalidCoefficient);
            }
            let increment = area(width, pair[0].heat_capacity, pair[1].heat_capacity)?;
            if increment <= Scalar::ZERO {
                return Err(PhysicsError::InsufficientResolution);
            }
            if pair[0].enthalpy.checked_add(increment)? != pair[1].enthalpy {
                return Err(PhysicsError::InvalidCoefficient);
            }
        }
        Ok(Self { knots })
    }

    pub fn domain(self) -> [Scalar; 2] {
        [
            self.knots[0].temperature,
            self.knots[self.knots.len() - 1].temperature,
        ]
    }

    pub fn heat_capacity(self, temperature: Scalar) -> Result<Scalar, PhysicsError> {
        let upper = self.upper(temperature)?;
        let right = self.knots[upper];
        if temperature == right.temperature {
            return Ok(right.heat_capacity);
        }
        let left = self.knots[upper - 1];
        left.heat_capacity.checked_add(
            right
                .heat_capacity
                .checked_sub(left.heat_capacity)?
                .checked_mul_div(
                    temperature.checked_sub(left.temperature)?,
                    right.temperature.checked_sub(left.temperature)?,
                )?,
        )
    }

    pub fn enthalpy(self, temperature: Scalar) -> Result<Scalar, PhysicsError> {
        let upper = self.upper(temperature)?;
        let right = self.knots[upper];
        if temperature == right.temperature {
            return Ok(right.enthalpy);
        }
        let left = self.knots[upper - 1];
        if left.heat_capacity == right.heat_capacity {
            return left.enthalpy.checked_add(
                left.heat_capacity
                    .checked_mul(temperature.checked_sub(left.temperature)?)?,
            );
        }
        // Integrate the unrounded linear capacity, not rounded cp(T) * dT.
        // All products are checked; oversized scales are diagnostics, not loss
        // of the quadratic term. The primitive is monotone because cp > 0.
        let width = i128::from(right.temperature.checked_sub(left.temperature)?.raw());
        let offset = i128::from(temperature.checked_sub(left.temperature)?.raw());
        let slope = i128::from(right.heat_capacity.raw()) - i128::from(left.heat_capacity.raw());
        let linear = width
            .checked_mul(2)
            .and_then(|v| v.checked_mul(i128::from(left.heat_capacity.raw())))
            .ok_or(PhysicsError::Overflow)?;
        let numerator = slope
            .checked_mul(offset)
            .and_then(|v| v.checked_add(linear))
            .and_then(|v| v.checked_mul(offset))
            .ok_or(PhysicsError::Overflow)?;
        let denominator = width
            .checked_mul(2 * i128::from(Scalar::SCALE))
            .ok_or(PhysicsError::Overflow)?;
        left.enthalpy
            .checked_add(Scalar::from_raw_ratio(numerator, denominator)?)
    }

    fn upper(self, temperature: Scalar) -> Result<usize, PhysicsError> {
        let [minimum, maximum] = self.domain();
        if temperature < minimum || temperature > maximum {
            return Err(PhysicsError::InvalidState);
        }
        Ok(self
            .knots
            .partition_point(|knot| knot.temperature < temperature))
    }
}

fn area(width: Scalar, left: Scalar, right: Scalar) -> Result<Scalar, PhysicsError> {
    let numerator = i128::from(width.raw())
        .checked_mul(i128::from(left.raw()) + i128::from(right.raw()))
        .ok_or(PhysicsError::Overflow)?;
    Scalar::from_raw_ratio(numerator, 2 * i128::from(Scalar::SCALE))
}
