use hearthline_physics::material::caloric::{CaloricKnot, CaloricLaw};
use hearthline_physics::transport::enthalpy::{
    EnthalpyLimits, EnthalpySystem, EnthalpyWorkspace, WaterTransport,
};
use hearthline_physics::transport::{Boundary, Grid3d, TransportGrid};
use hearthline_physics::{PhysicsError, Scalar};

const INSULATED: Boundary = Boundary {
    conductance: Scalar::ZERO,
    ambient: Scalar::ZERO,
};
fn s(value: &str) -> Scalar {
    value.parse().unwrap()
}
fn law(capacity: Scalar, output: &mut [CaloricKnot; 2]) -> CaloricLaw<'_> {
    CaloricLaw::compile(
        &[[s("0"), capacity], [s("20"), capacity]],
        Scalar::ZERO,
        output,
    )
    .unwrap()
}
fn limits() -> EnthalpyLimits {
    EnthalpyLimits {
        sweeps: 200,
        local_iterations: 64,
        residual: s("0.0000001"),
    }
}

#[test]
fn liquid_transfer_carries_upstream_heat_without_creating_energy() {
    let mut dry_knots = [CaloricKnot::default(); 2];
    let mut liquid_knots = dry_knots;
    let dry = law(s("1"), &mut dry_knots);
    let liquid = law(s("2"), &mut liquid_knots);
    let grid = TransportGrid::Dense(Grid3d::new([2, 1, 1], [Scalar::ONE; 3]).unwrap());
    let coefficients = [[s("1"); 3]; 2];
    let previous = [s("2"), s("0")];
    let current = [s("1.5"), s("0.5")];
    // q=1*(1.5-0.5)=1, dt=0.5: exactly half a mass unit moves left to right.
    let system = EnthalpySystem::new(
        grid,
        &[Scalar::ONE; 2],
        dry,
        [Scalar::ZERO; 3],
        &[INSULATED; 2],
        limits(),
    )
    .unwrap()
    .with_water(WaterTransport {
        liquid,
        previous_moisture: &previous,
        current_moisture: &current,
        conductance: &coefficients,
        boundary: &[INSULATED; 2],
        inlet_temperature: s("0"),
    })
    .unwrap();
    let mut temperature = [s("10"), s("2")];
    let energy = system.previous_inventory(&temperature).unwrap();
    let mut trial = temperature;
    let mut old = trial;
    let mut report = None;
    let measured = allocation_counter::measure(|| {
        report = Some(
            system
                .solve(
                    s("0.5"),
                    &mut temperature,
                    &[Scalar::ZERO; 2],
                    &mut EnthalpyWorkspace {
                        trial: &mut trial,
                        old_enthalpy: &mut old,
                    },
                )
                .unwrap(),
        )
    });
    assert_eq!(measured.count_total, 0);
    assert_eq!(temperature, [s("10"), s("6")]);
    assert_eq!(energy, s("52"));
    assert_eq!(system.inventory(&temperature).unwrap(), energy);
    assert_eq!(report.unwrap().transport.balance_error, Scalar::ZERO);
    assert_eq!(report.unwrap().transport.exchange, Scalar::ZERO);
}

#[test]
fn evaporation_and_condensation_balance_liquid_storage_sensible_flux_and_latent_heat() {
    let mut dry_knots = [CaloricKnot::default(); 2];
    let mut liquid_knots = dry_knots;
    let dry = law(s("1"), &mut dry_knots);
    let liquid = law(s("2"), &mut liquid_knots);
    let grid = TransportGrid::Dense(Grid3d::new([1; 3], [Scalar::ONE; 3]).unwrap());
    for (current, ambient, latent, expected) in
        [("0.5", "0", "-2", "9.333333333"), ("1.5", "2", "2", "9.5")]
    {
        let current = [s(current)];
        let boundary = [Boundary {
            conductance: s("1"),
            ambient: s(ambient),
        }];
        let system = EnthalpySystem::new(
            grid,
            &[Scalar::ONE],
            dry,
            [Scalar::ZERO; 3],
            &[INSULATED],
            limits(),
        )
        .unwrap()
        .with_water(WaterTransport {
            liquid,
            previous_moisture: &[Scalar::ONE],
            current_moisture: &current,
            conductance: &[[Scalar::ZERO; 3]],
            boundary: &boundary,
            inlet_temperature: s("6"),
        })
        .unwrap();
        let mut state = [s("10")];
        let initial = system.previous_inventory(&state).unwrap();
        let mut trial = state;
        let mut old = state;
        let report = system
            .solve(
                s("1"),
                &mut state,
                &[s(latent)],
                &mut EnthalpyWorkspace {
                    trial: &mut trial,
                    old_enthalpy: &mut old,
                },
            )
            .unwrap();
        assert!(state[0].checked_sub(s(expected)).unwrap().abs().unwrap() <= s("0.00000001"));
        assert!(
            system
                .inventory(&state)
                .unwrap()
                .checked_sub(initial)
                .unwrap()
                .checked_sub(report.transport.exchange)
                .unwrap()
                .abs()
                .unwrap()
                <= limits().residual
        );
        assert_eq!(report.transport.exchange > Scalar::ZERO, latent == "2");
    }
}

#[test]
fn disconnected_cells_and_uniform_temperature_preserve_the_appropriate_limits() {
    let mut dry_knots = [CaloricKnot::default(); 2];
    let mut liquid_knots = dry_knots;
    let dry = law(s("1"), &mut dry_knots);
    let liquid = law(s("2"), &mut liquid_knots);
    let grid = TransportGrid::Dense(Grid3d::new([2, 1, 1], [Scalar::ONE; 3]).unwrap());
    for disconnected in [false, true] {
        let previous = [s("2"), s("0")];
        let current = if disconnected {
            previous
        } else {
            [s("1.5"), s("0.5")]
        };
        let coefficients = [[if disconnected {
            Scalar::ZERO
        } else {
            Scalar::ONE
        }; 3]; 2];
        let system = EnthalpySystem::new(
            grid,
            &[Scalar::ONE; 2],
            dry,
            [Scalar::ZERO; 3],
            &[INSULATED; 2],
            limits(),
        )
        .unwrap()
        .with_water(WaterTransport {
            liquid,
            previous_moisture: &previous,
            current_moisture: &current,
            conductance: &coefficients,
            boundary: &[INSULATED; 2],
            inlet_temperature: s("0"),
        })
        .unwrap();
        let mut state = if disconnected {
            [s("10"), s("2")]
        } else {
            [s("10"); 2]
        };
        let before = state;
        let mut trial = state;
        let mut old = state;
        let report = system
            .solve(
                s("0.5"),
                &mut state,
                &[Scalar::ZERO; 2],
                &mut EnthalpyWorkspace {
                    trial: &mut trial,
                    old_enthalpy: &mut old,
                },
            )
            .unwrap();
        assert_eq!(state, before);
        assert_eq!(report.transport.balance_error, Scalar::ZERO);
    }
}

#[test]
fn water_configuration_and_thermal_failures_are_checked_before_commit() {
    let mut dry_knots = [CaloricKnot::default(); 2];
    let mut liquid_knots = dry_knots;
    let dry = law(s("1"), &mut dry_knots);
    let liquid = law(s("2"), &mut liquid_knots);
    let grid = TransportGrid::Dense(Grid3d::new([1; 3], [Scalar::ONE; 3]).unwrap());
    let base = EnthalpySystem::new(
        grid,
        &[Scalar::ONE],
        dry,
        [Scalar::ZERO; 3],
        &[INSULATED],
        limits(),
    )
    .unwrap();
    let negative = [s("-1")];
    let coefficients = [[Scalar::ZERO; 3]];
    let bad_coefficients = [[s("-1"); 3]];
    let bad_boundary = [Boundary {
        conductance: s("-1"),
        ambient: Scalar::ZERO,
    }];
    let mut water = WaterTransport {
        liquid,
        previous_moisture: &[Scalar::ONE],
        current_moisture: &[Scalar::ONE],
        conductance: &coefficients,
        boundary: &[INSULATED],
        inlet_temperature: s("10"),
    };
    for change in 0..7 {
        let mut input = water;
        match change {
            0 => input.previous_moisture = &[],
            1 => input.current_moisture = &negative,
            2 => input.conductance = &[],
            3 => input.conductance = &bad_coefficients,
            4 => input.boundary = &bad_boundary,
            5 => input.boundary = &[],
            _ => input.inlet_temperature = s("21"),
        }
        assert!(base.with_water(input).is_err());
    }
    let mut temperature = [s("10")];
    let before = temperature;
    let mut trial = temperature;
    let mut old = trial;
    let system = base.with_water(water).unwrap();
    let measured = allocation_counter::measure(|| {
        assert_eq!(
            system.solve(
                s("1"),
                &mut temperature,
                &[s("1000")],
                &mut EnthalpyWorkspace {
                    trial: &mut trial,
                    old_enthalpy: &mut old
                }
            ),
            Err(PhysicsError::InvalidState)
        );
    });
    assert_eq!(measured.count_total, 0);
    assert_eq!(temperature, before);
    let tiny_mass = [Scalar::from_raw(1)];
    let tiny_moisture = [Scalar::from_raw(1)];
    let tiny = EnthalpySystem::new(
        grid,
        &tiny_mass,
        dry,
        [Scalar::ZERO; 3],
        &[INSULATED],
        limits(),
    )
    .unwrap()
    .with_water(WaterTransport {
        previous_moisture: &tiny_moisture,
        current_moisture: &tiny_moisture,
        ..water
    })
    .unwrap();
    assert_eq!(
        tiny.inventory(&temperature),
        Err(PhysicsError::InsufficientResolution)
    );
    assert_eq!(
        tiny.solve(
            s("1"),
            &mut temperature,
            &[Scalar::ZERO],
            &mut EnthalpyWorkspace {
                trial: &mut trial,
                old_enthalpy: &mut old
            }
        ),
        Err(PhysicsError::InsufficientResolution)
    );
    assert_eq!(temperature, before);
    let different = [[s("1"), s("2")], [s("20"), s("2")]];
    let mut knots = [CaloricKnot::default(); 2];
    water.liquid = CaloricLaw::compile(&different, Scalar::ZERO, &mut knots).unwrap();
    assert!(base.with_water(water).is_err());
}

#[test]
fn surface_evaporation_converges_to_the_independently_integrated_wet_energy_equation() {
    let mut dry_knots = [CaloricKnot::default(); 2];
    let mut liquid_knots = dry_knots;
    let dry = law(s("1"), &mut dry_knots);
    let liquid = law(s("2"), &mut liquid_knots);
    let grid = TransportGrid::Dense(Grid3d::new([1; 3], [Scalar::ONE; 3]).unwrap());
    let base = EnthalpySystem::new(
        grid,
        &[Scalar::ONE],
        dry,
        [Scalar::ZERO; 3],
        &[INSULATED],
        limits(),
    )
    .unwrap();
    let boundary = [Boundary {
        conductance: Scalar::ONE,
        ambient: Scalar::ZERO,
    }];
    // w'=-w; E=(1+2*w)*T; E'=-w*(2*T+4).
    // Thus T=10+2*ln((1+2*exp(-t))/3); this is not the discrete recurrence.
    let exact = s("8.905664851");
    let mut previous_error = s("1");
    for count in [1, 2, 4, 8, 16, 32, 64] {
        let dt = Scalar::ratio(1, count).unwrap();
        let mut moisture = Scalar::ONE;
        let mut temperature = [s("10")];
        let mut trial = temperature;
        let mut old_enthalpy = trial;
        for _ in 0..count {
            let previous = [moisture];
            let current = [moisture
                .checked_div(Scalar::ONE.checked_add(dt).unwrap())
                .unwrap()];
            let system = base
                .with_water(WaterTransport {
                    liquid,
                    previous_moisture: &previous,
                    current_moisture: &current,
                    conductance: &[[Scalar::ZERO; 3]],
                    boundary: &boundary,
                    inlet_temperature: s("10"),
                })
                .unwrap();
            let latent = current[0].checked_mul(s("-4")).unwrap();
            system
                .solve(
                    dt,
                    &mut temperature,
                    &[latent],
                    &mut EnthalpyWorkspace {
                        trial: &mut trial,
                        old_enthalpy: &mut old_enthalpy,
                    },
                )
                .unwrap();
            moisture = current[0];
        }
        let error = temperature[0].checked_sub(exact).unwrap().abs().unwrap();
        assert!(error < previous_error, "{count}: {error:?}");
        if count >= 8 {
            assert!(
                error < previous_error.checked_mul(s("0.6")).unwrap(),
                "{count}: {error:?}"
            );
        }
        previous_error = error;
    }
    assert!(previous_error < Scalar::ratio(1, 64).unwrap());
}
