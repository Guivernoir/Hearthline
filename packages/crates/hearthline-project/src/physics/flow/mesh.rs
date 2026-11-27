//! Compile named Cartesian regions into one conforming, jointly solved mesh.
//! All coordinates must already use one nondimensional reference system.

use hearthline_model::ComponentId;
use hearthline_physics::PhysicsError;
use hearthline_physics::flow::FlowBoundary;
use hearthline_physics::flow::connected::{CellInventory, FaceLink, FlowCell, FlowMesh};
use hearthline_physics::transport::Grid3d;
use sha2::{Digest, Sha256};

use crate::physics::schedule::PhysicsInstanceIdentity;

const CONTRACT: &str = "hearthline-connected-flow-mesh/1:conforming-cartesian:stable-regions";

#[derive(Clone, Debug)]
pub struct FlowRegion {
    pub identity: PhysicsInstanceIdentity,
    pub grid: Grid3d,
    pub boundaries: [FlowBoundary; 6],
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FlowPatch {
    pub region: PhysicsInstanceIdentity,
    pub face: u8,
}

#[derive(Clone, Debug)]
pub struct FlowPatchConnection {
    pub id: ComponentId,
    pub first: FlowPatch,
    pub second: FlowPatch,
}

#[derive(Clone, Debug)]
pub struct CompiledFlowRegion {
    pub identity: PhysicsInstanceIdentity,
    pub grid: Grid3d,
    pub offset: usize,
}

#[derive(Clone, Debug)]
pub struct CompiledFlowConnection {
    pub id: ComponentId,
    pub regions: [usize; 2],
    pub faces: [u8; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectedMemoryPlan {
    pub cells: u64,
    pub immutable_bytes: u64,
    pub field_bytes: u64,
    pub compiler_input_bytes: u64,
    pub peak_planned_bytes: u64,
    pub allocation_budget_bytes: u64,
}

impl ConnectedMemoryPlan {
    pub(super) fn compile(
        regions: &[FlowRegion],
        region_capacity: usize,
        connection_capacity: usize,
        connection_count: usize,
        budget: u64,
    ) -> Result<Self, String> {
        let count = regions.iter().try_fold(0_u64, |n, region| {
            n.checked_add(region.grid.count() as u64)
                .ok_or("connected mesh count overflow")
        })?;
        if count == 0 || count > u64::from(u32::MAX) || regions.len() > u32::MAX as usize / 6 {
            return Err(
                "connected mesh requires nonempty, u32-addressable cells and boundaries".into(),
            );
        }
        let bytes = |count: u64, size: usize| {
            count
                .checked_mul(size as u64)
                .ok_or("connected mesh memory overflow")
        };
        let immutable_bytes = bytes(count, size_of::<FlowCell>())?
            .checked_add(bytes(
                regions.len() as u64,
                size_of::<CompiledFlowRegion>() + 6 * size_of::<FlowBoundary>(),
            )?)
            .and_then(|n| {
                n.checked_add(
                    bytes(connection_count as u64, size_of::<CompiledFlowConnection>()).ok()?,
                )
            })
            .and_then(|n| n.checked_add(size_of::<CompiledFlowMesh>() as u64 + 64))
            .ok_or("connected mesh memory overflow")?;
        let field_bytes = bytes(count, 3 * size_of::<CellInventory>())?;
        let compiler_input_bytes = bytes(region_capacity as u64, size_of::<FlowRegion>())?
            .checked_add(bytes(
                connection_capacity as u64,
                size_of::<FlowPatchConnection>(),
            )?)
            .ok_or("connected mesh memory overflow")?;
        let peak_planned_bytes = immutable_bytes
            .checked_add(field_bytes.max(compiler_input_bytes))
            .ok_or("connected mesh memory overflow")?;
        if u128::from(peak_planned_bytes) * 4 > u128::from(budget) * 3 {
            return Err("connected flow mesh exceeds the shared budget with 25% reserve".into());
        }
        Ok(Self {
            cells: count,
            immutable_bytes,
            field_bytes,
            compiler_input_bytes,
            peak_planned_bytes,
            allocation_budget_bytes: budget,
        })
    }
}

#[derive(Debug)]
pub struct CompiledFlowMesh {
    cells: Box<[FlowCell]>,
    boundaries: Box<[FlowBoundary]>,
    regions: Box<[CompiledFlowRegion]>,
    connections: Box<[CompiledFlowConnection]>,
    memory: ConnectedMemoryPlan,
    digest: String,
}

impl CompiledFlowMesh {
    pub fn compile(
        mut regions: Vec<FlowRegion>,
        mut connections: Vec<FlowPatchConnection>,
        budget: u64,
    ) -> Result<Self, String> {
        let memory = ConnectedMemoryPlan::compile(
            &regions,
            regions.capacity(),
            connections.capacity(),
            connections.len(),
            budget,
        )?;
        regions.sort_unstable_by(|a, b| a.identity.cmp(&b.identity));
        connections.sort_unstable_by(|a, b| a.id.cmp(&b.id));
        if regions.windows(2).any(|p| p[0].identity == p[1].identity)
            || connections.windows(2).any(|p| p[0].id == p[1].id)
        {
            return Err("duplicate connected flow region or connection identity".into());
        }
        for connection in &mut connections {
            if connection.second < connection.first {
                core::mem::swap(&mut connection.first, &mut connection.second);
            }
        }
        let mut cells = Vec::with_capacity(memory.cells as usize);
        let mut boundaries = Vec::with_capacity(regions.len() * 6);
        let mut compiled_regions = Vec::with_capacity(regions.len());
        for region in &regions {
            let offset = cells.len();
            let boundary_offset = boundaries.len();
            for local in 0..region.grid.count() {
                let mut faces = [FaceLink::Boundary(0); 6];
                for (face, link) in faces.iter_mut().enumerate() {
                    *link = match region
                        .grid
                        .face_neighbor(local, face)
                        .map_err(|e| e.to_string())?
                    {
                        Some(peer) => FaceLink::Cell((offset + peer) as u32),
                        None => FaceLink::Boundary((boundary_offset + face) as u32),
                    };
                }
                cells.push(FlowCell::new(region.grid.spacing(), faces).map_err(|e| e.to_string())?);
            }
            boundaries.extend_from_slice(&region.boundaries);
            compiled_regions.push(CompiledFlowRegion {
                identity: region.identity.clone(),
                grid: region.grid,
                offset,
            });
        }
        let mut compiled_connections = Vec::with_capacity(connections.len());
        for connection in &connections {
            let a = compiled_regions
                .binary_search_by(|r| r.identity.cmp(&connection.first.region))
                .map_err(|_| "unknown first flow region")?;
            let b = compiled_regions
                .binary_search_by(|r| r.identity.cmp(&connection.second.region))
                .map_err(|_| "unknown second flow region")?;
            connect(
                &mut cells,
                [&compiled_regions[a], &compiled_regions[b]],
                [connection.first.face, connection.second.face],
            )?;
            compiled_connections.push(CompiledFlowConnection {
                id: connection.id.clone(),
                regions: [a, b],
                faces: [connection.first.face, connection.second.face],
            });
        }
        FlowMesh::new(&cells, &boundaries).map_err(|e| e.to_string())?;
        let mut result = Self {
            cells: cells.into_boxed_slice(),
            boundaries: boundaries.into_boxed_slice(),
            regions: compiled_regions.into_boxed_slice(),
            connections: compiled_connections.into_boxed_slice(),
            memory,
            digest: String::new(),
        };
        result.digest = result.hash();
        Ok(result)
    }

    pub fn mesh(&self) -> Result<FlowMesh<'_>, String> {
        FlowMesh::new(&self.cells, &self.boundaries).map_err(|e| e.to_string())
    }

    /// Dense derived masks cost one byte per cell, separately from authoritative
    /// connection positions. Callers preallocate this buffer before sealing.
    pub fn isolation_mask_bytes(&self) -> usize {
        self.cells.len()
    }

    /// Rebuilds both sides of each closed named patch. Invalid buffer sizes are
    /// rejected before writing; connection order is the compiled ID order.
    pub fn write_isolation_mask(
        &self,
        closed_connections: &[bool],
        masks: &mut [u8],
    ) -> Result<(), PhysicsError> {
        if closed_connections.len() != self.connections.len() || masks.len() != self.cells.len() {
            return Err(PhysicsError::BufferLength);
        }
        masks.fill(0);
        for (connection, &closed) in self.connections.iter().zip(closed_connections) {
            if !closed {
                continue;
            }
            let regions = connection.regions.map(|index| &self.regions[index]);
            let axis = usize::from(connection.faces[0]) / 2;
            let transverse = [(axis + 1) % 3, (axis + 2) % 3];
            for u in 0..regions[0].grid.shape()[transverse[0]] {
                for v in 0..regions[0].grid.shape()[transverse[1]] {
                    let pair = patch_pair(regions, connection.faces, u, v);
                    for side in 0..2 {
                        masks[pair[side]] |= 1 << connection.faces[side];
                    }
                }
            }
        }
        Ok(())
    }

    pub fn isolated_mesh<'a>(&'a self, masks: &'a [u8]) -> Result<FlowMesh<'a>, PhysicsError> {
        FlowMesh::new(&self.cells, &self.boundaries)?.with_isolated_faces(masks)
    }
    pub fn regions(&self) -> &[CompiledFlowRegion] {
        &self.regions
    }
    pub fn connections(&self) -> &[CompiledFlowConnection] {
        &self.connections
    }
    pub const fn memory(&self) -> &ConnectedMemoryPlan {
        &self.memory
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }

    fn hash(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(CONTRACT);
        hash.update((self.regions.len() as u64).to_be_bytes());
        for region in &self.regions {
            for id in [
                &region.identity.site,
                &region.identity.cell,
                &region.identity.instance,
            ] {
                hash_id(&mut hash, id);
            }
            for size in region.grid.shape() {
                hash.update((size as u64).to_be_bytes());
            }
            for spacing in region.grid.spacing() {
                hash.update(spacing.raw().to_be_bytes());
            }
        }
        hash.update((self.connections.len() as u64).to_be_bytes());
        for connection in &self.connections {
            hash_id(&mut hash, &connection.id);
            for region in connection.regions {
                hash.update((region as u64).to_be_bytes());
            }
            hash.update(connection.faces);
        }
        for boundary in &self.boundaries {
            match boundary {
                FlowBoundary::AdiabaticSlipWall => hash.update([0]),
                FlowBoundary::Periodic => hash.update([1]),
                FlowBoundary::Reservoir(state) => {
                    hash.update([2]);
                    for value in [state.density, state.pressure]
                        .into_iter()
                        .chain(state.velocity)
                    {
                        hash.update(value.raw().to_be_bytes());
                    }
                }
                FlowBoundary::NoSlipWall {
                    velocity,
                    temperature,
                } => {
                    hash.update([3]);
                    for value in velocity {
                        hash.update(value.raw().to_be_bytes());
                    }
                    hash.update([u8::from(temperature.is_some())]);
                    if let Some(t) = temperature {
                        hash.update(t.raw().to_be_bytes());
                    }
                }
            }
        }
        crate::source::sha256_hex(hash.finalize().as_slice())
    }
}

fn hash_id(hash: &mut Sha256, id: &ComponentId) {
    hash.update((id.as_str().len() as u64).to_be_bytes());
    hash.update(id.as_str());
}

fn connect(
    cells: &mut [FlowCell],
    regions: [&CompiledFlowRegion; 2],
    faces: [u8; 2],
) -> Result<(), String> {
    if faces.iter().any(|face| *face >= 6) || faces[0] ^ 1 != faces[1] {
        return Err("connected flow patches must be opposite faces on one axis".into());
    }
    let axis = usize::from(faces[0]) / 2;
    let transverse = [(axis + 1) % 3, (axis + 2) % 3];
    for direction in transverse {
        if regions[0].grid.shape()[direction] != regions[1].grid.shape()[direction]
            || regions[0].grid.spacing()[direction] != regions[1].grid.spacing()[direction]
        {
            return Err(
                "nonconforming flow patches require an explicit conservative mortar mesh".into(),
            );
        }
    }
    for u in 0..regions[0].grid.shape()[transverse[0]] {
        for v in 0..regions[0].grid.shape()[transverse[1]] {
            let ids = patch_pair(regions, faces, u, v);
            if (0..2).any(|side| {
                !matches!(
                    cells[ids[side]].faces[usize::from(faces[side])],
                    FaceLink::Boundary(_)
                )
            }) {
                return Err("flow patch is already connected".into());
            }
            for side in 0..2 {
                cells[ids[side]].faces[usize::from(faces[side])] =
                    FaceLink::Cell(ids[side ^ 1] as u32);
            }
        }
    }
    Ok(())
}

fn patch_pair(regions: [&CompiledFlowRegion; 2], faces: [u8; 2], u: usize, v: usize) -> [usize; 2] {
    let axis = usize::from(faces[0]) / 2;
    let transverse = [(axis + 1) % 3, (axis + 2) % 3];
    core::array::from_fn(|side| {
        let shape = regions[side].grid.shape();
        let mut xyz = [0; 3];
        xyz[axis] = if faces[side].is_multiple_of(2) {
            0
        } else {
            shape[axis] - 1
        };
        xyz[transverse[0]] = u;
        xyz[transverse[1]] = v;
        regions[side].offset + xyz[0] + shape[0] * (xyz[1] + shape[1] * xyz[2])
    })
}
