use crate::{PhysicsError, Scalar};

use super::super::FlowBoundary;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FaceLink {
    Cell(u32),
    Boundary(u32),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum MeshFace {
    Cell(u32),
    Boundary(FlowBoundary),
}

pub const FLOW_ISOLATION_CONTRACT: &str =
    "flow-isolation/1:reciprocal-face-mask:stationary-adiabatic-slip:boundary-impulse";

/// Axis-aligned, conforming control volume. Faces are -X,+X,-Y,+Y,-Z,+Z.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowCell {
    spacing: [Scalar; 3],
    volume: Scalar,
    pub faces: [FaceLink; 6],
}

impl FlowCell {
    pub fn new(spacing: [Scalar; 3], faces: [FaceLink; 6]) -> Result<Self, PhysicsError> {
        if spacing.iter().any(|value| *value <= Scalar::ZERO) {
            return Err(PhysicsError::InvalidGeometry);
        }
        let volume = spacing[0]
            .checked_mul(spacing[1])?
            .checked_mul(spacing[2])?;
        let cell = Self {
            spacing,
            volume,
            faces,
        };
        if volume <= Scalar::ZERO || (0..3).any(|axis| cell.area(axis).is_err()) {
            return Err(PhysicsError::InvalidGeometry);
        }
        Ok(cell)
    }
    pub const fn spacing(self) -> [Scalar; 3] {
        self.spacing
    }
    pub const fn volume(self) -> Scalar {
        self.volume
    }
    pub(super) fn area(self, axis: usize) -> Result<Scalar, PhysicsError> {
        let area = self.spacing[(axis + 1) % 3].checked_mul(self.spacing[(axis + 2) % 3])?;
        if area <= Scalar::ZERO {
            return Err(PhysicsError::InvalidGeometry);
        }
        Ok(area)
    }
}

/// Borrowed compiled topology. Every interior face has exactly one reciprocal
/// peer; unequal normal widths are supported, nonconforming faces are rejected.
#[derive(Clone, Copy, Debug)]
pub struct FlowMesh<'a> {
    pub(super) cells: &'a [FlowCell],
    pub(super) boundaries: &'a [FlowBoundary],
    isolated_faces: Option<&'a [u8]>,
}

impl<'a> FlowMesh<'a> {
    pub fn new(
        cells: &'a [FlowCell],
        boundaries: &'a [FlowBoundary],
    ) -> Result<Self, PhysicsError> {
        if cells.is_empty() || cells.len() > u32::MAX as usize {
            return Err(PhysicsError::InvalidGeometry);
        }
        for (index, cell) in cells.iter().enumerate() {
            for (face, link) in cell.faces.iter().enumerate() {
                match *link {
                    FaceLink::Cell(peer) => {
                        let peer = cells
                            .get(peer as usize)
                            .ok_or(PhysicsError::InvalidGeometry)?;
                        if peer.faces[face ^ 1] != FaceLink::Cell(index as u32)
                            || (0..3).any(|axis| {
                                axis != face / 2 && cell.spacing[axis] != peer.spacing[axis]
                            })
                        {
                            return Err(PhysicsError::InvalidGeometry);
                        }
                    }
                    FaceLink::Boundary(boundary) => {
                        if boundaries
                            .get(boundary as usize)
                            .is_none_or(|b| *b == FlowBoundary::Periodic)
                        {
                            return Err(PhysicsError::InvalidGeometry);
                        }
                    }
                }
            }
        }
        Ok(Self {
            cells,
            boundaries,
            isolated_faces: None,
        })
    }

    /// One six-bit mask per cell. Each closed interior face must be mirrored on
    /// its peer; exterior boundaries cannot be overridden. State is not changed.
    pub fn with_isolated_faces(mut self, masks: &'a [u8]) -> Result<Self, PhysicsError> {
        if masks.len() != self.cells.len() {
            return Err(PhysicsError::BufferLength);
        }
        for (cell, &mask) in self.cells.iter().zip(masks) {
            if mask & !0b11_1111 != 0 {
                return Err(PhysicsError::InvalidGeometry);
            }
            for (face, link) in cell.faces.iter().enumerate() {
                let closed = mask & (1 << face) != 0;
                match *link {
                    FaceLink::Cell(peer) => {
                        if closed != (masks[peer as usize] & (1 << (face ^ 1)) != 0) {
                            return Err(PhysicsError::InvalidGeometry);
                        }
                    }
                    FaceLink::Boundary(_) if closed => return Err(PhysicsError::InvalidGeometry),
                    FaceLink::Boundary(_) => (),
                }
            }
        }
        self.isolated_faces = Some(masks);
        Ok(self)
    }

    pub(super) fn face(self, cell: usize, face: usize) -> MeshFace {
        if self
            .isolated_faces
            .is_some_and(|m| m[cell] & (1 << face) != 0)
        {
            return MeshFace::Boundary(FlowBoundary::AdiabaticSlipWall);
        }
        match self.cells[cell].faces[face] {
            FaceLink::Cell(peer) => MeshFace::Cell(peer),
            FaceLink::Boundary(id) => MeshFace::Boundary(self.boundaries[id as usize]),
        }
    }
    pub fn cells(self) -> &'a [FlowCell] {
        self.cells
    }
    pub fn boundaries(self) -> &'a [FlowBoundary] {
        self.boundaries
    }
}
