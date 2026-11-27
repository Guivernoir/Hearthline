use super::*;

#[test]
fn closed_link_matches_explicit_walls_and_retains_trapped_inventory() {
    let boundaries = [FlowBoundary::AdiabaticSlipWall];
    let connected = cells(&[s("0.37"), s("1.13")]);
    let mut walled = connected.clone();
    walled[0].faces[1] = FaceLink::Boundary(0);
    walled[1].faces[0] = FaceLink::Boundary(0);
    let mesh = FlowMesh::new(&connected, &boundaries).unwrap();
    let walls = FlowMesh::new(&walled, &boundaries).unwrap();
    let closed = mesh.with_isolated_faces(&[2, 1]).unwrap();
    let open = mesh.with_isolated_faces(&[0, 0]).unwrap();
    let transport = ViscousTransport::new(s("0.01"), s("0.02"), Scalar::ONE).unwrap();
    let start = initial(
        &connected,
        value("2", "1.5", [s("0.04"), s("0.02"), Scalar::ZERO]),
        value("1", "1", [s("-0.01"), Scalar::ZERO, Scalar::ZERO]),
    );
    let mut outputs = [start.clone(), start.clone(), start.clone(), start.clone()];
    let mut trial = start.clone();
    let mut next = start.clone();
    let allocations = allocation_counter::measure(|| {
        for (mesh, state) in [closed, walls, open, mesh].into_iter().zip(&mut outputs) {
            let flow = ConnectedFlow::new(mesh, gas(), limits(), Some(transport)).unwrap();
            for _ in 0..3 {
                let report = flow
                    .solve(
                        s("0.001"),
                        state,
                        &mut ConnectedWorkspace {
                            trial: &mut trial,
                            next: &mut next,
                        },
                    )
                    .unwrap();
                assert_eq!(report.boundary_exchange.mass, Scalar::ZERO);
                assert_eq!(report.boundary_exchange.energy, Scalar::ZERO);
                assert_eq!(report.balance_error, CellInventory::default());
            }
        }
    });
    assert_eq!(allocations.count_total, 0);
    let [closed, walls, reopened, open] = outputs;
    assert_eq!(closed, walls);
    assert_eq!(reopened, open);
    for (actual, initial) in closed.iter().zip(&start) {
        assert_eq!(actual.mass, initial.mass);
        assert_eq!(actual.energy, initial.energy);
    }
    assert_ne!(closed[0].momentum, start[0].momentum);
    assert!(open[0].mass < start[0].mass);
    let mut reopened_after_isolation = closed;
    let solver = ConnectedFlow::new(mesh, gas(), limits(), Some(transport)).unwrap();
    step(&solver, &mut reopened_after_isolation, s("0.001"));
    assert!(reopened_after_isolation[0].mass < start[0].mass);
}

#[test]
fn isolation_rejects_asymmetry_exterior_overrides_and_invalid_masks() {
    let cells = cells(&[Scalar::ONE; 2]);
    let mesh = FlowMesh::new(&cells, &[FlowBoundary::AdiabaticSlipWall]).unwrap();
    for masks in [[2, 0], [0, 1], [3, 1], [2, 65]] {
        assert_eq!(
            mesh.with_isolated_faces(&masks).unwrap_err(),
            PhysicsError::InvalidGeometry
        );
    }
    assert_eq!(
        mesh.with_isolated_faces(&[0]).unwrap_err(),
        PhysicsError::BufferLength
    );
}
