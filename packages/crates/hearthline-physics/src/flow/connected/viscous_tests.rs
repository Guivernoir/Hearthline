use super::*;
use crate::flow::connected::{FlowCell, FlowMesh};
use crate::flow::{IdealGas, Primitive};
use crate::transport::SolverLimits;

fn s(value: &str) -> Scalar {
    value.parse().unwrap()
}

#[test]
fn unequal_spacing_reproduces_linear_shear_gradient_and_face_work_on_every_axis() {
    let gas = IdealGas::new(s("1.4")).unwrap();
    let widths = [s("0.5"), s("1"), s("1.5")];
    let centers = [s("0.25"), s("1"), s("2.25")];
    let transport = ViscousTransport::new(Scalar::ONE, Scalar::ZERO, Scalar::ONE).unwrap();
    for axis in 0..3 {
        let component = (axis + 1) % 3;
        let primitive = |coordinate: Scalar| {
            let mut velocity = [Scalar::ZERO; 3];
            velocity[component] = coordinate.checked_mul(s("2")).unwrap();
            Primitive {
                density: Scalar::ONE,
                pressure: Scalar::ONE,
                velocity,
            }
        };
        let boundaries = [
            FlowBoundary::AdiabaticSlipWall,
            FlowBoundary::Reservoir(primitive(s("-0.25"))),
            FlowBoundary::Reservoir(primitive(s("3.75"))),
        ];
        let cells = core::array::from_fn::<_, 3, _>(|i| {
            let mut spacing = [Scalar::ONE; 3];
            spacing[axis] = widths[i];
            let mut faces = [FaceLink::Boundary(0); 6];
            faces[2 * axis] = if i == 0 {
                FaceLink::Boundary(1)
            } else {
                FaceLink::Cell(i as u32 - 1)
            };
            faces[2 * axis + 1] = if i == 2 {
                FaceLink::Boundary(2)
            } else {
                FaceLink::Cell(i as u32 + 1)
            };
            FlowCell::new(spacing, faces).unwrap()
        });
        let state = core::array::from_fn::<_, 3, _>(|i| {
            CellInventory::from_density(
                gas.conserved(primitive(centers[i])).unwrap(),
                cells[i].volume(),
            )
            .unwrap()
        });
        let mesh = FlowMesh::new(&cells, &boundaries).unwrap();
        let solver = ConnectedFlow::new(
            mesh,
            gas,
            SolverLimits {
                iterations: 10,
                residual: s("0.000001"),
            },
            Some(transport),
        )
        .unwrap();
        let mut expected = [[Scalar::ZERO; 3]; 3];
        expected[component][axis] = s("2");
        for cell in 0..3 {
            assert_eq!(solver.gradient(&state, cell).unwrap(), expected);
        }
        for (cell, work) in [(0, s("2")), (1, s("6"))] {
            let flux = solver
                .viscous_flux(&state, cell, 2 * axis + 1, transport)
                .unwrap();
            assert_eq!(flux.density, Scalar::ZERO);
            assert_eq!(flux.momentum[component], s("2"));
            assert_eq!(flux.total_energy, work);
        }
    }
}
