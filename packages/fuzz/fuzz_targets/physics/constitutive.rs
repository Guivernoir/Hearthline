use hearthline_physics::Scalar;
use hearthline_physics::material::PropertyTable;
use hearthline_physics::transport::nonlinear::{
    NonlinearDiffusion, NonlinearLimits, NonlinearWorkspace, SurfaceBoundary,
};
use hearthline_physics::transport::{
    Boundary, Grid3d, SolverLimits, TransportGrid, harmonic_conductance, surface_conductance,
};

pub fn exercise(input: &[u8]) {
    let byte = |i| input.get(i).copied().unwrap_or(0);
    if input.len() >= 24 {
        let value = |start| {
            Scalar::from_raw(i64::from_le_bytes(
                input[start..start + 8].try_into().unwrap(),
            ))
        };
        let (a, b, c) = (value(0), value(8), value(16));
        let _ = a.checked_mul_div(b, c);
        let _ = harmonic_conductance(a, b);
        let _ = surface_conductance(a, b);
        let knots = [[a, b], [c, a]];
        if let Ok(table) = PropertyTable::new(&knots) {
            let _ = table.value(b);
        }
    }
    let grid = Grid3d::new([2, 1, 1], [Scalar::ONE; 3]).unwrap();
    let knots = [
        [
            Scalar::ZERO,
            Scalar::ratio(i64::from(byte(0)), 100).unwrap(),
        ],
        [Scalar::ONE, Scalar::ratio(i64::from(byte(1)), 100).unwrap()],
    ];
    let table = PropertyTable::new(&knots).unwrap();
    let limits = NonlinearLimits {
        linear: SolverLimits {
            iterations: 12,
            residual: Scalar::from_raw(10),
        },
        iterations: 6,
        residual: Scalar::from_raw(100),
        relaxation: Scalar::ratio(i64::from(byte(2)) + 1, 256).unwrap(),
    };
    let system = NonlinearDiffusion::new(
        TransportGrid::Dense(grid),
        &[Scalar::ONE; 2],
        [Scalar::ONE; 3],
        table,
        SurfaceBoundary::Insulated,
        limits,
    )
    .unwrap();
    let mut state = [
        Scalar::ratio(i64::from(byte(3)), 255).unwrap(),
        Scalar::ratio(i64::from(byte(4)), 255).unwrap(),
    ];
    let old = state;
    let mut trial = [Scalar::ZERO; 2];
    let mut linear = trial;
    let mut coefficients = [[Scalar::ZERO; 3]; 2];
    let mut boundary = [Boundary::default(); 2];
    let mut work = NonlinearWorkspace {
        trial: &mut trial,
        linear: &mut linear,
        cell_conductance: &mut coefficients,
        boundary: &mut boundary,
    };
    match system.solve(
        Scalar::ratio(i64::from(byte(5)) + 1, 100).unwrap(),
        &mut state,
        &[Scalar::ZERO; 2],
        &mut work,
    ) {
        Ok(report) => {
            assert!(report.transport.residual <= limits.residual);
            assert_eq!(report.transport.exchange, Scalar::ZERO);
            let change = state[0]
                .checked_add(state[1])
                .unwrap()
                .checked_sub(old[0].checked_add(old[1]).unwrap())
                .unwrap();
            assert_eq!(change, report.transport.balance_error);
            for value in state {
                assert!(table.value(value).is_ok());
            }
        }
        Err(_) => assert_eq!(state, old),
    }
}
