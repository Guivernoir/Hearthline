use crate::{PhysicsError, Scalar};

/// Immutable symmetric CSR operator. Columns in each row are strictly ordered.
/// The host owns and budgets all arrays before constructing this checked view.
#[derive(Clone, Copy, Debug)]
pub struct StiffnessMatrix<'a> {
    rows: &'a [u32],
    columns: &'a [u32],
    values: &'a [Scalar],
}

impl<'a> StiffnessMatrix<'a> {
    pub fn new(
        rows: &'a [u32],
        columns: &'a [u32],
        values: &'a [Scalar],
    ) -> Result<Self, PhysicsError> {
        if rows.len() < 2
            || rows.len() - 1 > u32::MAX as usize
            || rows[0] != 0
            || rows.last().copied().map(|n| n as usize) != Some(columns.len())
            || columns.len() != values.len()
            || rows.windows(2).any(|p| p[0] >= p[1])
        {
            return Err(PhysicsError::InvalidGeometry);
        }
        let matrix = Self {
            rows,
            columns,
            values,
        };
        for row in 0..matrix.dimension() {
            let range = matrix.range(row);
            let columns = &columns[range.clone()];
            if columns.windows(2).any(|p| p[0] >= p[1])
                || columns
                    .iter()
                    .any(|column| *column as usize >= matrix.dimension())
            {
                return Err(PhysicsError::InvalidGeometry);
            }
            if matrix.entry(row, row).is_none_or(|v| v <= Scalar::ZERO) {
                return Err(PhysicsError::NonPositiveDefinite);
            }
            for index in range {
                if matrix.entry(matrix.columns[index] as usize, row) != Some(matrix.values[index]) {
                    return Err(PhysicsError::InvalidGeometry);
                }
            }
        }
        Ok(matrix)
    }

    pub fn dimension(self) -> usize {
        self.rows.len() - 1
    }

    pub fn apply(self, input: &[Scalar], output: &mut [Scalar]) -> Result<(), PhysicsError> {
        if input.len() != self.dimension() || output.len() != input.len() {
            return Err(PhysicsError::BufferLength);
        }
        for (row, output) in output.iter_mut().enumerate() {
            let mut sum = 0_i128;
            for index in self.range(row) {
                sum = sum
                    .checked_add(
                        i128::from(self.values[index].raw())
                            * i128::from(input[self.columns[index] as usize].raw()),
                    )
                    .ok_or(PhysicsError::Overflow)?;
            }
            *output = Scalar::from_product_sum(sum)?;
        }
        Ok(())
    }

    pub(super) fn diagonal(self, output: &mut [Scalar]) -> Result<(), PhysicsError> {
        if output.len() != self.dimension() {
            return Err(PhysicsError::BufferLength);
        }
        for (row, value) in output.iter_mut().enumerate() {
            *value = self.entry(row, row).ok_or(PhysicsError::InvalidGeometry)?;
        }
        Ok(())
    }

    pub(super) fn symmetric_sweep(
        self,
        residual: &[Scalar],
        diagonal: &[Scalar],
        constraints: &[Option<Scalar>],
        scale: Scalar,
        output: &mut [Scalar],
    ) -> Result<(), PhysicsError> {
        // (D+L)y = s*r, then (D+U)z = D*y. Output stores y then z;
        // constrained correction DOFs are zero, regardless of prescribed u.
        for row in 0..self.dimension() {
            if constraints[row].is_some() {
                output[row] = Scalar::ZERO;
                continue;
            }
            let mut sum = i128::from(scale.raw()) * i128::from(residual[row].raw());
            for index in self.range(row) {
                let column = self.columns[index] as usize;
                if column >= row {
                    break;
                }
                sum = sum
                    .checked_sub(
                        i128::from(self.values[index].raw()) * i128::from(output[column].raw()),
                    )
                    .ok_or(PhysicsError::Overflow)?;
            }
            output[row] = Scalar::from_raw_ratio(sum, i128::from(diagonal[row].raw()))?;
        }
        for row in (0..self.dimension()).rev() {
            if constraints[row].is_some() {
                continue;
            }
            let mut sum = i128::from(diagonal[row].raw()) * i128::from(output[row].raw());
            for index in self.range(row).rev() {
                let column = self.columns[index] as usize;
                if column <= row {
                    break;
                }
                sum = sum
                    .checked_sub(
                        i128::from(self.values[index].raw()) * i128::from(output[column].raw()),
                    )
                    .ok_or(PhysicsError::Overflow)?;
            }
            output[row] = Scalar::from_raw_ratio(sum, i128::from(diagonal[row].raw()))?;
        }
        Ok(())
    }

    fn range(self, row: usize) -> core::ops::Range<usize> {
        self.rows[row] as usize..self.rows[row + 1] as usize
    }

    fn entry(self, row: usize, column: usize) -> Option<Scalar> {
        let range = self.range(row);
        self.columns[range.clone()]
            .binary_search(&(column as u32))
            .ok()
            .map(|i| self.values[range.start + i])
    }
}

#[cfg(test)]
mod tests;
