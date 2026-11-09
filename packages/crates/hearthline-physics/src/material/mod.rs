use crate::{PhysicsError, Scalar};

pub mod caloric;

/// Immutable tabulated nonnegative property, with an explicit validity domain.
/// The caller owns the knots; this view never allocates or extrapolates.
#[derive(Clone, Copy, Debug)]
pub struct PropertyTable<'a> {
    knots: &'a [[Scalar; 2]],
}

impl<'a> PropertyTable<'a> {
    pub fn new(knots: &'a [[Scalar; 2]]) -> Result<Self, PhysicsError> {
        if knots.len() < 2 || knots.iter().any(|knot| knot[1] < Scalar::ZERO) {
            return Err(PhysicsError::InvalidCoefficient);
        }
        for pair in knots.windows(2) {
            if pair[1][0].checked_sub(pair[0][0])? <= Scalar::ZERO {
                return Err(PhysicsError::InvalidCoefficient);
            }
        }
        Ok(Self { knots })
    }

    pub fn value(self, state: Scalar) -> Result<Scalar, PhysicsError> {
        if state < self.knots[0][0] || state > self.knots[self.knots.len() - 1][0] {
            return Err(PhysicsError::InvalidState);
        }
        let upper = self.knots.partition_point(|knot| knot[0] < state);
        if self.knots[upper][0] == state {
            return Ok(self.knots[upper][1]);
        }
        let [x0, y0] = self.knots[upper - 1];
        let [x1, y1] = self.knots[upper];
        y0.checked_add(
            y1.checked_sub(y0)?
                .checked_mul_div(state.checked_sub(x0)?, x1.checked_sub(x0)?)?,
        )
    }

    pub fn domain(self) -> [Scalar; 2] {
        [self.knots[0][0], self.knots[self.knots.len() - 1][0]]
    }
}
