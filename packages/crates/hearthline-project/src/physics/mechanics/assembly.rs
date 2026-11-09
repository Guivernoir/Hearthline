use hearthline_physics::mechanics::{IsotropicElasticity, StiffnessMatrix, Tetrahedron};
use hearthline_physics::{PhysicsError, Scalar};

use super::reserved;
use crate::physics::storage::{add, multiply};

#[derive(Clone, Debug)]
pub(super) struct CompiledStiffness {
    rows: Vec<u32>,
    columns: Vec<u32>,
    values: Vec<Scalar>,
}

struct Entry {
    row: u32,
    column: u32,
    element: u32,
    value: Scalar,
}

/// Conservative preflight before any element or assembly allocation. Temporary
/// triplets are released before the simulation's field arrays are constructed.
pub(in crate::physics) fn storage_bound(nodes: u64, elements: u64) -> Result<(u64, u64), String> {
    let entries = multiply(elements, 144)?;
    let dofs = multiply(nodes, 3)?;
    if entries > u64::from(u32::MAX) || dofs > u64::from(u32::MAX) {
        return Err("mechanical sparse index capacity exceeded".into());
    }
    let retained = add(multiply(add(dofs, 1)?, 4)?, multiply(entries, 12)?)?;
    let scratch = add(
        multiply(entries, size_of::<Entry>() as u64)?,
        multiply(nodes, 4)?,
    )?;
    Ok((retained, scratch))
}

impl CompiledStiffness {
    pub(super) fn compile(
        nodes: usize,
        elements: &[Tetrahedron],
        materials: &[IsotropicElasticity],
    ) -> Result<Self, String> {
        storage_bound(nodes as u64, elements.len() as u64)?;
        if elements.len() != materials.len() {
            return Err("mechanical material count mismatch".into());
        }
        let mut entries = reserved::<Entry>(elements.len() * 144)?;
        for (index, (element, &material)) in elements.iter().zip(materials).enumerate() {
            let nodes = element.nodes();
            for row in 0..12 {
                for column in 0..12 {
                    let value = element
                        .stiffness(material, row, column)
                        .map_err(|e| e.to_string())?;
                    if value != Scalar::ZERO {
                        entries.push(Entry {
                            row: (nodes[row / 3] * 3 + row % 3) as u32,
                            column: (nodes[column / 3] * 3 + column % 3) as u32,
                            element: index as u32,
                            value,
                        });
                    }
                }
            }
        }
        // The element index makes the order total, including duplicate DOFs.
        // Unstable in-place sorting needs no additional heap workspace.
        entries.sort_unstable_by_key(|e| (e.row, e.column, e.element));
        let mut count = 0;
        for index in 0..entries.len() {
            let Entry {
                row,
                column,
                value,
                element,
            } = entries[index];
            if count > 0 && (entries[count - 1].row, entries[count - 1].column) == (row, column) {
                entries[count - 1].value = entries[count - 1]
                    .value
                    .checked_add(value)
                    .map_err(|e| e.to_string())?;
            } else {
                entries[count] = Entry {
                    row,
                    column,
                    value,
                    element,
                };
                count += 1;
            }
        }
        let mut rows = reserved(nodes * 3 + 1)?;
        rows.resize(nodes * 3 + 1, 0);
        let mut columns = reserved(count)?;
        let mut values = reserved(count)?;
        for entry in &entries[..count] {
            rows[entry.row as usize + 1] += 1;
            columns.push(entry.column);
            values.push(entry.value);
        }
        for row in 1..rows.len() {
            rows[row] += rows[row - 1];
        }
        let matrix = Self {
            rows,
            columns,
            values,
        };
        matrix.view().map_err(|e| e.to_string())?;
        Ok(matrix)
    }

    pub(super) fn view(&self) -> Result<StiffnessMatrix<'_>, PhysicsError> {
        StiffnessMatrix::new(&self.rows, &self.columns, &self.values)
    }

    pub(super) fn bytes(&self) -> u64 {
        (self.rows.len() * size_of::<u32>()
            + self.columns.len() * size_of::<u32>()
            + self.values.len() * size_of::<Scalar>()) as u64
    }
}
