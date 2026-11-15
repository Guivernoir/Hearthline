use hearthline_physics::Scalar;
use hearthline_physics::flow::connected::{
    CellInventory, ConnectedAdvanceWorkspace, ConnectedFlow, ConnectedRetryPolicy,
    ConnectedWorkspace, FaceLink, FlowCell, FlowMesh,
};
use hearthline_physics::flow::{FlowBoundary, IdealGas, Primitive, ViscousTransport};
use hearthline_physics::transport::SolverLimits;

pub fn exercise(input: &[u8]) {
    let Some(bytes) = input.get(..12) else {
        return;
    };
    let gas = IdealGas::new(Scalar::ratio(14, 10).unwrap()).unwrap();
    let axis = usize::from(bytes[0] % 3);
    let widths = [
        Scalar::ratio(i64::from(bytes[1]) + 10, 100).unwrap(),
        Scalar::ratio(i64::from(bytes[2]) + 10, 100).unwrap(),
    ];
    let mut cells = core::array::from_fn::<_, 2, _>(|i| {
        let mut faces = [FaceLink::Boundary(0); 6];
        faces[2 * axis + (i ^ 1)] = FaceLink::Cell((i ^ 1) as u32);
        let mut spacing = [Scalar::ONE; 3];
        spacing[axis] = widths[i];
        FlowCell::new(spacing, faces).unwrap()
    });
    if bytes[3] & 1 != 0 {
        cells[0].faces[usize::from(bytes[4] % 6)] = if bytes[5] & 1 != 0 {
            FaceLink::Cell(u32::from(bytes[6]))
        } else {
            FaceLink::Boundary(u32::from(bytes[6]))
        };
    }
    let boundaries = [FlowBoundary::AdiabaticSlipWall];
    let Ok(mesh) = FlowMesh::new(&cells, &boundaries) else {
        return;
    };
    let transport = (bytes[7] & 1 != 0).then(|| {
        ViscousTransport::new(
            Scalar::ratio(i64::from(bytes[8]), 1000).unwrap(),
            Scalar::ratio(i64::from(bytes[9]), 1000).unwrap(),
            Scalar::ONE,
        )
        .unwrap()
    });
    let limits = SolverLimits {
        iterations: 24,
        residual: Scalar::from_raw(50),
    };
    let solver = ConnectedFlow::new(mesh, gas, limits, transport).unwrap();
    let mut state = core::array::from_fn::<_, 2, _>(|i| {
        let pressure = Scalar::ratio(i64::from(bytes[10 + i]) + 50, 100).unwrap();
        let density = if bytes[7] & 2 == 0 {
            pressure
        } else {
            Scalar::ONE
        };
        CellInventory::from_density(
            gas.conserved(Primitive {
                pressure,
                density,
                velocity: [Scalar::ZERO; 3],
            })
            .unwrap(),
            cells[i].volume(),
        )
        .unwrap()
    });
    let original = state;
    let mut trial = [CellInventory::default(); 2];
    let mut next = trial;
    let dt = Scalar::ratio(i64::from(bytes[6]) + 1, 10000).unwrap();
    match solver.solve(
        dt,
        &mut state,
        &mut ConnectedWorkspace {
            trial: &mut trial,
            next: &mut next,
        },
    ) {
        Err(_) => assert_eq!(state, original),
        Ok(report) => {
            assert_eq!(report.balance_error, CellInventory::default());
            assert!(report.residual <= limits.residual);
            let before = solver.inventory(&original).unwrap();
            let after = solver.inventory(&state).unwrap();
            assert_eq!(before.mass, after.mass);
            assert_eq!(before.energy, after.energy);
            for (cell, value) in cells.iter().zip(state) {
                assert!(gas.primitive(value.density(cell.volume()).unwrap()).is_ok());
            }
        }
    }
    let mut candidate = original;
    if let Ok(report) = solver.advance(
        dt,
        ConnectedRetryPolicy {
            maximum_substeps: 16,
            minimum_step: Scalar::from_raw(1),
        },
        &original,
        &mut ConnectedAdvanceWorkspace {
            candidate: &mut candidate,
            trial: &mut trial,
            next: &mut next,
        },
    ) {
        assert_eq!(report.completed, dt);
        let before = solver.inventory(&original).unwrap();
        let after = solver.inventory(&candidate).unwrap();
        for ((old, new), exchange) in before
            .components()
            .into_iter()
            .zip(after.components())
            .zip(report.boundary_exchange.components())
        {
            assert_eq!(new.checked_sub(old).unwrap(), exchange);
        }
        let mut explicit = original;
        let floor = dt.raw() / i64::from(report.substeps);
        let remainder = dt.raw() % i64::from(report.substeps);
        for index in 0..report.substeps {
            let substep = Scalar::from_raw(floor + i64::from(i64::from(index) < remainder));
            solver
                .solve(
                    substep,
                    &mut explicit,
                    &mut ConnectedWorkspace {
                        trial: &mut trial,
                        next: &mut next,
                    },
                )
                .unwrap();
        }
        assert_eq!(candidate, explicit);
    }
}
