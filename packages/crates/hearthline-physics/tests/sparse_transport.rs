use hearthline_physics::transport::{
    Boundary, CellNeighbors, EXPOSED_FACE, Grid3d, SolverLimits, SparseGrid3d, TransportGrid,
    TransportSystem,
};
use hearthline_physics::{PhysicsError, Scalar};

fn scalar(n: i64) -> Scalar {
    Scalar::integer(n).unwrap()
}
fn adjacency(grid: Grid3d, ids: &[u32]) -> Vec<CellNeighbors> {
    ids.iter()
        .map(|id| {
            core::array::from_fn(|face| {
                grid.face_neighbor(*id as usize, face)
                    .unwrap()
                    .and_then(|id| ids.binary_search(&(id as u32)).ok())
                    .map_or(EXPOSED_FACE, |local| local as u32)
            })
        })
        .collect()
}

#[test]
fn full_sparse_lattice_is_bit_identical_to_dense_transport() {
    let grid = Grid3d::new([3, 2, 2], [Scalar::ONE; 3]).unwrap();
    let ids = (0..12).collect::<Vec<_>>();
    let faces = adjacency(grid, &ids);
    let sparse = SparseGrid3d::new(grid, &ids, &faces).unwrap();
    let storage = [Scalar::ONE; 12];
    let boundary = [Boundary::default(); 12];
    let limits = SolverLimits {
        iterations: 200,
        residual: Scalar::from_raw(10),
    };
    let dense = TransportSystem::new(grid, &storage, [Scalar::ONE; 3], &boundary, limits).unwrap();
    let sparse = TransportSystem::on_grid(
        TransportGrid::Sparse(sparse),
        &storage,
        [Scalar::ONE; 3],
        &boundary,
        limits,
    )
    .unwrap();
    let mut dense_fields = core::array::from_fn::<_, 12, _>(|i| scalar(i as i64));
    let mut sparse_fields = dense_fields;
    let mut scratch = [Scalar::ZERO; 12];
    let sources = [Scalar::ZERO; 12];
    let allocations = allocation_counter::measure(|| {
        for _ in 0..8 {
            let a = dense
                .solve(Scalar::ONE, &mut dense_fields, &sources, &mut scratch)
                .unwrap();
            let b = sparse
                .solve(Scalar::ONE, &mut sparse_fields, &sources, &mut scratch)
                .unwrap();
            assert_eq!(a, b);
            assert_eq!(dense_fields, sparse_fields);
        }
    });
    assert_eq!(allocations.count_total, 0);
}

#[test]
fn empty_space_cannot_store_or_short_circuit_transport() {
    let grid = Grid3d::new([5, 1, 1], [Scalar::ONE; 3]).unwrap();
    let ids = [0, 1, 3, 4];
    let faces = adjacency(grid, &ids);
    let sparse = SparseGrid3d::new(grid, &ids, &faces).unwrap();
    let boundary = [Boundary::default(); 4];
    let system = TransportSystem::on_grid(
        TransportGrid::Sparse(sparse),
        &[Scalar::ONE; 4],
        [Scalar::ONE; 3],
        &boundary,
        SolverLimits {
            iterations: 100,
            residual: Scalar::from_raw(10),
        },
    )
    .unwrap();
    let mut fields = [scalar(4), Scalar::ZERO, scalar(10), scalar(10)];
    let before = system.inventory(&fields).unwrap();
    let report = system
        .solve(
            Scalar::ONE,
            &mut fields,
            &[Scalar::ZERO; 4],
            &mut [Scalar::ZERO; 4],
        )
        .unwrap();
    assert!(fields[0] < scalar(4) && fields[1] > Scalar::ZERO);
    assert_eq!(&fields[2..], &[scalar(10); 2]);
    assert_eq!(
        system
            .inventory(&fields)
            .unwrap()
            .checked_sub(before)
            .unwrap(),
        report.balance_error
    );
    assert!(report.balance_error.abs().unwrap().raw() <= 10);
}

#[test]
fn malformed_sparse_adjacency_is_rejected_before_execution() {
    let grid = Grid3d::new([3, 1, 1], [Scalar::ONE; 3]).unwrap();
    for ids in [vec![], vec![0, 0], vec![1, 0], vec![0, 3]] {
        assert!(SparseGrid3d::new(grid, &ids, &vec![[EXPOSED_FACE; 6]; ids.len()]).is_err());
    }
    let ids = [0, 1, 2];
    let faces = adjacency(grid, &ids);
    for bad in [EXPOSED_FACE, 0, 2, 3] {
        let mut altered = faces.clone();
        altered[0][1] = bad;
        assert!(SparseGrid3d::new(grid, &ids, &altered).is_err());
    }
    assert_eq!(grid.face_neighbor(3, 0), Err(PhysicsError::InvalidGeometry));
    assert_eq!(grid.face_neighbor(0, 6), Err(PhysicsError::InvalidGeometry));
}

#[test]
fn subquantum_boundary_and_interior_conductance_do_not_silently_disable_transport() {
    let grid = Grid3d::new([2, 1, 1], [Scalar::ONE; 3]).unwrap();
    let limits = SolverLimits {
        iterations: 10,
        residual: Scalar::from_raw(1),
    };
    for boundary_case in [false, true] {
        let boundary = [Boundary {
            conductance: if boundary_case {
                Scalar::from_raw(1)
            } else {
                Scalar::ZERO
            },
            ambient: Scalar::ZERO,
        }; 2];
        let conductance = if boundary_case {
            [Scalar::ZERO; 3]
        } else {
            [Scalar::from_raw(1); 3]
        };
        let system =
            TransportSystem::new(grid, &[Scalar::ONE; 2], conductance, &boundary, limits).unwrap();
        let mut fields = [Scalar::ONE, scalar(2)];
        assert_eq!(
            system.solve(
                Scalar::ratio(1, 10).unwrap(),
                &mut fields,
                &[Scalar::ZERO; 2],
                &mut [Scalar::ZERO; 2]
            ),
            Err(PhysicsError::InsufficientResolution)
        );
        assert_eq!(fields, [Scalar::ONE, scalar(2)]);
    }
}
