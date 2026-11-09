use std::sync::OnceLock;

use hearthline_config::physics::PhysicsProofConfig;
use hearthline_config::physics::geometry::{GeometryReference, SolidGeometryDocument};
use hearthline_physics::Scalar;
use hearthline_physics::transport::{
    Boundary, EXPOSED_FACE, Grid3d, SolverLimits, SparseGrid3d, TransportGrid, TransportSystem,
};
use hearthline_project::physics::CompiledPhysicsProof;
use hearthline_project::physics::geometry::geometry_digest;

pub fn exercise(input: &[u8]) {
    if input.first() == Some(&b'Y') {
        if let Ok(source) = std::str::from_utf8(&input[1..input.len().min(16 * 1024)])
            && let Ok(document) = SolidGeometryDocument::from_yaml(source)
            && let Ok(digest) = geometry_digest(&document)
        {
            let mut config = config().clone();
            config.geometry = Some(GeometryReference {
                id: document.id.clone(),
                sha256: digest,
            });
            let _ = CompiledPhysicsProof::compile_with_geometry_and_budget(
                config,
                Some(document),
                4 * 1024 * 1024,
            );
        }
    }
    let byte = |index| input.get(index).copied().unwrap_or(0);
    let grid = Grid3d::new([2; 3], [Scalar::ONE; 3]).unwrap();
    let mut ids = [0; 8];
    let mut count = 0;
    for cell in 0..8 {
        if byte(0) & (1 << cell) != 0 {
            ids[count] = cell;
            count += 1;
        }
    }
    let ids = &ids[..count];
    let mut neighbors = [[EXPOSED_FACE; 6]; 8];
    for (row, id) in ids.iter().enumerate() {
        for face in 0..6 {
            neighbors[row][face] = grid
                .face_neighbor(*id as usize, face)
                .unwrap()
                .and_then(|neighbor| ids.binary_search(&(neighbor as u32)).ok())
                .map_or(EXPOSED_FACE, |local| local as u32);
        }
    }
    if count > 0 && byte(1) & 1 != 0 {
        neighbors[usize::from(byte(2)) % count][usize::from(byte(3)) % 6] = u32::from(byte(4));
    }
    let Ok(topology) = SparseGrid3d::new(grid, ids, &neighbors[..count]) else {
        return;
    };
    let storage = [Scalar::ONE; 8];
    let boundary = [Boundary::default(); 8];
    let system = TransportSystem::on_grid(
        TransportGrid::Sparse(topology),
        &storage[..count],
        [Scalar::ONE; 3],
        &boundary[..count],
        SolverLimits {
            iterations: 16,
            residual: Scalar::from_raw(100),
        },
    )
    .unwrap();
    let mut fields = core::array::from_fn::<_, 8, _>(|index| {
        Scalar::integer(i64::from(byte(index + 5))).unwrap()
    });
    let initial = fields;
    let mut scratch = [Scalar::ZERO; 8];
    let result = system.solve(
        Scalar::ratio(i64::from(byte(3)) + 1, 10).unwrap(),
        &mut fields[..count],
        &[Scalar::ZERO; 8][..count],
        &mut scratch[..count],
    );
    match result {
        Err(_) => assert_eq!(fields, initial),
        Ok(report) => assert_eq!(
            system
                .inventory(&fields[..count])
                .unwrap()
                .checked_sub(system.inventory(&initial[..count]).unwrap())
                .unwrap(),
            report.balance_error
        ),
    }
}

fn config() -> &'static PhysicsProofConfig {
    static CONFIG: OnceLock<PhysicsProofConfig> = OnceLock::new();
    CONFIG.get_or_init(|| {
        let mut config = serde_json::from_str::<hearthline_sim::physics::PhysicsReplay>(
            include_str!("../../../../project/physics/replays/drying-deformation.json"),
        )
        .unwrap()
        .config;
        config.allocation_budget_bytes = 4 * 1024 * 1024;
        config.shape = [6, 5, 3];
        config
    })
}
