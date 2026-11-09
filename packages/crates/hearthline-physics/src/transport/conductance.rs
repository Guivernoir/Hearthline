use crate::{PhysicsError, Scalar};

/// Two equal half-cell resistances in series. Inputs already include the common
/// area / center-distance factor. Both cells compute exactly the same value.
pub fn harmonic_conductance(left: Scalar, right: Scalar) -> Result<Scalar, PhysicsError> {
    if left < Scalar::ZERO || right < Scalar::ZERO {
        return Err(PhysicsError::InvalidCoefficient);
    }
    if left == Scalar::ZERO || right == Scalar::ZERO {
        return Ok(Scalar::ZERO);
    }
    let a = i128::from(left.raw());
    let b = i128::from(right.raw());
    // 2 * (i64::MAX)^2 is below i128::MAX; the domain checks exclude negatives.
    Scalar::from_raw_ratio(2 * a * b, a + b)
}

/// Film and center-to-surface conductances in series. Neither is a prescribed
/// surface value; this includes the half-cell resistance at a Robin boundary.
pub fn surface_conductance(film: Scalar, half_cell: Scalar) -> Result<Scalar, PhysicsError> {
    if film < Scalar::ZERO || half_cell < Scalar::ZERO {
        return Err(PhysicsError::InvalidCoefficient);
    }
    if film == Scalar::ZERO || half_cell == Scalar::ZERO {
        return Ok(Scalar::ZERO);
    }
    let a = i128::from(film.raw());
    let b = i128::from(half_cell.raw());
    let result = Scalar::from_raw_ratio(a * b, a + b)?;
    if result == Scalar::ZERO {
        return Err(PhysicsError::InsufficientResolution);
    }
    Ok(result)
}

#[derive(Clone, Copy)]
pub(super) enum Conductance<'a> {
    Uniform([Scalar; 3]),
    Cells(&'a [[Scalar; 3]]),
}

impl Conductance<'_> {
    pub(super) fn validate(self, cells: usize) -> Result<(), PhysicsError> {
        let values = match self {
            Self::Uniform(ref axes) => axes.as_slice(),
            Self::Cells(values) => {
                if values.len() != cells {
                    return Err(PhysicsError::BufferLength);
                }
                values.as_flattened()
            }
        };
        if values.iter().any(|value| *value < Scalar::ZERO) {
            return Err(PhysicsError::InvalidCoefficient);
        }
        Ok(())
    }

    pub(super) fn face(
        self,
        cell: usize,
        neighbor: usize,
        axis: usize,
    ) -> Result<Scalar, PhysicsError> {
        match self {
            Self::Uniform(axes) => Ok(axes[axis]),
            Self::Cells(values) => harmonic_conductance(values[cell][axis], values[neighbor][axis]),
        }
    }
}
