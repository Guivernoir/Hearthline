use hearthline_physics::material::PropertyTable;
use hearthline_physics::transport::nonlinear::{
    NonlinearDiffusion, NonlinearLimits, NonlinearWorkspace, SurfaceBoundary,
};
use hearthline_physics::transport::{Boundary, Grid3d, SolverLimits, TransportGrid};
use hearthline_physics::{PhysicsError, Scalar};

fn s(value: &str) -> Scalar {
    value.parse().unwrap()
}
fn limits() -> NonlinearLimits {
    NonlinearLimits {
        linear: SolverLimits {
            iterations: 1000,
            residual: s("0.000000001"),
        },
        iterations: 100,
        residual: s("0.0000001"),
        relaxation: s("0.5"),
    }
}

#[test]
fn nonlinear_limits_and_invalid_trial_domains_are_rejected_without_publication() {
    let grid = TransportGrid::Dense(Grid3d::new([2, 1, 1], [Scalar::ONE; 3]).unwrap());
    let knots = [[s("0"), s("1")], [s("1"), s("2")]];
    let table = PropertyTable::new(&knots).unwrap();
    for fault in 0..6 {
        let mut options = limits();
        match fault {
            0 => options.iterations = 0,
            1 => options.linear.iterations = 0,
            2 => options.linear.residual = Scalar::ZERO,
            3 => options.residual = Scalar::ZERO,
            4 => options.relaxation = Scalar::ZERO,
            _ => options.relaxation = s("1.1"),
        }
        assert!(
            NonlinearDiffusion::new(
                grid,
                &[Scalar::ONE; 2],
                [Scalar::ONE; 3],
                table,
                SurfaceBoundary::Insulated,
                options
            )
            .is_err()
        );
    }
    assert!(
        NonlinearDiffusion::new(
            grid,
            &[],
            [Scalar::ONE; 3],
            table,
            SurfaceBoundary::Insulated,
            limits()
        )
        .is_err()
    );
    assert!(
        NonlinearDiffusion::new(
            grid,
            &[Scalar::ZERO; 2],
            [Scalar::ONE; 3],
            table,
            SurfaceBoundary::Insulated,
            limits()
        )
        .is_err()
    );
    assert!(
        NonlinearDiffusion::new(
            grid,
            &[Scalar::ONE; 2],
            [s("-1"); 3],
            table,
            SurfaceBoundary::Insulated,
            limits()
        )
        .is_err()
    );
    assert!(
        NonlinearDiffusion::new(
            grid,
            &[Scalar::ONE; 2],
            [Scalar::ONE; 3],
            table,
            SurfaceBoundary::Film {
                conductance: [s("-1"); 3],
                ambient: Scalar::ZERO
            },
            limits()
        )
        .is_err()
    );
    let system = NonlinearDiffusion::new(
        grid,
        &[Scalar::ONE; 2],
        [Scalar::ONE; 3],
        table,
        SurfaceBoundary::Insulated,
        limits(),
    )
    .unwrap();
    let mut state = [s("0.5"); 2];
    let mut trial = [Scalar::ZERO; 2];
    let mut linear = trial;
    let mut coefficients = [[Scalar::ZERO; 3]; 2];
    let mut boundary = [Boundary::default(); 2];
    let mut workspace = NonlinearWorkspace {
        trial: &mut trial,
        linear: &mut linear,
        cell_conductance: &mut coefficients,
        boundary: &mut boundary,
    };
    assert_eq!(
        system.solve(Scalar::ZERO, &mut state, &[Scalar::ZERO; 2], &mut workspace),
        Err(PhysicsError::InvalidCoefficient)
    );
    assert_eq!(
        system.solve(Scalar::ONE, &mut state, &[], &mut workspace),
        Err(PhysicsError::BufferLength)
    );
    assert_eq!(
        system.solve(Scalar::ONE, &mut state, &[s("1000"); 2], &mut workspace),
        Err(PhysicsError::InvalidState)
    );
    assert_eq!(state, [s("0.5"); 2]);
}

#[test]
fn temporal_refinement_approaches_an_independent_continuous_solution() {
    let grid = Grid3d::new([2, 1, 1], [Scalar::ONE; 3]).unwrap();
    let knots = [
        [s("0"), s("0.1")],
        [s("0.25"), s("0.5")],
        [s("0.75"), s("0.5")],
        [s("1"), s("0.1")],
    ];
    let system = NonlinearDiffusion::new(
        TransportGrid::Dense(grid),
        &[Scalar::ONE; 2],
        [Scalar::ONE; 3],
        PropertyTable::new(&knots).unwrap(),
        SurfaceBoundary::Insulated,
        limits(),
    )
    .unwrap();
    // Along this symmetric trajectory, du/dt=(0.1+1.6u)(1-2u), u(0)=0.
    // The transcendental reference is evaluated only in this test, never in
    // authoritative state. R=(u+1/16)/(1/2-u)=(1/8)*exp(1.8*t).
    let ratio = 0.125 * 1.8_f64.exp();
    let exact = (0.5 * ratio - 0.0625) / (1.0 + ratio);
    let errors = [1, 2, 4, 8, 16].map(|steps| {
        let mut state = [s("0"), s("1")];
        let mut trial = [Scalar::ZERO; 2];
        let mut linear = trial;
        let mut coefficients = [[Scalar::ZERO; 3]; 2];
        let mut boundary = [Boundary::default(); 2];
        let mut workspace = NonlinearWorkspace {
            trial: &mut trial,
            linear: &mut linear,
            cell_conductance: &mut coefficients,
            boundary: &mut boundary,
        };
        for _ in 0..steps {
            system
                .solve(
                    Scalar::ratio(1, steps).unwrap(),
                    &mut state,
                    &[Scalar::ZERO; 2],
                    &mut workspace,
                )
                .unwrap();
        }
        (state[0].raw() as f64 / Scalar::SCALE as f64 - exact).abs()
    });
    assert!(errors.windows(2).all(|p| p[1] < p[0]), "{errors:?}");
    // The single coarse step reaches a knot in the constitutive law. Assess
    // first-order asymptotic convergence on the three finest time grids.
    assert!(
        errors[2..].windows(2).all(|p| p[1] < 0.6 * p[0]),
        "{errors:?}"
    );
    assert!(errors[4] < 0.008, "{errors:?}");
}

#[test]
fn changing_properties_converge_to_analytic_implicit_solution_without_allocation() {
    let grid = Grid3d::new([2, 1, 1], [Scalar::ONE; 3]).unwrap();
    let knots = [
        [s("0"), s("0.1")],
        [s("0.25"), s("0.5")],
        [s("0.75"), s("0.5")],
        [s("1"), s("0.1")],
    ];
    let system = NonlinearDiffusion::new(
        TransportGrid::Dense(grid),
        &[Scalar::ONE; 2],
        [Scalar::ONE; 3],
        PropertyTable::new(&knots).unwrap(),
        SurfaceBoundary::Insulated,
        limits(),
    )
    .unwrap();
    let mut state = [s("0"), s("1")];
    let mut trial = [Scalar::ZERO; 2];
    let mut linear = trial;
    let mut coefficients = [[Scalar::ZERO; 3]; 2];
    let mut boundary = [Boundary::default(); 2];
    let mut workspace = NonlinearWorkspace {
        trial: &mut trial,
        linear: &mut linear,
        cell_conductance: &mut coefficients,
        boundary: &mut boundary,
    };
    let mut report = None;
    let allocations = allocation_counter::measure(|| {
        report = Some(
            system
                .solve(s("1"), &mut state, &[Scalar::ZERO; 2], &mut workspace)
                .unwrap(),
        );
    });
    assert_eq!(allocations.count_total, 0);
    assert!(report.unwrap().iterations > 1);
    assert!(report.unwrap().transport.residual <= limits().residual);
    assert!(report.unwrap().transport.balance_error.abs().unwrap() < s("0.000001"));
    for (actual, exact) in state.into_iter().zip([s("0.25"), s("0.75")]) {
        assert!(actual.checked_sub(exact).unwrap().abs().unwrap() < s("0.000001"));
    }
}

#[test]
fn failed_nonlinear_and_invalid_domain_steps_never_publish_iterates() {
    let grid = Grid3d::new([2, 1, 1], [Scalar::ONE; 3]).unwrap();
    let knots = [[s("0"), s("1")], [s("1"), s("4")]];
    let mut options = limits();
    options.iterations = 1;
    let system = NonlinearDiffusion::new(
        TransportGrid::Dense(grid),
        &[Scalar::ONE; 2],
        [Scalar::ONE; 3],
        PropertyTable::new(&knots).unwrap(),
        SurfaceBoundary::Insulated,
        options,
    )
    .unwrap();
    let mut state = [s("0"), s("1")];
    let before = state;
    let mut trial = [Scalar::ZERO; 2];
    let mut linear = trial;
    let mut coefficients = [[Scalar::ZERO; 3]; 2];
    let mut boundary = [Boundary::default(); 2];
    let mut workspace = NonlinearWorkspace {
        trial: &mut trial,
        linear: &mut linear,
        cell_conductance: &mut coefficients,
        boundary: &mut boundary,
    };
    assert!(matches!(
        system.solve(s("1"), &mut state, &[Scalar::ZERO; 2], &mut workspace),
        Err(PhysicsError::NonConvergence { .. })
    ));
    assert_eq!(state, before);
    state[0] = s("-0.000000001");
    let before = state;
    assert_eq!(
        system.solve(s("1"), &mut state, &[Scalar::ZERO; 2], &mut workspace),
        Err(PhysicsError::InvalidState)
    );
    assert_eq!(state, before);
}

#[test]
fn convection_includes_half_cell_resistance_and_has_an_analytic_inventory_balance() {
    let grid = Grid3d::new([1; 3], [Scalar::ONE; 3]).unwrap();
    let knots = [[s("0"), s("1")], [s("1"), s("1")]];
    let system = NonlinearDiffusion::new(
        TransportGrid::Dense(grid),
        &[Scalar::ONE],
        [Scalar::ONE; 3],
        PropertyTable::new(&knots).unwrap(),
        SurfaceBoundary::Film {
            conductance: [s("2"); 3],
            ambient: s("0"),
        },
        limits(),
    )
    .unwrap();
    let mut state = [s("1")];
    let mut trial = [Scalar::ZERO];
    let mut linear = trial;
    let mut coefficients = [[Scalar::ZERO; 3]];
    let mut boundary = [Boundary::default()];
    let mut workspace = NonlinearWorkspace {
        trial: &mut trial,
        linear: &mut linear,
        cell_conductance: &mut coefficients,
        boundary: &mut boundary,
    };
    let report = system
        .solve(s("1"), &mut state, &[Scalar::ZERO], &mut workspace)
        .unwrap();
    // Six faces, each with film=2 and half-cell=2 in series: total G=6.
    assert_eq!(boundary[0].conductance, s("6"));
    assert!(
        state[0]
            .checked_sub(Scalar::ratio(1, 7).unwrap())
            .unwrap()
            .abs()
            .unwrap()
            < s("0.0000001")
    );
    assert!(report.transport.balance_error.abs().unwrap() < s("0.0000001"));
}
