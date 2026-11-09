use hearthline_config::physics::capacity::CapacityKernel;
use hearthline_config::physics::geometry::{
    GeometryReference, SOLID_GEOMETRY_SCHEMA, SolidGeometry, SolidGeometryDocument,
};
use hearthline_model::ComponentId;
use hearthline_physics::transport::{CellNeighbors, EXPOSED_FACE, Grid3d, SparseGrid3d};
use hearthline_physics::{PhysicsError, Scalar};
use sha2::{Digest, Sha256};

use super::storage::{add, multiply};
use super::{PhysicsStorageLayout, error, positive};

/// Digest covers the typed geometry; YAML mapping order/comments are irrelevant.
pub fn geometry_digest(document: &SolidGeometryDocument) -> Result<String, String> {
    validate_document(document)?;
    let source = serde_json::to_vec(document).map_err(|e| e.to_string())?;
    Ok(crate::source::sha256_hex(&Sha256::digest(source)))
}

fn validate_document(document: &SolidGeometryDocument) -> Result<(), String> {
    if document.schema_version != SOLID_GEOMETRY_SCHEMA {
        return Err("unsupported solid geometry schema".into());
    }
    ComponentId::new(&document.id).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub struct SolidDomainPlan {
    grid: Grid3d,
    wall: [usize; 2],
    floor: usize,
    pub cells: u64,
    pub nodes: u64,
}

impl SolidDomainPlan {
    pub fn compile(
        document: &SolidGeometryDocument,
        reference: &GeometryReference,
        grid: Grid3d,
    ) -> Result<Self, String> {
        if document.id != reference.id || geometry_digest(document)? != reference.sha256 {
            return Err("solid geometry identity or digest mismatch".into());
        }
        u32::try_from(grid.count())
            .map_err(|_| "solid lattice exceeds its u32 index representation")?;
        let SolidGeometry::OpenBasin {
            outer_extent_m,
            wall_thickness_m,
            floor_thickness_m,
        } = &document.solid;
        let extent = outer_extent_m.each_ref().map(|value| positive(value));
        let extent = [extent[0].clone()?, extent[1].clone()?, extent[2].clone()?];
        let spacing = grid.spacing();
        let shape = grid.shape();
        for axis in 0..3 {
            let length = spacing[axis]
                .checked_mul(Scalar::integer(shape[axis] as i64).map_err(error)?)
                .map_err(error)?;
            if length != extent[axis] {
                return Err("solid geometry extent does not match the proof lattice".into());
            }
        }
        let aligned = |value: &str, axis: usize| -> Result<usize, String> {
            let value = positive(value)?;
            if value.raw() % spacing[axis].raw() != 0 {
                return Err(
                    "solid faces must align exactly with lattice planes; refine the mesh".into(),
                );
            }
            usize::try_from(value.raw() / spacing[axis].raw()).map_err(|e| e.to_string())
        };
        let wall = [
            aligned(&wall_thickness_m[0], 0)?,
            aligned(&wall_thickness_m[1], 1)?,
        ];
        let floor = aligned(floor_thickness_m, 2)?;
        if wall[0] >= shape[0].div_ceil(2) || wall[1] >= shape[1].div_ceil(2) || floor >= shape[2] {
            return Err("open basin requires positive interior width, depth and height".into());
        }
        let inner = [
            (shape[0] - 2 * wall[0]) as u64,
            (shape[1] - 2 * wall[1]) as u64,
            (shape[2] - floor) as u64,
        ];
        let void_cells = multiply(multiply(inner[0], inner[1])?, inner[2])?;
        let cells = grid.count() as u64 - void_cells;
        let dense_nodes = shape
            .into_iter()
            .try_fold(1_u64, |n, axis| multiply(n, axis as u64 + 1))?;
        u32::try_from(dense_nodes)
            .map_err(|_| "solid node lattice exceeds its u32 index representation")?;
        let void_nodes = multiply(multiply(inner[0] - 1, inner[1] - 1)?, inner[2])?;
        Ok(Self {
            grid,
            wall,
            floor,
            cells,
            nodes: dense_nodes - void_nodes,
        })
    }

    pub(super) fn storage_layout(
        self,
        kernel: CapacityKernel,
    ) -> Result<PhysicsStorageLayout, String> {
        if kernel == CapacityKernel::CompressibleFlow {
            return Err("solid geometry cannot be used as a receiver gas domain".into());
        }
        let (nodes, elements) = if kernel == CapacityKernel::DryingElasticity {
            (self.nodes, multiply(self.cells, 6)?)
        } else {
            (0, 0)
        };
        let mut layout = PhysicsStorageLayout::for_mesh(self.cells, nodes, elements, kernel)?;
        layout.immutable_geometry_bytes = add(
            layout.immutable_geometry_bytes,
            multiply(
                self.cells,
                (size_of::<u32>() + size_of::<CellNeighbors>()) as u64,
            )?,
        )?;
        Ok(layout)
    }

    /// Construction-only compact node map, charged to compiler scratch and
    /// released before runtime field storage is allocated.
    pub(super) fn node_ids(self) -> Result<Vec<u32>, String> {
        let shape = self.grid.shape().map(|n| n + 1);
        let mut nodes = reserved(self.nodes as usize)?;
        for z in 0..shape[2] {
            for y in 0..shape[1] {
                let full = z <= self.floor || y <= self.wall[1] || y >= shape[1] - 1 - self.wall[1];
                for x in 0..if full { shape[0] } else { self.wall[0] + 1 } {
                    nodes.push((x + shape[0] * (y + shape[1] * z)) as u32);
                }
                if !full {
                    for x in shape[0] - 1 - self.wall[0]..shape[0] {
                        nodes.push((x + shape[0] * (y + shape[1] * z)) as u32);
                    }
                }
            }
        }
        if nodes.len() != self.nodes as usize {
            return Err("solid node count mismatch".into());
        }
        Ok(nodes)
    }

    pub(super) fn build(self) -> Result<CompiledSolidDomain, String> {
        let shape = self.grid.shape();
        let mut lattice_ids = reserved(self.cells as usize)?;
        for z in 0..shape[2] {
            for y in 0..shape[1] {
                let full = z < self.floor || y < self.wall[1] || y >= shape[1] - self.wall[1];
                for x in 0..if full { shape[0] } else { self.wall[0] } {
                    lattice_ids.push((x + shape[0] * (y + shape[1] * z)) as u32);
                }
                if !full {
                    for x in shape[0] - self.wall[0]..shape[0] {
                        lattice_ids.push((x + shape[0] * (y + shape[1] * z)) as u32);
                    }
                }
            }
        }
        if lattice_ids.len() != self.cells as usize {
            return Err("solid cell count mismatch".into());
        }
        let mut neighbors = reserved(lattice_ids.len())?;
        for &lattice in &lattice_ids {
            let mut faces = [EXPOSED_FACE; 6];
            for (face, neighbor) in faces.iter_mut().enumerate() {
                if let Some(next) = self
                    .grid
                    .face_neighbor(lattice as usize, face)
                    .map_err(error)?
                    && let Ok(local) = lattice_ids.binary_search(&(next as u32))
                {
                    *neighbor = local as u32;
                }
            }
            neighbors.push(faces);
        }
        let domain = CompiledSolidDomain {
            plan: self,
            lattice_ids,
            neighbors,
        };
        domain.topology().map_err(error)?;
        Ok(domain)
    }
}

#[derive(Clone, Debug)]
pub struct CompiledSolidDomain {
    plan: SolidDomainPlan,
    lattice_ids: Vec<u32>,
    neighbors: Vec<CellNeighbors>,
}

impl CompiledSolidDomain {
    pub const fn plan(&self) -> SolidDomainPlan {
        self.plan
    }
    pub fn lattice_ids(&self) -> &[u32] {
        &self.lattice_ids
    }
    pub fn topology(&self) -> Result<SparseGrid3d<'_>, PhysicsError> {
        SparseGrid3d::new(self.plan.grid, &self.lattice_ids, &self.neighbors)
    }
    pub fn exposed_faces(&self, cell: usize) -> Option<[bool; 6]> {
        self.neighbors
            .get(cell)
            .map(|faces| faces.map(|neighbor| neighbor == EXPOSED_FACE))
    }
}

fn reserved<T>(count: usize) -> Result<Vec<T>, String> {
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|e| e.to_string())?;
    Ok(values)
}
