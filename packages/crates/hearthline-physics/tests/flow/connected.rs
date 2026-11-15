use hearthline_physics::flow::connected::{
    CellInventory, ConnectedFlow, ConnectedWorkspace, FaceLink, FlowCell, FlowMesh,
};
use hearthline_physics::flow::{
    CompressibleFlow, Conserved, FlowBoundary, FlowWorkspace, IdealGas, Primitive, ViscousTransport,
};
use hearthline_physics::transport::{Grid3d, SolverLimits};
use hearthline_physics::{PhysicsError, Scalar};

#[path = "retries.rs"]
mod retries;

fn s(value: &str) -> Scalar {
    value.parse().unwrap()
}
fn gas() -> IdealGas {
    IdealGas::new(s("1.4")).unwrap()
}
fn limits() -> SolverLimits {
    SolverLimits {
        iterations: 400,
        residual: s("0.00000005"),
    }
}
fn value(pressure: &str, density: &str, velocity: [Scalar; 3]) -> Conserved {
    gas()
        .conserved(Primitive {
            pressure: s(pressure),
            density: s(density),
            velocity,
        })
        .unwrap()
}
fn cells(widths: &[Scalar]) -> Vec<FlowCell> {
    widths
        .iter()
        .enumerate()
        .map(|(i, &width)| {
            let mut faces = [FaceLink::Boundary(0); 6];
            if i > 0 {
                faces[0] = FaceLink::Cell(i as u32 - 1);
            }
            if i + 1 < widths.len() {
                faces[1] = FaceLink::Cell(i as u32 + 1);
            }
            FlowCell::new([width, Scalar::ONE, Scalar::ONE], faces).unwrap()
        })
        .collect()
}
fn initial(cells: &[FlowCell], left: Conserved, right: Conserved) -> Vec<CellInventory> {
    cells
        .iter()
        .enumerate()
        .map(|(i, cell)| {
            CellInventory::from_density(if i == 0 { left } else { right }, cell.volume()).unwrap()
        })
        .collect()
}
fn step(
    solver: &ConnectedFlow<'_>,
    state: &mut [CellInventory],
    dt: Scalar,
) -> hearthline_physics::flow::connected::ConnectedReport {
    let mut trial = vec![CellInventory::default(); state.len()];
    let mut next = trial.clone();
    solver
        .solve(
            dt,
            state,
            &mut ConnectedWorkspace {
                trial: &mut trial,
                next: &mut next,
            },
        )
        .unwrap()
}

#[test]
fn unequal_volumes_conserve_exact_inventory_at_every_commit_without_allocating() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let cells = cells(&[s("0.37"), s("1.13"), s("0.71")]);
            let boundaries = [FlowBoundary::AdiabaticSlipWall];
            let solver = ConnectedFlow::new(
                FlowMesh::new(&cells, &boundaries).unwrap(),
                gas(),
                limits(),
                None,
            )
            .unwrap();
            let mut state = initial(
                &cells,
                value("2", "2", [Scalar::ZERO; 3]),
                value("1", "1", [Scalar::ZERO; 3]),
            );
            let original = state.clone();
            let before = solver.inventory(&state).unwrap();
            let mut trial = state.clone();
            let mut next = state.clone();
            let dt = s("0.001");
            let allocation = allocation_counter::measure(|| {
                for _ in 0..100 {
                    let report = solver
                        .solve(
                            dt,
                            &mut state,
                            &mut ConnectedWorkspace {
                                trial: &mut trial,
                                next: &mut next,
                            },
                        )
                        .unwrap();
                    assert_eq!(report.balance_error, CellInventory::default());
                    assert_eq!(report.boundary_exchange.mass, Scalar::ZERO);
                    assert_eq!(report.boundary_exchange.energy, Scalar::ZERO);
                    let after = solver.inventory(&state).unwrap();
                    assert_eq!(after.mass, before.mass);
                    assert_eq!(after.energy, before.energy);
                    assert!(report.residual <= limits().residual);
                    assert!(report.evaluations <= u64::from(report.iterations) * 2);
                }
            });
            assert_eq!(allocation.count_total, 0);
            assert!(state[0].mass < original[0].mass);
            assert!(state[1].mass > original[1].mass);
            assert!(state[2].mass > original[2].mass);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn sealed_regions_do_not_exchange_and_uniform_rest_is_exact() {
    let mut cells = cells(&[s("0.37"), s("1.13")]);
    cells[0].faces[1] = FaceLink::Boundary(0);
    cells[1].faces[0] = FaceLink::Boundary(0);
    let boundaries = [FlowBoundary::AdiabaticSlipWall];
    let solver = ConnectedFlow::new(
        FlowMesh::new(&cells, &boundaries).unwrap(),
        gas(),
        limits(),
        None,
    )
    .unwrap();
    let mut state = initial(
        &cells,
        value("2", "2", [Scalar::ZERO; 3]),
        value("1", "1", [Scalar::ZERO; 3]),
    );
    let original = state.clone();
    for _ in 0..3 {
        step(&solver, &mut state, s("0.01"));
    }
    assert_eq!(state, original);
}

#[test]
fn viscous_heat_and_shear_cross_the_connected_face_without_creating_energy() {
    let cells = cells(&[s("0.5"), s("1.5")]);
    let boundaries = [FlowBoundary::AdiabaticSlipWall];
    let transport = ViscousTransport::new(s("0.02"), s("0.03"), Scalar::ONE).unwrap();
    let solver = ConnectedFlow::new(
        FlowMesh::new(&cells, &boundaries).unwrap(),
        gas(),
        limits(),
        Some(transport),
    )
    .unwrap();
    let mut state = initial(
        &cells,
        value("2", "1", [Scalar::ZERO, s("0.1"), Scalar::ZERO]),
        value("1", "1", [Scalar::ZERO; 3]),
    );
    let before = solver.inventory(&state).unwrap();
    let original = state.clone();
    for _ in 0..10 {
        step(&solver, &mut state, s("0.001"));
    }
    let after = solver.inventory(&state).unwrap();
    assert_eq!(after.mass, before.mass);
    assert_eq!(after.energy, before.energy);
    assert!(state[1].energy > original[1].energy);
    assert!(state[1].momentum[1] > original[1].momentum[1]);
}

#[test]
fn boundary_exchange_accounts_for_reservoir_supply_and_wall_heat() {
    let mut cells = cells(&[s("0.5"), s("1")]);
    cells[0].faces[0] = FaceLink::Boundary(1);
    let boundaries = [
        FlowBoundary::NoSlipWall {
            velocity: [Scalar::ZERO; 3],
            temperature: Some(s("2")),
        },
        FlowBoundary::Reservoir(Primitive {
            density: s("2"),
            pressure: s("2"),
            velocity: [Scalar::ZERO; 3],
        }),
    ];
    let transport = ViscousTransport::new(s("0.01"), s("0.02"), Scalar::ONE).unwrap();
    let solver = ConnectedFlow::new(
        FlowMesh::new(&cells, &boundaries).unwrap(),
        gas(),
        limits(),
        Some(transport),
    )
    .unwrap();
    let mut state = initial(
        &cells,
        value("1", "1", [Scalar::ZERO; 3]),
        value("1", "1", [Scalar::ZERO; 3]),
    );
    let before = solver.inventory(&state).unwrap();
    let report = step(&solver, &mut state, s("0.001"));
    let after = solver.inventory(&state).unwrap();
    for ((a, b), exchange) in after
        .components()
        .into_iter()
        .zip(before.components())
        .zip(report.boundary_exchange.components())
    {
        assert_eq!(a.checked_sub(b).unwrap(), exchange);
    }
    assert!(report.boundary_exchange.mass > Scalar::ZERO);
    assert!(report.boundary_exchange.energy > Scalar::ZERO);
}

#[test]
fn rejections_preserve_every_authoritative_cell() {
    let cells = cells(&[Scalar::ONE; 2]);
    let boundaries = [FlowBoundary::AdiabaticSlipWall];
    let mesh = FlowMesh::new(&cells, &boundaries).unwrap();
    let solver = ConnectedFlow::new(
        mesh,
        gas(),
        SolverLimits {
            iterations: 1,
            residual: Scalar::from_raw(1),
        },
        None,
    )
    .unwrap();
    let mut state = initial(
        &cells,
        value("2", "2", [Scalar::ZERO; 3]),
        value("1", "1", [Scalar::ZERO; 3]),
    );
    let original = state.clone();
    let mut trial = state.clone();
    let mut next = state.clone();
    assert!(matches!(
        solver.solve(
            s("0.01"),
            &mut state,
            &mut ConnectedWorkspace {
                trial: &mut trial,
                next: &mut next
            }
        ),
        Err(PhysicsError::NonConvergence { .. })
    ));
    assert_eq!(state, original);
    assert_eq!(
        solver.solve(
            Scalar::ZERO,
            &mut state,
            &mut ConnectedWorkspace {
                trial: &mut trial,
                next: &mut next
            }
        ),
        Err(PhysicsError::InvalidCoefficient)
    );
    assert_eq!(
        solver.solve(
            s("0.01"),
            &mut state,
            &mut ConnectedWorkspace {
                trial: &mut trial[..1],
                next: &mut next
            }
        ),
        Err(PhysicsError::BufferLength)
    );
    assert_eq!(state, original);
    assert_eq!(
        solver.solve(
            Scalar::from_raw(i64::MAX),
            &mut state,
            &mut ConnectedWorkspace {
                trial: &mut trial,
                next: &mut next
            }
        ),
        Err(PhysicsError::Overflow)
    );
    assert_eq!(state, original);
    state[1].mass = Scalar::ZERO;
    let invalid = state.clone();
    assert_eq!(
        solver.solve(
            s("0.01"),
            &mut state,
            &mut ConnectedWorkspace {
                trial: &mut trial,
                next: &mut next
            }
        ),
        Err(PhysicsError::InvalidState)
    );
    assert_eq!(state, invalid);
}

#[test]
fn implicit_time_refinement_converges_for_unequal_connected_volumes() {
    let cells = cells(&[s("0.37"), s("0.71"), s("1.13")]);
    let boundaries = [FlowBoundary::AdiabaticSlipWall];
    let solver = ConnectedFlow::new(
        FlowMesh::new(&cells, &boundaries).unwrap(),
        gas(),
        limits(),
        None,
    )
    .unwrap();
    let run = |steps: i64| {
        let mut state = initial(
            &cells,
            value("2", "2", [Scalar::ZERO; 3]),
            value("1", "1", [Scalar::ZERO; 3]),
        );
        let dt = s("0.032")
            .checked_div(Scalar::integer(steps).unwrap())
            .unwrap();
        for _ in 0..steps {
            step(&solver, &mut state, dt);
        }
        state
    };
    let reference = run(64);
    let error = |steps| {
        run(steps)
            .iter()
            .zip(&reference)
            .flat_map(|(a, b)| a.components().into_iter().zip(b.components()))
            .map(|(a, b)| a.checked_sub(b).unwrap().abs().unwrap())
            .max()
            .unwrap()
    };
    let coarse = error(4);
    let medium = error(8);
    let fine = error(16);
    assert!(
        medium < coarse.checked_mul(s("0.65")).unwrap(),
        "{coarse:?} {medium:?}"
    );
    assert!(
        fine < medium.checked_mul(s("0.65")).unwrap(),
        "{medium:?} {fine:?}"
    );
}

fn advected_contact_error(n: usize) -> Scalar {
    let h = Scalar::ratio(1, n as i64).unwrap();
    let mut cells = cells(&vec![h; n]);
    cells[0].faces[0] = FaceLink::Cell(n as u32 - 1);
    cells[n - 1].faces[1] = FaceLink::Cell(0);
    let boundaries = [FlowBoundary::AdiabaticSlipWall];
    // One inventory quantum is n density quanta at volume 1/n. Declare a
    // four-quantum equation tolerance; do not change it inside the solver.
    let verification_limits = SolverLimits {
        iterations: 400,
        residual: Scalar::from_raw(4 * n as i64),
    };
    let solver = ConnectedFlow::new(
        FlowMesh::new(&cells, &boundaries).unwrap(),
        gas(),
        verification_limits,
        None,
    )
    .unwrap();
    // A periodic contact wave: constant pressure/velocity, rho(x,t)=f(x-u*t).
    let profile = |x: Scalar| {
        let complement = Scalar::ONE.checked_sub(x).unwrap();
        Scalar::ONE
            .checked_add(
                x.checked_mul(x)
                    .unwrap()
                    .checked_mul(complement)
                    .unwrap()
                    .checked_mul(complement)
                    .unwrap(),
            )
            .unwrap()
    };
    let mut state: Vec<_> = (0..n)
        .map(|i| {
            let x = Scalar::ratio(2 * i as i64 + 1, 2 * n as i64).unwrap();
            CellInventory::from_density(
                gas()
                    .conserved(Primitive {
                        density: profile(x),
                        velocity: [s("0.25"), Scalar::ZERO, Scalar::ZERO],
                        pressure: Scalar::ONE,
                    })
                    .unwrap(),
                h,
            )
            .unwrap()
        })
        .collect();
    let mut trial = state.clone();
    let mut next = state.clone();
    let dt = s("0.0005");
    for _ in 0..80 {
        solver
            .solve(
                dt,
                &mut state,
                &mut ConnectedWorkspace {
                    trial: &mut trial,
                    next: &mut next,
                },
            )
            .unwrap();
    }
    let mut error = Scalar::ZERO;
    for (i, cell) in state.iter().enumerate() {
        let mut x = Scalar::ratio(2 * i as i64 + 1, 2 * n as i64)
            .unwrap()
            .checked_sub(s("0.01"))
            .unwrap();
        if x < Scalar::ZERO {
            x = x.checked_add(Scalar::ONE).unwrap();
        }
        let difference = cell
            .density(h)
            .unwrap()
            .density
            .checked_sub(profile(x))
            .unwrap()
            .abs()
            .unwrap();
        error = error
            .checked_add(difference.checked_mul(h).unwrap())
            .unwrap();
    }
    error
}

#[test]
fn periodic_contact_wave_converges_toward_its_analytical_spatial_solution() {
    let coarse = advected_contact_error(8);
    let medium = advected_contact_error(16);
    let fine = advected_contact_error(32);
    assert!(
        medium < coarse.checked_mul(s("0.8")).unwrap(),
        "{coarse:?} {medium:?}"
    );
    assert!(
        fine < medium.checked_mul(s("0.8")).unwrap(),
        "{medium:?} {fine:?}"
    );
    assert!(fine < s("0.001"), "{fine:?}");
}

#[test]
fn topology_rejects_nonreciprocal_missing_and_nonconforming_faces() {
    let good = cells(&[Scalar::ONE; 2]);
    let boundaries = [FlowBoundary::AdiabaticSlipWall];
    assert!(FlowMesh::new(&[], &boundaries).is_err());
    for link in [
        FaceLink::Cell(0),
        FaceLink::Cell(99),
        FaceLink::Boundary(99),
    ] {
        let mut cells = good.clone();
        cells[0].faces[1] = link;
        assert!(FlowMesh::new(&cells, &boundaries).is_err());
    }
    let mut cells = good.clone();
    cells[1] = FlowCell::new([Scalar::ONE, s("2"), Scalar::ONE], cells[1].faces).unwrap();
    assert!(FlowMesh::new(&cells, &boundaries).is_err());
    assert!(FlowMesh::new(&good, &[FlowBoundary::Periodic]).is_err());
    assert!(FlowCell::new([Scalar::from_raw(1); 3], [FaceLink::Boundary(0); 6]).is_err());
    let no_slip = [FlowBoundary::NoSlipWall {
        velocity: [Scalar::ZERO; 3],
        temperature: None,
    }];
    assert!(
        ConnectedFlow::new(
            FlowMesh::new(&good, &no_slip).unwrap(),
            gas(),
            limits(),
            None
        )
        .is_err()
    );
}

#[test]
fn connected_inviscid_fields_match_existing_rectangular_solver_to_tolerance() {
    let cells = cells(&[Scalar::ONE; 4]);
    let boundaries = [FlowBoundary::AdiabaticSlipWall];
    let solver = ConnectedFlow::new(
        FlowMesh::new(&cells, &boundaries).unwrap(),
        gas(),
        limits(),
        None,
    )
    .unwrap();
    let mut state = initial(
        &cells,
        value("2", "2", [Scalar::ZERO; 3]),
        value("1", "1", [Scalar::ZERO; 3]),
    );
    let mut dense: Vec<_> = state
        .iter()
        .map(|s| s.density(Scalar::ONE).unwrap())
        .collect();
    let baseline = CompressibleFlow::new(
        Grid3d::new([4, 1, 1], [Scalar::ONE; 3]).unwrap(),
        gas(),
        [boundaries[0]; 6],
        limits(),
    )
    .unwrap();
    let mut trial = dense.clone();
    let mut next = dense.clone();
    for _ in 0..10 {
        step(&solver, &mut state, s("0.001"));
        baseline
            .solve(
                s("0.001"),
                &mut dense,
                &mut FlowWorkspace {
                    trial: &mut trial,
                    next: &mut next,
                },
            )
            .unwrap();
    }
    for (cell, baseline) in state.iter().zip(dense) {
        for (a, b) in cell
            .density(Scalar::ONE)
            .unwrap()
            .components()
            .into_iter()
            .zip(baseline.components())
        {
            assert!(a.checked_sub(b).unwrap().abs().unwrap() <= s("0.000001"));
        }
    }
}

#[test]
fn three_dimensional_viscous_fields_preserve_the_existing_stencil_to_tolerance() {
    let grid = Grid3d::new([4, 2, 2], [Scalar::ONE; 3]).unwrap();
    let cells: Vec<_> = (0..grid.count())
        .map(|cell| {
            let faces =
                core::array::from_fn(|face| match grid.face_neighbor(cell, face).unwrap() {
                    Some(peer) => FaceLink::Cell(peer as u32),
                    None => FaceLink::Boundary(0),
                });
            FlowCell::new(grid.spacing(), faces).unwrap()
        })
        .collect();
    let boundary = FlowBoundary::NoSlipWall {
        velocity: [Scalar::ZERO; 3],
        temperature: Some(s("1.5")),
    };
    let boundaries = [boundary];
    let transport = ViscousTransport::new(s("0.01"), s("0.02"), Scalar::ONE).unwrap();
    let solver = ConnectedFlow::new(
        FlowMesh::new(&cells, &boundaries).unwrap(),
        gas(),
        limits(),
        Some(transport),
    )
    .unwrap();
    let baseline =
        CompressibleFlow::new_viscous(grid, gas(), [boundary; 6], limits(), transport).unwrap();
    let mut dense: Vec<_> = (0..grid.count())
        .map(|cell| {
            gas()
                .conserved(Primitive {
                    density: Scalar::ONE
                        .checked_add(Scalar::ratio(cell as i64, 100).unwrap())
                        .unwrap(),
                    pressure: Scalar::ONE
                        .checked_add(Scalar::ratio((cell % 3) as i64, 10).unwrap())
                        .unwrap(),
                    velocity: [
                        Scalar::ratio((cell % 4) as i64, 100).unwrap(),
                        Scalar::ratio(-((cell / 4 % 2) as i64), 100).unwrap(),
                        Scalar::ratio((cell / 8) as i64, 100).unwrap(),
                    ],
                })
                .unwrap()
        })
        .collect();
    let mut state: Vec<_> = dense
        .iter()
        .map(|value| CellInventory::from_density(*value, Scalar::ONE).unwrap())
        .collect();
    let mut trial = dense.clone();
    let mut next = dense.clone();
    for _ in 0..10 {
        step(&solver, &mut state, s("0.001"));
        baseline
            .solve(
                s("0.001"),
                &mut dense,
                &mut FlowWorkspace {
                    trial: &mut trial,
                    next: &mut next,
                },
            )
            .unwrap();
    }
    for (cell, baseline) in state.iter().zip(dense) {
        for (a, b) in cell
            .density(Scalar::ONE)
            .unwrap()
            .components()
            .into_iter()
            .zip(baseline.components())
        {
            assert!(a.checked_sub(b).unwrap().abs().unwrap() <= s("0.000001"));
        }
    }
}
