use hearthline_physics::drying::DryingWorkspace;
use hearthline_physics::drying::variable::{VariableDryingWorkspace, trial_step};
use hearthline_physics::material::PropertyTable;
use hearthline_physics::transport::nonlinear::{
    NonlinearDiffusion, NonlinearLimits, SurfaceBoundary,
};
use hearthline_physics::transport::{
    Boundary, Grid3d, SolverLimits, TransportGrid, TransportSystem,
};
use hearthline_physics::{PhysicsError, Scalar};

fn s(value: &str) -> Scalar {
    value.parse().unwrap()
}

#[test]
fn moisture_dependent_drying_and_condensation_exchange_matching_latent_heat() {
    for (initial, ambient, removes_water) in [("0.2", "0.02", true), ("0.02", "0.2", false)] {
        let grid = Grid3d::new([2, 1, 1], [Scalar::ONE; 3]).unwrap();
        let knots = [[s("0"), s("0.1")], [s("0.3"), s("1")]];
        let options = SolverLimits {
            iterations: 1000,
            residual: s("0.00000001"),
        };
        let moisture_storage = [s("1"); 2];
        let moisture_system = NonlinearDiffusion::new(
            TransportGrid::Dense(grid),
            &moisture_storage,
            [s("0.1"); 3],
            PropertyTable::new(&knots).unwrap(),
            SurfaceBoundary::Film {
                conductance: [s("0.01"); 3],
                ambient: s(ambient),
            },
            NonlinearLimits {
                linear: options,
                iterations: 100,
                residual: s("0.0000001"),
                relaxation: s("0.75"),
            },
        )
        .unwrap();
        let thermal_storage = [s("1000"); 2];
        let thermal_boundary = [Boundary::default(); 2];
        let thermal_system = TransportSystem::new(
            grid,
            &thermal_storage,
            [s("1"); 3],
            &thermal_boundary,
            SolverLimits {
                iterations: 1000,
                residual: s("0.00001"),
            },
        )
        .unwrap();
        let moisture = [s(initial); 2];
        let temperature = [s("300"); 2];
        let mut moisture_trial = [Scalar::ZERO; 2];
        let mut temperature_trial = moisture_trial;
        let mut linear = moisture_trial;
        let mut heat_sources = moisture_trial;
        let mut nonlinear_trial = moisture_trial;
        let mut conductance = [[Scalar::ZERO; 3]; 2];
        let mut boundary = [Boundary::default(); 2];
        let mut work = VariableDryingWorkspace {
            drying: DryingWorkspace {
                moisture_trial: &mut moisture_trial,
                temperature_trial: &mut temperature_trial,
                linear: &mut linear,
                heat_sources: &mut heat_sources,
            },
            nonlinear_trial: &mut nonlinear_trial,
            cell_conductance: &mut conductance,
            moisture_boundary: &mut boundary,
        };
        let mut report = None;
        let allocations = allocation_counter::measure(|| {
            report = Some(
                trial_step(
                    &moisture_system,
                    &thermal_system,
                    s("1"),
                    s("1000"),
                    &moisture,
                    &temperature,
                    &mut work,
                )
                .unwrap(),
            );
        });
        assert_eq!(allocations.count_total, 0);
        let report = report.unwrap();
        assert_eq!(report.water_removed_kg > Scalar::ZERO, removes_water);
        assert_eq!(
            work.drying.temperature_trial[0] < temperature[0],
            removes_water
        );
        let energy_change = thermal_system
            .inventory(work.drying.temperature_trial)
            .unwrap()
            .checked_sub(thermal_system.inventory(&temperature).unwrap())
            .unwrap();
        let latent = report.water_removed_kg.checked_mul(s("1000")).unwrap();
        assert!(energy_change.checked_add(latent).unwrap().abs().unwrap() < s("0.0001"));
        let actual_loss = moisture
            .iter()
            .zip(work.drying.moisture_trial.iter())
            .try_fold(Scalar::ZERO, |total, (old, new)| {
                total.checked_add(old.checked_sub(*new)?)
            })
            .unwrap();
        assert!(
            actual_loss
                .checked_sub(report.water_removed_kg)
                .unwrap()
                .abs()
                .unwrap()
                < s("0.000001")
        );
        // The trial API never publishes either field, even if the thermal stage fails.
        assert_eq!(moisture, [s(initial); 2]);
        assert_eq!(temperature, [s("300"); 2]);
        let failed = trial_step(
            &moisture_system,
            &thermal_system,
            s("1"),
            s("1000000000"),
            &moisture,
            &temperature,
            &mut work,
        );
        if removes_water {
            assert!(matches!(
                failed,
                Err(PhysicsError::InvalidState) | Err(PhysicsError::InsufficientResolution)
            ));
        }
        assert_eq!(moisture, [s(initial); 2]);
        assert_eq!(temperature, [s("300"); 2]);
    }
}
