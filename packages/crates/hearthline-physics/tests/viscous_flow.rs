use hearthline_physics::Scalar;
use hearthline_physics::flow::{
    CompressibleFlow, Conserved, FlowBoundary, FlowWorkspace, IdealGas, Primitive, ViscousTransport,
};
use hearthline_physics::transport::{Grid3d, SolverLimits};

fn s(value: &str) -> Scalar {
    value.parse().unwrap()
}
fn gas() -> IdealGas {
    IdealGas::new(s("1.4")).unwrap()
}
fn limits() -> SolverLimits {
    SolverLimits {
        iterations: 500,
        residual: s("0.00000001"),
    }
}
fn wall(velocity: [Scalar; 3], temperature: Option<Scalar>) -> FlowBoundary {
    FlowBoundary::NoSlipWall {
        velocity,
        temperature,
    }
}

#[test]
fn newtonian_stress_and_fourier_heat_flux_match_affine_fields() {
    let transport = ViscousTransport::new(s("0.1"), s("0.2"), Scalar::ONE).unwrap();
    let mut gradient = [[Scalar::ZERO; 3]; 3];
    gradient[0][1] = Scalar::ONE;
    let flux = transport
        .flux(gradient, [s("2"), Scalar::ZERO, Scalar::ZERO], s("3"), 1)
        .unwrap();
    assert_eq!(flux.density, Scalar::ZERO);
    assert_eq!(flux.momentum, [s("0.1"), Scalar::ZERO, Scalar::ZERO]);
    assert_eq!(flux.total_energy, s("0.8"));
    gradient[1][0] = s("-1");
    assert_eq!(
        transport
            .flux(gradient, [Scalar::ZERO; 3], Scalar::ZERO, 1)
            .unwrap(),
        Conserved::default()
    );
    gradient = [[Scalar::ZERO; 3]; 3];
    gradient[0][0] = Scalar::ONE;
    let normal = transport
        .flux(gradient, [Scalar::ZERO; 3], Scalar::ZERO, 0)
        .unwrap();
    assert_eq!(normal.momentum[0], s("0.133333333"));
    assert!(
        transport
            .flux(gradient, [Scalar::ZERO; 3], Scalar::ZERO, 3)
            .is_err()
    );
}

#[test]
fn stationary_adiabatic_walls_dissipate_motion_without_destroying_energy_or_mass() {
    let grid = Grid3d::new([2, 4, 2], [s("0.25"); 3]).unwrap();
    let boundaries = [
        FlowBoundary::Periodic,
        FlowBoundary::Periodic,
        wall([Scalar::ZERO; 3], None),
        wall([Scalar::ZERO; 3], None),
        FlowBoundary::Periodic,
        FlowBoundary::Periodic,
    ];
    let solver = CompressibleFlow::new_viscous(
        grid,
        gas(),
        boundaries,
        limits(),
        ViscousTransport::new(s("0.1"), s("0.1"), Scalar::ONE).unwrap(),
    )
    .unwrap();
    let initial = gas()
        .conserved(Primitive {
            density: Scalar::ONE,
            velocity: [s("0.2"), Scalar::ZERO, Scalar::ZERO],
            pressure: Scalar::ONE,
        })
        .unwrap();
    let mut state = [initial; 16];
    let before = solver.inventory(&state).unwrap();
    let mut trial = [Conserved::default(); 16];
    let mut next = trial;
    let mut work = FlowWorkspace {
        trial: &mut trial,
        next: &mut next,
    };
    let allocations = allocation_counter::measure(|| {
        for _ in 0..10 {
            let report = solver.solve(s("0.005"), &mut state, &mut work).unwrap();
            assert_eq!(report.boundary_exchange.total_energy, Scalar::ZERO);
            assert_eq!(report.boundary_exchange.density, Scalar::ZERO);
        }
    });
    assert_eq!(allocations.count_total, 0);
    let after = solver.inventory(&state).unwrap();
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
    assert!(after.momentum[0] < before.momentum[0]);
    assert!(state[0].momentum[0] < state[2].momentum[0]);
}

#[test]
fn heated_walls_and_moving_lids_deliver_heat_and_mechanical_work() {
    let grid = Grid3d::new([2; 3], [s("0.5"); 3]).unwrap();
    for heated in [true, false] {
        let mut boundaries = [wall([Scalar::ZERO; 3], None); 6];
        boundaries[3] = if heated {
            wall([Scalar::ZERO; 3], Some(s("2")))
        } else {
            wall([s("0.2"), Scalar::ZERO, Scalar::ZERO], None)
        };
        let solver = CompressibleFlow::new_viscous(
            grid,
            gas(),
            boundaries,
            limits(),
            ViscousTransport::new(s("0.1"), s("0.1"), Scalar::ONE).unwrap(),
        )
        .unwrap();
        let initial = gas()
            .conserved(Primitive {
                density: Scalar::ONE,
                velocity: [Scalar::ZERO; 3],
                pressure: Scalar::ONE,
            })
            .unwrap();
        let mut state = [initial; 8];
        let mut trial = [Conserved::default(); 8];
        let mut next = trial;
        let before = solver.inventory(&state).unwrap();
        let report = solver
            .solve(
                s("0.005"),
                &mut state,
                &mut FlowWorkspace {
                    trial: &mut trial,
                    next: &mut next,
                },
            )
            .unwrap();
        let after = solver.inventory(&state).unwrap();
        assert!(report.boundary_exchange.total_energy > Scalar::ZERO);
        assert!(after.total_energy > before.total_energy);
        assert_eq!(report.boundary_exchange.density, Scalar::ZERO);
        if !heated {
            assert!(state[2].momentum[0] > Scalar::ZERO);
        }
    }
}

#[test]
fn transport_and_boundary_configuration_cannot_silently_discard_requested_physics() {
    assert!(ViscousTransport::new(s("-1"), Scalar::ONE, Scalar::ONE).is_err());
    assert!(ViscousTransport::new(Scalar::ONE, s("-1"), Scalar::ONE).is_err());
    assert!(ViscousTransport::new(Scalar::ONE, Scalar::ONE, Scalar::ZERO).is_err());
    let grid = Grid3d::new([2; 3], [Scalar::ONE; 3]).unwrap();
    let no_slip = [wall([Scalar::ZERO; 3], None); 6];
    assert!(CompressibleFlow::new(grid, gas(), no_slip, limits()).is_err());
    let transport = ViscousTransport::new(Scalar::ONE, Scalar::ONE, Scalar::ONE).unwrap();
    for change in 0..3 {
        let mut boundaries = no_slip;
        boundaries[0] = match change {
            0 => FlowBoundary::Periodic,
            1 => wall([Scalar::ONE; 3], None),
            _ => wall([Scalar::ZERO; 3], Some(Scalar::ZERO)),
        };
        assert!(
            CompressibleFlow::new_viscous(grid, gas(), boundaries, limits(), transport).is_err()
        );
    }
}
