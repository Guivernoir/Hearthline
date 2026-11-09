use hearthline_physics::Scalar;
use hearthline_physics::material::caloric::{CaloricKnot, CaloricLaw};
use hearthline_physics::transport::enthalpy::{
    EnthalpyLimits, EnthalpySystem, EnthalpyWorkspace, WaterTransport,
};
use hearthline_physics::transport::{Boundary, Grid3d, TransportGrid};

pub fn exercise(input: &[u8]) {
    if input.len() >= 32 {
        let raw = |i| Scalar::from_raw(i64::from_le_bytes(input[i..i + 8].try_into().unwrap()));
        let points = [[raw(0), raw(8)], [raw(16), raw(24)]];
        let mut knots = [CaloricKnot::default(); 2];
        if let Ok(law) = CaloricLaw::compile(&points, Scalar::ZERO, &mut knots) {
            let _ = law.enthalpy(raw(8));
            let _ = law.heat_capacity(raw(24));
        }
    }
    let byte = |i| i64::from(input.get(i).copied().unwrap_or(0));
    let points = [
        [Scalar::ZERO, Scalar::ratio(byte(0) + 1, 64).unwrap()],
        [Scalar::ONE, Scalar::ratio(byte(1) + 1, 64).unwrap()],
    ];
    let mut knots = [CaloricKnot::default(); 2];
    let law = CaloricLaw::compile(&points, Scalar::ZERO, &mut knots).unwrap();
    let grid = Grid3d::new([2, 1, 1], [Scalar::ONE; 3]).unwrap();
    let limits = EnthalpyLimits {
        sweeps: 8,
        local_iterations: 16,
        residual: Scalar::from_raw(100),
    };
    let mass = [Scalar::ONE; 2];
    let boundary = [Boundary::default(); 2];
    let system = EnthalpySystem::new(
        TransportGrid::Dense(grid),
        &mass,
        law,
        [Scalar::ONE; 3],
        &boundary,
        limits,
    )
    .unwrap();
    let wet = byte(5) & 1 != 0;
    let previous_moisture = [Scalar::ONE, Scalar::ZERO];
    let current_moisture = [Scalar::ratio(3, 4).unwrap(), Scalar::ratio(1, 4).unwrap()];
    let conductance = [[Scalar::ONE; 3]; 2];
    let system = if wet {
        system
            .with_water(WaterTransport {
                liquid: law,
                previous_moisture: &previous_moisture,
                current_moisture: &current_moisture,
                conductance: &conductance,
                boundary: &boundary,
                inlet_temperature: Scalar::ZERO,
            })
            .unwrap()
    } else {
        system
    };
    let mut temperature = [
        Scalar::ratio(byte(2), 255).unwrap(),
        Scalar::ratio(byte(3), 255).unwrap(),
    ];
    let old = temperature;
    let mut trial = [Scalar::ZERO; 2];
    let mut old_enthalpy = trial;
    let result = system.solve(
        if wet {
            Scalar::ratio(1, 2).unwrap()
        } else {
            Scalar::ratio(byte(4) + 1, 100).unwrap()
        },
        &mut temperature,
        &[Scalar::ZERO; 2],
        &mut EnthalpyWorkspace {
            trial: &mut trial,
            old_enthalpy: &mut old_enthalpy,
        },
    );
    match result {
        Err(_) => assert_eq!(temperature, old),
        Ok(report) => {
            assert!(report.transport.residual <= limits.residual);
            assert_eq!(report.transport.exchange, Scalar::ZERO);
            let difference = system
                .inventory(&temperature)
                .unwrap()
                .checked_sub(system.previous_inventory(&old).unwrap())
                .unwrap();
            assert_eq!(difference, report.transport.balance_error);
            // Two true cell residuals, with two raw units per cell reserved
            // for separately rounded inventory products, bound the ledger.
            let bound = limits
                .residual
                .checked_add(Scalar::from_raw(2))
                .unwrap()
                .checked_mul(Scalar::integer(2).unwrap())
                .unwrap();
            assert!(difference.abs().unwrap() <= bound);
        }
    }
}
