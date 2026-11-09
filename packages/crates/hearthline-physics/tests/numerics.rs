use hearthline_physics::gas::IsothermalReceiver;
use hearthline_physics::transport::{Boundary, Grid3d, SolverLimits, TransportSystem};
use hearthline_physics::{PhysicsError, Scalar};

fn s(value: &str) -> Scalar {
    value.parse().unwrap()
}
fn limits() -> SolverLimits {
    SolverLimits {
        iterations: 10_000,
        residual: s("0.00000001"),
    }
}

#[test]
fn decimals_are_exact_checked_and_round_ties_to_even() {
    assert_eq!(s("-0.000000001").raw(), -1);
    assert_eq!(s("9223372036.854775807").raw(), i64::MAX);
    assert_eq!(s("-9223372036.854775808").raw(), i64::MIN);
    for value in [
        "",
        "-",
        ".1",
        "1.",
        "1e2",
        "NaN",
        " 1",
        "1.0000000001",
        "1.2.3",
        "+1",
        "1x",
    ] {
        assert!(value.parse::<Scalar>().is_err(), "{value}");
    }
    assert_eq!(
        "9223372036.854775808".parse::<Scalar>(),
        Err(PhysicsError::Overflow)
    );
    assert_eq!(Scalar::from_raw(1).checked_mul(s("0.5")).unwrap().raw(), 0);
    assert_eq!(Scalar::from_raw(3).checked_mul(s("0.5")).unwrap().raw(), 2);
    assert_eq!(
        Scalar::from_raw(-3).checked_mul(s("0.5")).unwrap().raw(),
        -2
    );
    assert_eq!(Scalar::from_raw(5).checked_div(s("2")).unwrap().raw(), 2);
    assert_eq!(Scalar::from_raw(7).checked_div(s("2")).unwrap().raw(), 4);
    assert_eq!(Scalar::ratio(-1, 3).unwrap(), s("-0.333333333"));
    assert_eq!(Scalar::ratio(1, -3).unwrap(), s("-0.333333333"));
}

#[test]
fn arithmetic_never_silently_saturates_or_divides_by_zero() {
    let max = Scalar::from_raw(i64::MAX);
    let min = Scalar::from_raw(i64::MIN);
    assert_eq!(max.checked_add(Scalar::ONE), Err(PhysicsError::Overflow));
    assert_eq!(min.checked_sub(Scalar::ONE), Err(PhysicsError::Overflow));
    assert_eq!(max.checked_mul(s("2")), Err(PhysicsError::Overflow));
    assert_eq!(max.checked_div(s("0.1")), Err(PhysicsError::Overflow));
    assert_eq!(min.checked_neg(), Err(PhysicsError::Overflow));
    assert_eq!(min.abs(), Err(PhysicsError::Overflow));
    assert_eq!(Scalar::integer(i64::MAX), Err(PhysicsError::Overflow));
    assert_eq!(
        Scalar::ONE.checked_div(Scalar::ZERO),
        Err(PhysicsError::DivisionByZero)
    );
    assert_eq!(Scalar::ratio(1, 0), Err(PhysicsError::DivisionByZero));
}

#[test]
fn geometry_rejects_empty_overflow_and_unrepresentable_volume() {
    for shape in [[0, 1, 1], [usize::MAX, 2, 2]] {
        assert!(Grid3d::new(shape, [Scalar::ONE; 3]).is_err());
    }
    assert!(Grid3d::new([1; 3], [Scalar::ZERO; 3]).is_err());
    assert!(Grid3d::new([1; 3], [s("0.000000001"); 3]).is_err());
    let grid = Grid3d::new([2, 3, 4], [s("0.5"); 3]).unwrap();
    assert_eq!(grid.count(), 24);
    assert_eq!(grid.volume().unwrap(), s("0.125"));
    assert_eq!(grid.conductance(s("2")).unwrap(), [Scalar::ONE; 3]);
    assert!(grid.conductance(s("-1")).is_err());
}

#[test]
fn two_volumes_match_implicit_analytic_solution_and_conserve_inventory() {
    let grid = Grid3d::new([2, 1, 1], [Scalar::ONE; 3]).unwrap();
    let storage = [Scalar::ONE; 2];
    let boundary = [Boundary::default(); 2];
    let system =
        TransportSystem::new(grid, &storage, [Scalar::ONE; 3], &boundary, limits()).unwrap();
    let mut field = [Scalar::ONE, Scalar::ZERO];
    let mut work = [Scalar::ZERO; 2];
    let report = system
        .solve(Scalar::ONE, &mut field, &[Scalar::ZERO; 2], &mut work)
        .unwrap();
    assert!(
        field[0]
            .checked_sub(s("0.666666667"))
            .unwrap()
            .abs()
            .unwrap()
            < s("0.0000001")
    );
    assert!(
        field[1]
            .checked_sub(s("0.333333333"))
            .unwrap()
            .abs()
            .unwrap()
            < s("0.0000001")
    );
    assert!(report.balance_error.abs().unwrap() < s("0.0000001"));
    assert_eq!(report.exchange, Scalar::ZERO);
}

#[test]
fn convergence_failure_preserves_authoritative_state() {
    let grid = Grid3d::new([2, 1, 1], [Scalar::ONE; 3]).unwrap();
    let storage = [Scalar::ONE; 2];
    let boundary = [Boundary::default(); 2];
    let system = TransportSystem::new(
        grid,
        &storage,
        [s("100"); 3],
        &boundary,
        SolverLimits {
            iterations: 1,
            residual: Scalar::from_raw(1),
        },
    )
    .unwrap();
    let mut field = [Scalar::ONE, Scalar::ZERO];
    let original = field;
    assert!(matches!(
        system.solve(
            Scalar::ONE,
            &mut field,
            &[Scalar::ZERO; 2],
            &mut [Scalar::ZERO; 2]
        ),
        Err(PhysicsError::NonConvergence { iterations: 1, .. })
    ));
    assert_eq!(field, original);
}

#[test]
fn invalid_solver_inputs_and_overflow_preserve_state() {
    let grid = Grid3d::new([1; 3], [Scalar::ONE; 3]).unwrap();
    assert!(TransportSystem::new(grid, &[], [Scalar::ONE; 3], &[], limits()).is_err());
    for storage in [Scalar::ZERO, s("-1")] {
        assert!(
            TransportSystem::new(
                grid,
                &[storage],
                [Scalar::ONE; 3],
                &[Boundary::default()],
                limits()
            )
            .is_err()
        );
    }
    let storage = [Scalar::ONE];
    let boundary = [Boundary::default()];
    let system =
        TransportSystem::new(grid, &storage, [Scalar::ZERO; 3], &boundary, limits()).unwrap();
    let mut field = [s("9223372036")];
    assert!(
        system
            .solve(Scalar::ONE, &mut field, &[s("100")], &mut [Scalar::ZERO])
            .is_err()
    );
    assert_eq!(field, [s("9223372036")]);
    assert!(
        system
            .solve(
                Scalar::ZERO,
                &mut field,
                &[Scalar::ZERO],
                &mut [Scalar::ZERO]
            )
            .is_err()
    );
    assert!(system.solve(Scalar::ONE, &mut field, &[], &mut []).is_err());
    assert!(system.inventory(&[]).is_err());
    assert!(system.boundary_rates(&field, &mut []).is_err());
}

#[test]
fn constant_three_dimensional_field_is_stationary_and_steps_do_not_allocate() {
    let grid = Grid3d::new([3, 4, 5], [Scalar::ONE; 3]).unwrap();
    let storage = [Scalar::ONE; 60];
    let boundary = [Boundary {
        conductance: Scalar::ONE,
        ambient: s("2"),
    }; 60];
    let system =
        TransportSystem::new(grid, &storage, [Scalar::ONE; 3], &boundary, limits()).unwrap();
    let mut field = [s("2"); 60];
    let mut work = [Scalar::ZERO; 60];
    let allocation = allocation_counter::measure(|| {
        for _ in 0..20 {
            system
                .solve(Scalar::ONE, &mut field, &[Scalar::ZERO; 60], &mut work)
                .unwrap();
        }
    });
    assert_eq!(allocation.count_total, 0);
    assert_eq!(field, [s("2"); 60]);
}

#[test]
fn time_refinement_approaches_continuous_two_volume_solution() {
    fn solve(steps: i64) -> f64 {
        let grid = Grid3d::new([2, 1, 1], [Scalar::ONE; 3]).unwrap();
        let storage = [Scalar::ONE; 2];
        let boundary = [Boundary::default(); 2];
        let system =
            TransportSystem::new(grid, &storage, [Scalar::ONE; 3], &boundary, limits()).unwrap();
        let mut field = [Scalar::ONE, Scalar::ZERO];
        for _ in 0..steps {
            system
                .solve(
                    Scalar::ratio(1, steps).unwrap(),
                    &mut field,
                    &[Scalar::ZERO; 2],
                    &mut [Scalar::ZERO; 2],
                )
                .unwrap();
        }
        field[0].raw() as f64 / Scalar::SCALE as f64
    }
    let exact = (1.0 + (-2.0_f64).exp()) / 2.0;
    let coarse = (solve(10) - exact).abs();
    let fine = (solve(20) - exact).abs();
    assert!(fine < coarse * 0.6);
}

#[test]
fn receiver_uses_absolute_pressure_and_finite_mass() {
    let receiver = IsothermalReceiver::new(s("1"), s("300"), s("287.05")).unwrap();
    let mass = receiver.mass_kg(s("100000")).unwrap();
    assert!(mass > s("1"));
    assert_eq!(receiver.pressure_pa(mass).unwrap(), s("100000"));
    assert!(receiver.mass_kg(Scalar::ZERO).is_err());
    assert!(receiver.pressure_pa(s("-1")).is_err());
    assert!(IsothermalReceiver::new(Scalar::ZERO, s("300"), s("287.05")).is_err());
}

#[test]
fn manufactured_steady_gradient_is_preserved_along_each_spatial_axis() {
    let grid = Grid3d::new([3; 3], [Scalar::ONE; 3]).unwrap();
    for axis in 0..3 {
        let mut values = [Scalar::ZERO; 27];
        let mut boundary = [Boundary::default(); 27];
        for cell in 0..27 {
            let coordinate = (cell / [1, 3, 9][axis]) % 3;
            values[cell] = Scalar::ratio(2 * coordinate as i64 + 1, 2).unwrap();
            if coordinate == 0 {
                boundary[cell] = Boundary {
                    conductance: s("2"),
                    ambient: Scalar::ZERO,
                };
            }
            if coordinate == 2 {
                boundary[cell] = Boundary {
                    conductance: s("2"),
                    ambient: s("3"),
                };
            }
        }
        let storage = [Scalar::ONE; 27];
        let system =
            TransportSystem::new(grid, &storage, [Scalar::ONE; 3], &boundary, limits()).unwrap();
        let before = values;
        system
            .solve(
                Scalar::ONE,
                &mut values,
                &[Scalar::ZERO; 27],
                &mut [Scalar::ZERO; 27],
            )
            .unwrap();
        assert_eq!(before, values);
    }
}
