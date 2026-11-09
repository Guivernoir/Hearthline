use hearthline_physics::material::caloric::{CaloricKnot, CaloricLaw};
use hearthline_physics::transport::enthalpy::{EnthalpyLimits, EnthalpySystem, EnthalpyWorkspace};
use hearthline_physics::transport::{Boundary, Grid3d, TransportGrid};
use hearthline_physics::{PhysicsError, Scalar};

const INSULATED: Boundary = Boundary {
    conductance: Scalar::ZERO,
    ambient: Scalar::ZERO,
};

fn s(value: &str) -> Scalar {
    value.parse().unwrap()
}
fn limits() -> EnthalpyLimits {
    EnthalpyLimits {
        sweeps: 200,
        local_iterations: 64,
        residual: s("0.00000002"),
    }
}

#[test]
fn variable_capacity_heating_stores_the_integral_not_cp_times_temperature_change() {
    let mut knots = [CaloricKnot::default(); 2];
    let law = CaloricLaw::compile(
        &[[s("0"), s("1")], [s("4"), s("5")]],
        Scalar::ZERO,
        &mut knots,
    )
    .unwrap();
    let grid = TransportGrid::Dense(Grid3d::new([1; 3], [s("1"); 3]).unwrap());
    let system = EnthalpySystem::new(
        grid,
        &[Scalar::ONE],
        law,
        [Scalar::ZERO; 3],
        &[INSULATED],
        limits(),
    )
    .unwrap();
    let mut temperature = [Scalar::ZERO];
    let mut trial = temperature;
    let mut old_enthalpy = temperature;
    let mut work = EnthalpyWorkspace {
        trial: &mut trial,
        old_enthalpy: &mut old_enthalpy,
    };
    let mut report = None;
    let measured = allocation_counter::measure(|| {
        report = Some(
            system
                .solve(s("1"), &mut temperature, &[s("1.5")], &mut work)
                .unwrap(),
        )
    });
    assert_eq!(measured.count_total, 0);
    assert!(temperature[0].checked_sub(s("1")).unwrap().abs().unwrap() <= s("0.00000001"));
    let report = report.unwrap();
    assert_eq!(report.transport.exchange, s("1.5"));
    assert!(report.transport.balance_error.abs().unwrap() <= limits().residual);
    assert!(
        law.heat_capacity(temperature[0])
            .unwrap()
            .checked_mul(temperature[0])
            .unwrap()
            > report.transport.exchange
    );
}

#[test]
fn closed_volumes_equilibrate_by_energy_not_arithmetic_temperature_average() {
    let mut knots = [CaloricKnot::default(); 2];
    let law = CaloricLaw::compile(
        &[[s("0"), s("1")], [s("4"), s("5")]],
        Scalar::ZERO,
        &mut knots,
    )
    .unwrap();
    let grid = TransportGrid::Dense(Grid3d::new([2, 1, 1], [s("1"); 3]).unwrap());
    let system = EnthalpySystem::new(
        grid,
        &[Scalar::ONE; 2],
        law,
        [s("1"); 3],
        &[INSULATED; 2],
        limits(),
    )
    .unwrap();
    let mut state = [s("0"), s("2")];
    let initial = system.inventory(&state).unwrap();
    let mut trial = state;
    let mut old = state;
    let mut work = EnthalpyWorkspace {
        trial: &mut trial,
        old_enthalpy: &mut old,
    };
    for _ in 0..32 {
        let report = system
            .solve(s("1"), &mut state, &[Scalar::ZERO; 2], &mut work)
            .unwrap();
        assert_eq!(report.transport.exchange, Scalar::ZERO);
        assert!(report.transport.balance_error.abs().unwrap() < s("0.0000001"));
    }
    // h(T)=T+T^2/2; h(0)+h(2)=4 gives common T=sqrt(5)-1.
    for value in state {
        assert!(value.checked_sub(s("1.236067977")).unwrap().abs().unwrap() < s("0.000001"));
    }
    assert!(
        system
            .inventory(&state)
            .unwrap()
            .checked_sub(initial)
            .unwrap()
            .abs()
            .unwrap()
            < s("0.000002")
    );
}

#[test]
fn heterogeneous_heat_flux_has_the_analytic_implicit_constant_capacity_limit() {
    let mut knots = [CaloricKnot::default(); 2];
    let law =
        CaloricLaw::compile(&[[s("0"), s("1")], [s("2"), s("1")]], s("17"), &mut knots).unwrap();
    let grid = TransportGrid::Dense(Grid3d::new([2, 1, 1], [s("1"); 3]).unwrap());
    let conductance = [[s("1"); 3], [s("9"); 3]];
    let system = EnthalpySystem::heterogeneous(
        grid,
        &[Scalar::ONE; 2],
        law,
        &conductance,
        &[INSULATED; 2],
        limits(),
    )
    .unwrap();
    let mut state = [s("0"), s("1")];
    let mut trial = state;
    let mut old = state;
    let report = system
        .solve(
            s("1"),
            &mut state,
            &[Scalar::ZERO; 2],
            &mut EnthalpyWorkspace {
                trial: &mut trial,
                old_enthalpy: &mut old,
            },
        )
        .unwrap();
    assert!(
        state[0]
            .checked_sub(Scalar::ratio(9, 23).unwrap())
            .unwrap()
            .abs()
            .unwrap()
            < s("0.0000001")
    );
    assert!(
        state[1]
            .checked_sub(Scalar::ratio(14, 23).unwrap())
            .unwrap()
            .abs()
            .unwrap()
            < s("0.0000001")
    );
    assert!(report.transport.balance_error.abs().unwrap() < s("0.0000001"));
}

#[test]
fn energy_domain_and_iteration_failures_are_transactional() {
    let mut knots = [CaloricKnot::default(); 2];
    let law = CaloricLaw::compile(
        &[[s("0"), s("1")], [s("4"), s("5")]],
        Scalar::ZERO,
        &mut knots,
    )
    .unwrap();
    let grid = TransportGrid::Dense(Grid3d::new([2, 1, 1], [s("1"); 3]).unwrap());
    for mode in 0..4 {
        let mut settings = limits();
        if mode == 1 {
            settings.local_iterations = 1;
        }
        if mode == 2 {
            settings.sweeps = 1;
        }
        let system = EnthalpySystem::new(
            grid,
            &[Scalar::ONE; 2],
            law,
            [s("1"); 3],
            &[INSULATED; 2],
            settings,
        )
        .unwrap();
        let mut state = [s("0"), s("2")];
        let before = state;
        let mut trial = state;
        let mut old = state;
        let sources = if mode == 0 {
            [s("1000"); 2]
        } else {
            [Scalar::ZERO; 2]
        };
        let dt = if mode == 3 {
            Scalar::from_raw(1)
        } else {
            s("1")
        };
        let result = system.solve(
            dt,
            &mut state,
            &sources,
            &mut EnthalpyWorkspace {
                trial: &mut trial,
                old_enthalpy: &mut old,
            },
        );
        if mode == 3 {
            assert!(result.is_ok());
        } else {
            assert!(result.is_err(), "mode {mode}");
            assert_eq!(state, before);
        }
    }
    let mut state = [s("-1"), s("2")];
    let mut trial = state;
    let mut old = state;
    let system = EnthalpySystem::new(
        grid,
        &[Scalar::ONE; 2],
        law,
        [s("1"); 3],
        &[INSULATED; 2],
        limits(),
    )
    .unwrap();
    assert_eq!(
        system.solve(
            s("1"),
            &mut state,
            &[Scalar::ZERO; 2],
            &mut EnthalpyWorkspace {
                trial: &mut trial,
                old_enthalpy: &mut old
            }
        ),
        Err(PhysicsError::InvalidState)
    );
}

#[test]
fn endpoint_forcing_converges_in_time_to_an_independent_energy_solution() {
    let mut knots = [CaloricKnot::default(); 2];
    let law = CaloricLaw::compile(
        &[[s("0"), s("1")], [s("4"), s("5")]],
        Scalar::ZERO,
        &mut knots,
    )
    .unwrap();
    let grid = TransportGrid::Dense(Grid3d::new([1; 3], [s("1"); 3]).unwrap());
    let system = EnthalpySystem::new(
        grid,
        &[Scalar::ONE],
        law,
        [Scalar::ZERO; 3],
        &[INSULATED],
        limits(),
    )
    .unwrap();
    let mut previous_error = s("1");
    for steps in [1, 2, 4, 8, 16] {
        let dt = Scalar::ratio(1, steps).unwrap();
        let mut state = [Scalar::ZERO];
        let mut trial = state;
        let mut old = state;
        for step in 1..=steps {
            let source = s("1")
                .checked_add(Scalar::ratio(step, steps).unwrap())
                .unwrap();
            system
                .solve(
                    dt,
                    &mut state,
                    &[source],
                    &mut EnthalpyWorkspace {
                        trial: &mut trial,
                        old_enthalpy: &mut old,
                    },
                )
                .unwrap();
        }
        // h'=1+t, h(0)=0: T(1)=1. Backward Euler instead has
        // h_N=1.5+dt/2 and T_N=sqrt(4+dt)-1.
        let expected = s("4")
            .checked_add(dt)
            .unwrap()
            .sqrt()
            .unwrap()
            .checked_sub(s("1"))
            .unwrap();
        assert!(state[0].checked_sub(expected).unwrap().abs().unwrap() < s("0.0000001"));
        let error = state[0].checked_sub(s("1")).unwrap().abs().unwrap();
        assert!(error < previous_error.checked_mul(s("0.55")).unwrap());
        previous_error = error;
    }
}

#[test]
fn invalid_buffers_coefficients_and_unrepresentable_energy_are_rejected() {
    let mut knots = [CaloricKnot::default(); 2];
    let law = CaloricLaw::compile(
        &[[s("0"), s("4")], [s("1"), s("4")]],
        Scalar::ZERO,
        &mut knots,
    )
    .unwrap();
    let grid = TransportGrid::Dense(Grid3d::new([1; 3], [s("1"); 3]).unwrap());
    for settings in [
        EnthalpyLimits {
            sweeps: 0,
            ..limits()
        },
        EnthalpyLimits {
            local_iterations: 0,
            ..limits()
        },
        EnthalpyLimits {
            residual: Scalar::ZERO,
            ..limits()
        },
    ] {
        assert!(
            EnthalpySystem::new(
                grid,
                &[Scalar::ONE],
                law,
                [Scalar::ZERO; 3],
                &[INSULATED],
                settings
            )
            .is_err()
        );
    }
    assert!(
        EnthalpySystem::new(grid, &[], law, [Scalar::ZERO; 3], &[INSULATED], limits()).is_err()
    );
    assert!(
        EnthalpySystem::new(
            grid,
            &[Scalar::ZERO],
            law,
            [Scalar::ZERO; 3],
            &[INSULATED],
            limits()
        )
        .is_err()
    );
    assert!(
        EnthalpySystem::new(
            grid,
            &[Scalar::ONE],
            law,
            [s("-1"); 3],
            &[INSULATED],
            limits()
        )
        .is_err()
    );
    assert!(
        EnthalpySystem::heterogeneous(grid, &[Scalar::ONE], law, &[], &[INSULATED], limits())
            .is_err()
    );
    let bad_boundary = [Boundary {
        conductance: s("-1"),
        ambient: s("0"),
    }];
    assert!(
        EnthalpySystem::new(
            grid,
            &[Scalar::ONE],
            law,
            [Scalar::ZERO; 3],
            &bad_boundary,
            limits()
        )
        .is_err()
    );
    let system = EnthalpySystem::new(
        grid,
        &[Scalar::ONE],
        law,
        [Scalar::ZERO; 3],
        &[INSULATED],
        EnthalpyLimits {
            residual: Scalar::from_raw(1),
            ..limits()
        },
    )
    .unwrap();
    let mut state = [Scalar::ZERO];
    let mut trial = state;
    let mut old = state;
    assert_eq!(system.inventory(&[]), Err(PhysicsError::BufferLength));
    assert_eq!(
        system.solve(
            s("1"),
            &mut state,
            &[],
            &mut EnthalpyWorkspace {
                trial: &mut trial,
                old_enthalpy: &mut old
            }
        ),
        Err(PhysicsError::BufferLength)
    );
    assert_eq!(
        system.solve(
            Scalar::ZERO,
            &mut state,
            &[Scalar::ONE],
            &mut EnthalpyWorkspace {
                trial: &mut trial,
                old_enthalpy: &mut old
            }
        ),
        Err(PhysicsError::InvalidCoefficient)
    );
    let measured = allocation_counter::measure(|| {
        assert_eq!(
            system.solve(
                s("1"),
                &mut state,
                &[Scalar::from_raw(2)],
                &mut EnthalpyWorkspace {
                    trial: &mut trial,
                    old_enthalpy: &mut old
                }
            ),
            Err(PhysicsError::InsufficientResolution)
        );
        for source in [Scalar::from_raw(1), Scalar::from_raw(-1)] {
            assert_eq!(
                system.solve(
                    Scalar::from_raw(1),
                    &mut state,
                    &[source],
                    &mut EnthalpyWorkspace {
                        trial: &mut trial,
                        old_enthalpy: &mut old
                    }
                ),
                Err(PhysicsError::InsufficientResolution)
            );
        }
    });
    assert_eq!(measured.count_total, 0);
    assert_eq!(state, [Scalar::ZERO]);
}
