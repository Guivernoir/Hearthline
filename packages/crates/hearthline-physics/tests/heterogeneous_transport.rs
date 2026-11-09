use hearthline_physics::material::PropertyTable;
use hearthline_physics::transport::{
    Boundary, Grid3d, SolverLimits, TransportGrid, TransportSystem, harmonic_conductance,
    surface_conductance,
};
use hearthline_physics::{PhysicsError, Scalar};

fn s(value: &str) -> Scalar {
    value.parse().unwrap()
}

#[test]
fn a_stagnated_quantized_iterate_reports_resolution_not_false_convergence() {
    let grid = Grid3d::new([1; 3], [Scalar::ONE; 3]).unwrap();
    let boundary = [Boundary {
        conductance: s("9"),
        ambient: Scalar::ZERO,
    }];
    let system = TransportSystem::new(
        grid,
        &[Scalar::ONE],
        [Scalar::ONE; 3],
        &boundary,
        SolverLimits {
            iterations: 100,
            residual: Scalar::from_raw(1),
        },
    )
    .unwrap();
    let mut state = [Scalar::ZERO];
    assert_eq!(
        system.solve(
            s("1"),
            &mut state,
            &[Scalar::from_raw(2)],
            &mut [Scalar::ZERO]
        ),
        Err(PhysicsError::InsufficientResolution)
    );
    assert_eq!(state, [Scalar::ZERO]);
}

#[test]
fn property_interpolation_is_checked_exact_at_knots_and_never_extrapolates() {
    let knots = [
        [s("0"), s("0.1")],
        [s("0.2"), s("0.5")],
        [s("1"), s("0.25")],
    ];
    let table = PropertyTable::new(&knots).unwrap();
    assert_eq!(table.value(s("0.1")).unwrap(), s("0.3"));
    assert_eq!(table.value(s("0.6")).unwrap(), s("0.375"));
    for [x, y] in knots {
        assert_eq!(table.value(x).unwrap(), y);
    }
    assert_eq!(table.domain(), [s("0"), s("1")]);
    for x in [s("-0.000000001"), s("1.000000001")] {
        assert_eq!(table.value(x), Err(PhysicsError::InvalidState));
    }
    for bad in [
        vec![],
        vec![[s("0"), s("1")]],
        vec![[s("0"), s("1")], [s("0"), s("2")]],
        vec![[s("1"), s("1")], [s("0"), s("2")]],
        vec![[s("0"), s("-1")], [s("1"), s("2")]],
    ] {
        assert!(PropertyTable::new(&bad).is_err());
    }
    assert_eq!(
        s("0.000000001")
            .checked_mul_div(s("0.5"), s("0.5"))
            .unwrap(),
        s("0.000000001")
    );
    assert_eq!(
        s("2").checked_mul_div(s("3"), s("0")),
        Err(PhysicsError::DivisionByZero)
    );
    let max = Scalar::from_raw(i64::MAX);
    assert_eq!(max.checked_mul_div(max, max).unwrap(), max);
}

#[test]
fn series_resistance_is_symmetric_resolved_and_handles_extreme_contrasts() {
    assert_eq!(harmonic_conductance(s("1"), s("9")).unwrap(), s("1.8"));
    assert_eq!(surface_conductance(s("1"), s("9")).unwrap(), s("0.9"));
    let max = Scalar::from_raw(i64::MAX);
    assert_eq!(harmonic_conductance(max, max).unwrap(), max);
    assert_eq!(
        harmonic_conductance(Scalar::from_raw(1), max).unwrap(),
        Scalar::from_raw(2)
    );
    assert_eq!(
        surface_conductance(Scalar::from_raw(1), Scalar::from_raw(1)),
        Err(PhysicsError::InsufficientResolution)
    );
    for a in [Scalar::ZERO, Scalar::from_raw(1), s("0.1"), s("1000"), max] {
        for b in [Scalar::ZERO, Scalar::from_raw(1), s("1"), max] {
            assert_eq!(harmonic_conductance(a, b), harmonic_conductance(b, a));
        }
    }
    assert!(harmonic_conductance(s("-1"), s("2")).is_err());
    assert_eq!(surface_conductance(s("0"), s("1")).unwrap(), Scalar::ZERO);
}

#[test]
fn two_materials_match_analytic_backward_euler_and_preserve_inventory() {
    let grid = Grid3d::new([2, 1, 1], [Scalar::ONE; 3]).unwrap();
    let coefficients = [[s("1"); 3], [s("9"); 3]];
    let boundary = [Boundary::default(); 2];
    let system = TransportSystem::heterogeneous(
        TransportGrid::Dense(grid),
        &[Scalar::ONE; 2],
        &coefficients,
        &boundary,
        SolverLimits {
            iterations: 200,
            residual: s("0.000000001"),
        },
    )
    .unwrap();
    let mut values = [s("0"), s("1")];
    let mut work = [Scalar::ZERO; 2];
    let mut report = None;
    let allocations = allocation_counter::measure(|| {
        report = Some(
            system
                .solve(s("1"), &mut values, &[Scalar::ZERO; 2], &mut work)
                .unwrap(),
        );
    });
    assert_eq!(allocations.count_total, 0);
    assert!(
        values[0]
            .checked_sub(Scalar::ratio(9, 23).unwrap())
            .unwrap()
            .abs()
            .unwrap()
            < s("0.00000001")
    );
    assert!(
        values[1]
            .checked_sub(Scalar::ratio(14, 23).unwrap())
            .unwrap()
            .abs()
            .unwrap()
            < s("0.00000001")
    );
    assert!(report.unwrap().balance_error.abs().unwrap() < s("0.00000001"));
}

#[test]
fn zero_conductivity_is_an_isolation_boundary_and_uniform_fields_keep_legacy_results() {
    let grid = Grid3d::new([3; 3], [Scalar::ONE; 3]).unwrap();
    let storage = [Scalar::ONE; 27];
    let boundary = [Boundary::default(); 27];
    let mut coefficients = [[Scalar::ONE; 3]; 27];
    let initial = core::array::from_fn::<_, 27, _>(|i| Scalar::integer(i as i64).unwrap());
    let limits = SolverLimits {
        iterations: 200,
        residual: s("0.00000001"),
    };
    let mut uniform = initial;
    let mut heterogeneous = initial;
    TransportSystem::new(grid, &storage, [Scalar::ONE; 3], &boundary, limits)
        .unwrap()
        .solve(
            s("0.5"),
            &mut uniform,
            &[Scalar::ZERO; 27],
            &mut [Scalar::ZERO; 27],
        )
        .unwrap();
    TransportSystem::heterogeneous(
        TransportGrid::Dense(grid),
        &storage,
        &coefficients,
        &boundary,
        limits,
    )
    .unwrap()
    .solve(
        s("0.5"),
        &mut heterogeneous,
        &[Scalar::ZERO; 27],
        &mut [Scalar::ZERO; 27],
    )
    .unwrap();
    assert_eq!(heterogeneous, uniform);
    coefficients[13] = [Scalar::ZERO; 3];
    let mut state = [Scalar::ZERO; 27];
    state[13] = s("1");
    let before = state;
    TransportSystem::heterogeneous(
        TransportGrid::Dense(grid),
        &storage,
        &coefficients,
        &boundary,
        limits,
    )
    .unwrap()
    .solve(
        s("10"),
        &mut state,
        &[Scalar::ZERO; 27],
        &mut [Scalar::ZERO; 27],
    )
    .unwrap();
    assert_eq!(state, before);
    assert!(
        TransportSystem::heterogeneous(
            TransportGrid::Dense(grid),
            &storage,
            &[],
            &boundary,
            limits
        )
        .is_err()
    );
    coefficients[0][0] = s("-1");
    assert!(
        TransportSystem::heterogeneous(
            TransportGrid::Dense(grid),
            &storage,
            &coefficients,
            &boundary,
            limits
        )
        .is_err()
    );
}
