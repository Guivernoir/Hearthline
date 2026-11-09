#![no_main]

use hearthline_physics::Scalar;
use hearthline_physics::flow::{
    CompressibleFlow, Conserved, FlowBoundary, FlowWorkspace, GasScales, IdealGas, Primitive,
    ViscousTransport,
};
use hearthline_physics::mechanics::{IsotropicElasticity, StiffnessMatrix, Tetrahedron};
use hearthline_physics::transport::{Grid3d, SolverLimits};
use libfuzzer_sys::fuzz_target;

#[path = "physics/caloric.rs"]
mod caloric;
#[path = "physics/capacity.rs"]
mod capacity;
#[path = "physics/constitutive.rs"]
mod constitutive;
#[path = "physics/geometry.rs"]
mod geometry;
#[path = "physics/material.rs"]
mod material;
#[path = "physics/mechanics.rs"]
mod mechanics;

fuzz_target!(|input: &[u8]| {
    geometry::exercise(input);
    constitutive::exercise(input);
    caloric::exercise(input);
    material::exercise(input);
    capacity::exercise(input);
    mechanics::exercise(input);
    if let Ok(text) = core::str::from_utf8(&input[..input.len().min(64)]) {
        let _ = text.parse::<Scalar>();
    }
    let Some(bytes) = input.get(..24) else {
        return;
    };
    let a = Scalar::from_raw(i64::from_le_bytes(bytes[0..8].try_into().unwrap()));
    let b = Scalar::from_raw(i64::from_le_bytes(bytes[8..16].try_into().unwrap()));
    let _ = a.checked_add(b);
    let _ = a.checked_sub(b);
    let _ = a.checked_mul(b);
    let _ = a.checked_div(b);
    let _ = a.sqrt();
    let rows = if bytes[22] & 2 == 0 {
        [0, 2, 4]
    } else {
        [0, u32::from(bytes[20]), u32::from(bytes[21])]
    };
    let columns = if bytes[22] & 2 == 0 {
        [0, 1, 0, 1]
    } else {
        [
            u32::from(bytes[16]),
            u32::from(bytes[17]),
            u32::from(bytes[18]),
            u32::from(bytes[19]),
        ]
    };
    let values = [a, b, b, a];
    if let Ok(matrix) = StiffnessMatrix::new(&rows, &columns, &values) {
        let _ = matrix.apply(&[a, b], &mut [Scalar::ZERO; 2]);
    }
    if let Ok(scales) = GasScales::new(a, b, Scalar::ONE, Scalar::ONE) {
        let _ = scales.pressure_to_solver(a);
        let _ = scales.temperature_to_si(b);
        let _ = scales.conductivity_to_solver(a);
    }
    let positions = [
        [a, b, Scalar::ZERO],
        [Scalar::ONE, Scalar::ZERO, Scalar::ZERO],
        [Scalar::ZERO, Scalar::ONE, Scalar::ZERO],
        [Scalar::ZERO, Scalar::ZERO, Scalar::ONE],
    ];
    if let Ok(element) = Tetrahedron::compile([0, 1, 2, 3], &positions) {
        assert!(element.volume() > Scalar::ZERO);
        let _ = element.strain(&[a; 12]);
        if let Ok(material) = IsotropicElasticity::new(a, b) {
            let _ = element.stiffness(material, usize::from(bytes[20]), usize::from(bytes[21]));
        }
    }
    let gamma = Scalar::ratio(14, 10).unwrap();
    let gas = IdealGas::new(gamma).unwrap();
    let bounded = bytes[23] & 1 == 0;
    let initial = Primitive {
        density: if bounded {
            Scalar::ratio(i64::from(bytes[17]) + 1, 100).unwrap()
        } else {
            a
        },
        velocity: [Scalar::ZERO; 3],
        pressure: if bounded {
            Scalar::ratio(i64::from(bytes[18]) + 1, 100).unwrap()
        } else {
            b
        },
    };
    let Ok(left) = gas.conserved(initial) else {
        return;
    };
    let right = gas
        .conserved(Primitive {
            density: Scalar::ONE,
            velocity: [Scalar::ZERO; 3],
            pressure: Scalar::ONE,
        })
        .unwrap();
    let grid = Grid3d::new([2, 1, 1], [Scalar::ONE; 3]).unwrap();
    let limits = SolverLimits {
        iterations: 8,
        residual: Scalar::from_raw(10),
    };
    let solver = if bytes[22] & 1 == 0 {
        CompressibleFlow::new(grid, gas, [FlowBoundary::AdiabaticSlipWall; 6], limits)
    } else {
        let transport = ViscousTransport::new(
            Scalar::ratio(i64::from(bytes[20]), 1000).unwrap(),
            Scalar::ratio(i64::from(bytes[21]), 1000).unwrap(),
            Scalar::ONE,
        )
        .unwrap();
        CompressibleFlow::new_viscous(
            grid,
            gas,
            [FlowBoundary::NoSlipWall {
                velocity: [Scalar::ZERO; 3],
                temperature: None,
            }; 6],
            limits,
            transport,
        )
    }
    .unwrap();
    let mut state = [left, right];
    let original = state;
    let mut trial = [Conserved::default(); 2];
    let mut next = trial;
    let mut workspace = FlowWorkspace {
        trial: &mut trial,
        next: &mut next,
    };
    let dt = Scalar::ratio(i64::from(bytes[16]) + 1, 1000).unwrap();
    if solver.solve(dt, &mut state, &mut workspace).is_err() {
        assert_eq!(state, original);
    } else {
        for value in state {
            assert!(gas.primitive(value).is_ok());
        }
    }
});
