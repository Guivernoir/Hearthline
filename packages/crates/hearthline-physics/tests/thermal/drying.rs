use hearthline_physics::drying::{DryingWorkspace, trial_step};
use hearthline_physics::transport::{Boundary, Grid3d, SolverLimits, TransportSystem};
use hearthline_physics::{PhysicsError, Scalar};

fn s(value: &str) -> Scalar {
    value.parse().unwrap()
}

#[test]
fn latent_heat_matches_evaporated_mass_and_invalid_temperature_is_not_published() {
    let grid = Grid3d::new([1; 3], [Scalar::ONE; 3]).unwrap();
    let capacity = [Scalar::ONE];
    let wet_boundary = [Boundary {
        conductance: Scalar::ONE,
        ambient: Scalar::ZERO,
    }];
    let insulated = [Boundary::default()];
    let limits = SolverLimits {
        iterations: 10,
        residual: Scalar::from_raw(1),
    };
    let moisture_system =
        TransportSystem::new(grid, &capacity, [Scalar::ZERO; 3], &wet_boundary, limits).unwrap();
    let thermal_system =
        TransportSystem::new(grid, &capacity, [Scalar::ZERO; 3], &insulated, limits).unwrap();
    let mut moisture_trial = [Scalar::ZERO];
    let mut temperature_trial = [Scalar::ZERO];
    let mut linear = [Scalar::ZERO];
    let mut heat_sources = [Scalar::ZERO];
    let mut workspace = DryingWorkspace {
        moisture_trial: &mut moisture_trial,
        temperature_trial: &mut temperature_trial,
        linear: &mut linear,
        heat_sources: &mut heat_sources,
    };
    let moisture = [Scalar::ONE];
    let temperature = [s("300")];
    let report = trial_step(
        &moisture_system,
        &thermal_system,
        Scalar::ONE,
        s("100"),
        &moisture,
        &temperature,
        &mut workspace,
    )
    .unwrap();
    assert_eq!(report.water_removed_kg, s("0.5"));
    assert_eq!(workspace.moisture_trial, &[s("0.5")]);
    assert_eq!(workspace.temperature_trial, &[s("250")]);
    assert_eq!(report.thermal.exchange, s("-50"));
    assert_eq!(report.thermal.balance_error, Scalar::ZERO);
    assert_eq!(
        trial_step(
            &moisture_system,
            &thermal_system,
            Scalar::ONE,
            s("1000"),
            &moisture,
            &temperature,
            &mut workspace
        ),
        Err(PhysicsError::InvalidState)
    );
    assert_eq!(temperature, [s("300")]);
    assert_eq!(moisture, [Scalar::ONE]);
    assert_eq!(
        trial_step(
            &moisture_system,
            &thermal_system,
            Scalar::ONE,
            s("1"),
            &[],
            &temperature,
            &mut workspace
        ),
        Err(PhysicsError::BufferLength)
    );
    assert_eq!(
        trial_step(
            &moisture_system,
            &thermal_system,
            Scalar::ONE,
            s("-1"),
            &moisture,
            &temperature,
            &mut workspace
        ),
        Err(PhysicsError::InvalidState)
    );
}
