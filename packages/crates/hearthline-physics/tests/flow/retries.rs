use super::*;
use hearthline_physics::flow::connected::{ConnectedAdvanceWorkspace, ConnectedRetryPolicy};

#[test]
fn whole_horizon_retries_preserve_time_inventory_and_match_explicit_substeps() {
    let cells = cells(&[s("0.37"), s("0.71")]);
    let boundaries = [FlowBoundary::AdiabaticSlipWall];
    let limits = SolverLimits {
        iterations: 14,
        residual: s("0.0000001"),
    };
    let solver = ConnectedFlow::new(
        FlowMesh::new(&cells, &boundaries).unwrap(),
        gas(),
        limits,
        None,
    )
    .unwrap();
    let state = initial(
        &cells,
        value("2", "2", [Scalar::ZERO; 3]),
        value("1", "1", [Scalar::ZERO; 3]),
    );
    let mut candidate = state.clone();
    let mut trial = state.clone();
    let mut next = state.clone();
    let horizon = s("0.010000001");
    let policy = ConnectedRetryPolicy {
        maximum_substeps: 128,
        minimum_step: Scalar::from_raw(1),
    };
    let mut result = None;
    let allocation = allocation_counter::measure(|| {
        result = Some(
            solver
                .advance(
                    horizon,
                    policy,
                    &state,
                    &mut ConnectedAdvanceWorkspace {
                        candidate: &mut candidate,
                        trial: &mut trial,
                        next: &mut next,
                    },
                )
                .unwrap(),
        );
    });
    let report = result.unwrap();
    assert_eq!(report.substeps, 64);
    assert_eq!(allocation.count_total, 0);
    assert!(report.substeps > 1);
    assert_eq!(report.attempts, report.substeps.trailing_zeros() + 1);
    assert_eq!(report.completed, horizon);
    assert!(report.accepted_evaluations <= policy.evaluation_bound(horizon, limits).unwrap());
    let before = solver.inventory(&state).unwrap();
    let after = solver.inventory(&candidate).unwrap();
    for ((before, after), exchange) in before
        .components()
        .into_iter()
        .zip(after.components())
        .zip(report.boundary_exchange.components())
    {
        assert_eq!(after.checked_sub(before).unwrap(), exchange);
    }
    let mut explicit = state.clone();
    let mut elapsed = Scalar::ZERO;
    let floor = horizon.raw() / i64::from(report.substeps);
    let remainder = horizon.raw() % i64::from(report.substeps);
    for index in 0..report.substeps {
        let dt = Scalar::from_raw(floor + i64::from(i64::from(index) < remainder));
        elapsed = elapsed.checked_add(dt).unwrap();
        solver
            .solve(
                dt,
                &mut explicit,
                &mut ConnectedWorkspace {
                    trial: &mut trial,
                    next: &mut next,
                },
            )
            .unwrap();
    }
    assert_eq!(elapsed, horizon);
    assert_eq!(candidate, explicit);
}

#[test]
fn exhausted_retries_and_invalid_policy_leave_accepted_fields_untouched() {
    let cells = cells(&[s("0.37"), s("0.71")]);
    let boundaries = [FlowBoundary::AdiabaticSlipWall];
    let solver = ConnectedFlow::new(
        FlowMesh::new(&cells, &boundaries).unwrap(),
        gas(),
        SolverLimits {
            iterations: 1,
            residual: Scalar::from_raw(1),
        },
        None,
    )
    .unwrap();
    let state = initial(
        &cells,
        value("2", "2", [Scalar::ZERO; 3]),
        value("1", "1", [Scalar::ZERO; 3]),
    );
    let original = state.clone();
    let mut candidate = state.clone();
    let mut trial = state.clone();
    let mut next = state.clone();
    for (maximum_substeps, minimum, attempts, substeps) in [
        (8, "0.000001", 4, 8),
        (8, "0.0003", 2, 2),
        (1, "0.000001", 1, 1),
    ] {
        let allocation = allocation_counter::measure(|| {
            let error = solver
                .advance(
                    s("0.001"),
                    ConnectedRetryPolicy {
                        maximum_substeps,
                        minimum_step: s(minimum),
                    },
                    &state,
                    &mut ConnectedAdvanceWorkspace {
                        candidate: &mut candidate,
                        trial: &mut trial,
                        next: &mut next,
                    },
                )
                .unwrap_err();
            assert!(matches!(error.cause, PhysicsError::NonConvergence { .. }));
            assert_eq!(error.attempts, attempts);
            assert_eq!(error.substeps, substeps);
            assert_eq!(error.completed_substeps, 0);
        });
        assert_eq!(allocation.count_total, 0);
        assert_eq!(state, original);
    }
    for policy in [
        ConnectedRetryPolicy {
            maximum_substeps: 0,
            minimum_step: Scalar::ONE,
        },
        ConnectedRetryPolicy {
            maximum_substeps: 3,
            minimum_step: Scalar::from_raw(1),
        },
        ConnectedRetryPolicy {
            maximum_substeps: 2,
            minimum_step: Scalar::ZERO,
        },
        ConnectedRetryPolicy {
            maximum_substeps: 2,
            minimum_step: Scalar::ONE,
        },
    ] {
        let error = solver
            .advance(
                s("0.001"),
                policy,
                &state,
                &mut ConnectedAdvanceWorkspace {
                    candidate: &mut candidate,
                    trial: &mut trial,
                    next: &mut next,
                },
            )
            .unwrap_err();
        assert_eq!(error.attempts, 0);
        assert_eq!(error.cause, PhysicsError::InvalidCoefficient);
    }
    assert!(
        ConnectedRetryPolicy {
            maximum_substeps: 1 << 31,
            minimum_step: Scalar::from_raw(1)
        }
        .evaluation_bound(
            Scalar::from_raw(i64::MAX),
            SolverLimits {
                iterations: u32::MAX,
                residual: Scalar::ONE
            }
        )
        .is_err()
    );
}

#[test]
fn fatal_arithmetic_and_invalid_inputs_are_not_retried() {
    let cells = cells(&[s("0.37"), s("0.71")]);
    let boundaries = [FlowBoundary::AdiabaticSlipWall];
    let solver = ConnectedFlow::new(
        FlowMesh::new(&cells, &boundaries).unwrap(),
        gas(),
        limits(),
        None,
    )
    .unwrap();
    let state = initial(
        &cells,
        value("2", "2", [Scalar::ZERO; 3]),
        value("1", "1", [Scalar::ZERO; 3]),
    );
    let mut candidate = state.clone();
    let mut trial = state.clone();
    let mut next = state.clone();
    let policy = ConnectedRetryPolicy {
        maximum_substeps: 8,
        minimum_step: Scalar::from_raw(1),
    };
    let error = solver
        .advance(
            Scalar::from_raw(i64::MAX),
            policy,
            &state,
            &mut ConnectedAdvanceWorkspace {
                candidate: &mut candidate,
                trial: &mut trial,
                next: &mut next,
            },
        )
        .unwrap_err();
    assert_eq!(error.cause, PhysicsError::Overflow);
    assert_eq!(error.attempts, 1);
    let error = solver
        .advance(
            s("0.01"),
            policy,
            &state,
            &mut ConnectedAdvanceWorkspace {
                candidate: &mut candidate[..1],
                trial: &mut trial,
                next: &mut next,
            },
        )
        .unwrap_err();
    assert_eq!(error.cause, PhysicsError::BufferLength);
    assert_eq!(error.attempts, 0);
    let mut invalid = state.clone();
    invalid[1].mass = Scalar::ZERO;
    let error = solver
        .advance(
            s("0.01"),
            policy,
            &invalid,
            &mut ConnectedAdvanceWorkspace {
                candidate: &mut candidate,
                trial: &mut trial,
                next: &mut next,
            },
        )
        .unwrap_err();
    assert_eq!(error.cause, PhysicsError::InvalidState);
    assert_eq!(error.attempts, 0);
}

#[test]
fn partial_attempts_are_discarded_before_recovery_and_never_double_count_supply() {
    let mut cells = cells(&[s("0.37"), s("0.71")]);
    cells[0].faces[0] = FaceLink::Boundary(1);
    let boundaries = [
        FlowBoundary::AdiabaticSlipWall,
        FlowBoundary::Reservoir(Primitive {
            density: s("2"),
            pressure: s("2"),
            velocity: [Scalar::ONE, Scalar::ZERO, Scalar::ZERO],
        }),
    ];
    let solver = ConnectedFlow::new(
        FlowMesh::new(&cells, &boundaries).unwrap(),
        gas(),
        SolverLimits {
            iterations: 24,
            residual: s("0.0000001"),
        },
        None,
    )
    .unwrap();
    let state = initial(
        &cells,
        value("1", "1", [Scalar::ZERO; 3]),
        value("1", "1", [Scalar::ZERO; 3]),
    );
    let original = state.clone();
    let mut candidate = state.clone();
    let mut trial = state.clone();
    let mut next = state.clone();
    let policy = ConnectedRetryPolicy {
        maximum_substeps: 4,
        minimum_step: Scalar::from_raw(1),
    };
    let allocation = allocation_counter::measure(|| {
        let error = solver
            .advance(
                Scalar::ONE,
                policy,
                &state,
                &mut ConnectedAdvanceWorkspace {
                    candidate: &mut candidate,
                    trial: &mut trial,
                    next: &mut next,
                },
            )
            .unwrap_err();
        assert_eq!(error.attempts, 3);
        assert_eq!(error.substeps, 4);
        assert_eq!(error.completed_substeps, 2);
        assert!(matches!(error.cause, PhysicsError::NonConvergence { .. }));
    });
    assert_eq!(allocation.count_total, 0);
    assert_eq!(state, original);
    assert_ne!(candidate, original);
    let mut accepted = None;
    let allocation = allocation_counter::measure(|| {
        accepted = Some(
            solver
                .advance(
                    Scalar::ONE,
                    ConnectedRetryPolicy {
                        maximum_substeps: 8,
                        ..policy
                    },
                    &state,
                    &mut ConnectedAdvanceWorkspace {
                        candidate: &mut candidate,
                        trial: &mut trial,
                        next: &mut next,
                    },
                )
                .unwrap(),
        );
    });
    assert_eq!(allocation.count_total, 0);
    let report = accepted.unwrap();
    assert_eq!(report.substeps, 8);
    assert_eq!(report.attempts, 4);
    let mut explicit = original;
    for _ in 0..8 {
        solver
            .solve(
                s("0.125"),
                &mut explicit,
                &mut ConnectedWorkspace {
                    trial: &mut trial,
                    next: &mut next,
                },
            )
            .unwrap();
    }
    assert_eq!(candidate, explicit);
    let before = solver.inventory(&state).unwrap();
    let after = solver.inventory(&candidate).unwrap();
    assert!(report.boundary_exchange.mass > Scalar::ZERO);
    for ((old, new), exchange) in before
        .components()
        .into_iter()
        .zip(after.components())
        .zip(report.boundary_exchange.components())
    {
        assert_eq!(new.checked_sub(old).unwrap(), exchange);
    }
}
