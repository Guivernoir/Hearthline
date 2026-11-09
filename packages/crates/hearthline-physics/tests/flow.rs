use hearthline_physics::flow::{
    CompressibleFlow, Conserved, FlowBoundary, FlowWorkspace, IdealGas, Primitive,
};
use hearthline_physics::flow::{FlowAdvanceWorkspace, FlowRetryPolicy};
use hearthline_physics::transport::{Grid3d, SolverLimits};
use hearthline_physics::{PhysicsError, Scalar};

fn s(value: &str) -> Scalar {
    value.parse().unwrap()
}
fn gas() -> IdealGas {
    IdealGas::new(s("1.4")).unwrap()
}

#[test]
fn gas_reference_scales_preserve_declared_si_units_and_reject_lost_resolution() {
    use hearthline_physics::flow::GasScales;
    let scales = GasScales::new(s("2"), s("10"), s("3"), s("5")).unwrap();
    assert_eq!(scales.time(), s("0.2"));
    assert_eq!(scales.pressure(), s("300"));
    assert_eq!(scales.temperature(), s("20"));
    assert_eq!(scales.length_to_solver(s("4")).unwrap(), s("2"));
    assert_eq!(scales.density_to_solver(s("6")).unwrap(), s("2"));
    assert_eq!(scales.density_to_si(s("2")).unwrap(), s("6"));
    assert_eq!(scales.pressure_to_solver(s("900")).unwrap(), s("3"));
    assert_eq!(scales.pressure_to_si(s("3")).unwrap(), s("900"));
    assert_eq!(scales.temperature_to_solver(s("300")).unwrap(), s("15"));
    assert_eq!(scales.temperature_to_si(s("15")).unwrap(), s("300"));
    assert_eq!(scales.velocity_to_si(s("2")).unwrap(), s("20"));
    assert_eq!(scales.time_to_solver(s("0.4")).unwrap(), s("2"));
    assert_eq!(scales.viscosity_to_solver(s("60")).unwrap(), Scalar::ONE);
    assert_eq!(
        scales.conductivity_to_solver(s("300")).unwrap(),
        Scalar::ONE
    );
    assert!(GasScales::new(Scalar::ZERO, Scalar::ONE, Scalar::ONE, Scalar::ONE).is_err());
    assert!(GasScales::new(Scalar::ONE, s("0.000000001"), Scalar::ONE, Scalar::ONE).is_err());
}
fn primitive(pressure: &str) -> Primitive {
    Primitive {
        density: Scalar::ONE,
        velocity: [Scalar::ZERO; 3],
        pressure: s(pressure),
    }
}
fn limits() -> SolverLimits {
    SolverLimits {
        iterations: 300,
        residual: s("0.00000001"),
    }
}

#[test]
fn square_root_is_integer_deterministic_and_correctly_rounded() {
    assert_eq!(Scalar::ZERO.sqrt().unwrap(), Scalar::ZERO);
    assert_eq!(s("4").sqrt().unwrap(), s("2"));
    assert_eq!(s("2").sqrt().unwrap(), s("1.414213562"));
    assert_eq!(s("0.000000001").sqrt().unwrap(), s("0.000031623"));
    assert!(s("-1").sqrt().is_err());
    assert_eq!(Scalar::ratio_wide(1, 3).unwrap(), s("0.333333333"));
    assert_eq!(
        Scalar::ratio_wide(i128::MAX, 1),
        Err(PhysicsError::Overflow)
    );
}

#[test]
fn ideal_gas_roundtrip_accounts_for_kinetic_energy() {
    let state = Primitive {
        density: s("2"),
        velocity: [s("0.5"), s("-1"), s("0.25")],
        pressure: s("3"),
    };
    let conserved = gas().conserved(state).unwrap();
    assert_eq!(gas().primitive(conserved).unwrap(), state);
    assert_eq!(conserved.momentum, [s("1"), s("-2"), s("0.5")]);
    assert!(gas().primitive(Conserved::default()).is_err());
    assert!(
        gas()
            .primitive(Conserved {
                density: Scalar::ONE,
                momentum: [s("10"); 3],
                total_energy: Scalar::ONE
            })
            .is_err()
    );
    assert!(IdealGas::new(Scalar::ONE).is_err());
}

#[test]
fn a_uniform_three_dimensional_gas_remains_at_rest_without_allocations() {
    let grid = Grid3d::new([3; 3], [Scalar::ONE; 3]).unwrap();
    let solver =
        CompressibleFlow::new(grid, gas(), [FlowBoundary::AdiabaticSlipWall; 6], limits()).unwrap();
    let initial = gas().conserved(primitive("1")).unwrap();
    let mut state = [initial; 27];
    let mut trial = [Conserved::default(); 27];
    let mut next = trial;
    let mut workspace = FlowWorkspace {
        trial: &mut trial,
        next: &mut next,
    };
    let measured = allocation_counter::measure(|| {
        for _ in 0..10 {
            let report = solver.solve(s("0.1"), &mut state, &mut workspace).unwrap();
            assert_eq!(report.balance_error, Conserved::default());
        }
    });
    assert_eq!(measured.count_total, 0);
    assert_eq!(state, [initial; 27]);
}

#[test]
fn pressure_drives_momentum_while_closed_walls_conserve_mass_and_energy() {
    let grid = Grid3d::new([4, 2, 2], [s("0.25"); 3]).unwrap();
    let solver =
        CompressibleFlow::new(grid, gas(), [FlowBoundary::AdiabaticSlipWall; 6], limits()).unwrap();
    let mut state: [Conserved; 16] = core::array::from_fn(|cell| {
        gas()
            .conserved(primitive(if cell % 4 < 2 { "2" } else { "1" }))
            .unwrap()
    });
    let before = solver.inventory(&state).unwrap();
    let mut trial = [Conserved::default(); 16];
    let mut next = trial;
    let mut workspace = FlowWorkspace {
        trial: &mut trial,
        next: &mut next,
    };
    let mut error = Conserved::default();
    for _ in 0..20 {
        let report = solver
            .solve(s("0.005"), &mut state, &mut workspace)
            .unwrap();
        error.density = error
            .density
            .checked_add(report.balance_error.density)
            .unwrap();
        error.total_energy = error
            .total_energy
            .checked_add(report.balance_error.total_energy)
            .unwrap();
        assert_eq!(report.boundary_exchange.density, Scalar::ZERO);
        assert_eq!(report.boundary_exchange.total_energy, Scalar::ZERO);
    }
    let after = solver.inventory(&state).unwrap();
    assert!(state[1].momentum[0] > Scalar::ZERO);
    assert!(gas().primitive(state[0]).unwrap().pressure < s("2"));
    assert!(gas().primitive(state[3]).unwrap().pressure > Scalar::ONE);
    assert!(
        after
            .density
            .checked_sub(before.density)
            .unwrap()
            .abs()
            .unwrap()
            < s("0.000001")
    );
    assert!(
        after
            .total_energy
            .checked_sub(before.total_energy)
            .unwrap()
            .abs()
            .unwrap()
            < s("0.000001")
    );
    assert_eq!(
        after.density.checked_sub(before.density).unwrap(),
        error.density
    );
    assert_eq!(
        after.total_energy.checked_sub(before.total_energy).unwrap(),
        error.total_energy
    );
}

#[test]
fn failed_flow_step_keeps_authoritative_conserved_fields() {
    let grid = Grid3d::new([2, 1, 1], [Scalar::ONE; 3]).unwrap();
    let solver = CompressibleFlow::new(
        grid,
        gas(),
        [FlowBoundary::AdiabaticSlipWall; 6],
        SolverLimits {
            iterations: 1,
            residual: Scalar::from_raw(1),
        },
    )
    .unwrap();
    let mut state = [
        gas().conserved(primitive("2")).unwrap(),
        gas().conserved(primitive("1")).unwrap(),
    ];
    let before = state;
    let mut trial = [Conserved::default(); 2];
    let mut next = trial;
    let mut workspace = FlowWorkspace {
        trial: &mut trial,
        next: &mut next,
    };
    assert!(matches!(
        solver.solve(s("0.1"), &mut state, &mut workspace),
        Err(PhysicsError::NonConvergence { .. })
    ));
    assert_eq!(state, before);
    assert!(
        solver
            .solve(Scalar::ZERO, &mut state, &mut workspace)
            .is_err()
    );
    assert!(solver.solve(Scalar::ONE, &mut [], &mut workspace).is_err());
}

#[test]
fn nonlinear_retries_preserve_the_complete_requested_time_and_rollback() {
    let grid = Grid3d::new([2, 1, 1], [s("0.1"); 3]).unwrap();
    let solver =
        CompressibleFlow::new(grid, gas(), [FlowBoundary::AdiabaticSlipWall; 6], limits()).unwrap();
    let original = [
        gas().conserved(primitive("2")).unwrap(),
        gas().conserved(primitive("1")).unwrap(),
    ];
    let mut state = original;
    let mut candidate = [Conserved::default(); 2];
    let mut iteration_trial = candidate;
    let mut iteration_next = candidate;
    let mut work = FlowAdvanceWorkspace {
        candidate: &mut candidate,
        iteration_trial: &mut iteration_trial,
        iteration_next: &mut iteration_next,
    };
    let exhausted = solver
        .advance(
            500_003,
            FlowRetryPolicy {
                maximum_substeps: 1,
                minimum_step_us: 1,
            },
            &mut state,
            &mut work,
        )
        .unwrap_err();
    assert_eq!(exhausted.attempts, 1);
    assert_eq!(state, original);
    let report = solver
        .advance(
            500_003,
            FlowRetryPolicy {
                maximum_substeps: 128,
                minimum_step_us: 1,
            },
            &mut state,
            &mut work,
        )
        .unwrap();
    assert_eq!(report.completed_us, 500_003);
    assert!(report.attempts > 1);
    assert!(report.substeps > 1);
    let first_result = state;
    state = original;
    let second = solver
        .advance(
            500_003,
            FlowRetryPolicy {
                maximum_substeps: 128,
                minimum_step_us: 1,
            },
            &mut state,
            &mut work,
        )
        .unwrap();
    assert_eq!(first_result, state);
    assert_eq!(report, second);
    let before = state;
    assert!(
        solver
            .advance(
                0,
                FlowRetryPolicy {
                    maximum_substeps: 128,
                    minimum_step_us: 1
                },
                &mut state,
                &mut work
            )
            .is_err()
    );
    assert_eq!(state, before);
}
