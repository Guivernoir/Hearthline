use crate::PhysicsError;

use super::Grid3d;

pub const EXPOSED_FACE: u32 = u32::MAX;
pub type CellNeighbors = [u32; 6];

/// Material cells in ascending x-fast lattice order, with precompiled face
/// adjacency. Empty space owns no transported fields or hidden inventory.
#[derive(Clone, Copy, Debug)]
pub struct SparseGrid3d<'a> {
    grid: Grid3d,
    lattice_ids: &'a [u32],
    neighbors: &'a [CellNeighbors],
}

impl<'a> SparseGrid3d<'a> {
    pub fn new(
        grid: Grid3d,
        lattice_ids: &'a [u32],
        neighbors: &'a [CellNeighbors],
    ) -> Result<Self, PhysicsError> {
        if lattice_ids.is_empty()
            || lattice_ids.len() > u32::MAX as usize
            || lattice_ids.len() != neighbors.len()
            || lattice_ids.windows(2).any(|pair| pair[0] >= pair[1])
            || lattice_ids
                .last()
                .is_some_and(|id| *id as usize >= grid.count())
        {
            return Err(PhysicsError::InvalidGeometry);
        }
        for (&lattice, faces) in lattice_ids.iter().zip(neighbors) {
            for (face, expected) in grid.neighbors(lattice as usize).into_iter().enumerate() {
                let expected = expected
                    .and_then(|(id, _)| u32::try_from(id).ok())
                    .and_then(|id| lattice_ids.binary_search(&id).ok())
                    .map_or(EXPOSED_FACE, |id| id as u32);
                if faces[face] != expected {
                    return Err(PhysicsError::InvalidGeometry);
                }
            }
        }
        Ok(Self {
            grid,
            lattice_ids,
            neighbors,
        })
    }

    pub const fn lattice(self) -> Grid3d {
        self.grid
    }
    pub fn count(self) -> usize {
        self.lattice_ids.len()
    }
    pub fn lattice_ids(self) -> &'a [u32] {
        self.lattice_ids
    }
    pub fn neighbors(self) -> &'a [CellNeighbors] {
        self.neighbors
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TransportGrid<'a> {
    Dense(Grid3d),
    Sparse(SparseGrid3d<'a>),
}

impl TransportGrid<'_> {
    pub fn count(self) -> usize {
        match self {
            Self::Dense(grid) => grid.count(),
            Self::Sparse(grid) => grid.count(),
        }
    }

    pub(super) fn neighbors(self, cell: usize) -> [Option<(usize, usize)>; 6] {
        match self {
            Self::Dense(grid) => grid.neighbors(cell),
            Self::Sparse(grid) => core::array::from_fn(|face| {
                let neighbor = grid.neighbors[cell][face];
                (neighbor != EXPOSED_FACE).then_some((neighbor as usize, face / 2))
            }),
        }
    }
}
