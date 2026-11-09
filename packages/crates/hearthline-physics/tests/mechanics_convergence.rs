use std::collections::BTreeMap;

use hearthline_physics::Scalar;
use hearthline_physics::mechanics::{
    IsotropicElasticity, MechanicalSystem, MechanicsWorkspace, StiffnessMatrix, Tetrahedron,
};
use hearthline_physics::transport::SolverLimits;

fn exact(x: Scalar) -> Scalar {
    x.checked_mul(Scalar::ONE.checked_sub(x).unwrap()).unwrap()
}

fn errors(divisions: usize) -> [Scalar; 2] {
    let width = divisions + 1;
    let index = |x, y, z| x + width * (y + width * z);
    let mut positions = Vec::new();
    for z in 0..width {
        for y in 0..width {
            for x in 0..width {
                positions
                    .push([x, y, z].map(|n| Scalar::ratio(n as i64, divisions as i64).unwrap()));
            }
        }
    }
    let mut elements = Vec::new();
    for z in 0..divisions {
        for y in 0..divisions {
            for x in 0..divisions {
                for axes in [
                    [0, 1, 2],
                    [0, 2, 1],
                    [1, 0, 2],
                    [1, 2, 0],
                    [2, 0, 1],
                    [2, 1, 0],
                ] {
                    let mut xyz = [x, y, z];
                    let mut nodes = [index(x, y, z); 4];
                    for (corner, axis) in axes.into_iter().enumerate() {
                        xyz[axis] += 1;
                        nodes[corner + 1] = index(xyz[0], xyz[1], xyz[2]);
                    }
                    // Odd coordinate permutations reverse the reference tetrahedron.
                    let odd = (axes[0] > axes[1]) ^ (axes[0] > axes[2]) ^ (axes[1] > axes[2]);
                    if odd {
                        nodes.swap(1, 2);
                    }
                    elements.push(Tetrahedron::compile(nodes, &positions).unwrap());
                }
            }
        }
    }
    let dofs = positions.len() * 3;
    let material = IsotropicElasticity::new(Scalar::ONE, Scalar::ratio(1, 4).unwrap()).unwrap();
    let materials = vec![material; elements.len()];
    let mut constraints = vec![None; dofs];
    for (node, position) in positions.iter().enumerate() {
        if position
            .iter()
            .any(|v| *v == Scalar::ZERO || *v == Scalar::ONE)
        {
            constraints[3 * node..3 * node + 3].copy_from_slice(&[
                Some(exact(position[0])),
                Some(Scalar::ZERO),
                Some(Scalar::ZERO),
            ]);
        }
    }
    // Unit cube, E=1, nu=1/4: lambda=mu=0.4. For u=(x(1-x),0,0),
    // -div(sigma)=(2*(lambda+2*mu),0,0)=(2.4,0,0).
    // A constant body load integrates exactly as f*V/4 per tetrahedron node.
    let mut load = vec![Scalar::ZERO; dofs];
    let mut entries = BTreeMap::<(usize, usize), Scalar>::new();
    for element in &elements {
        let force = element
            .volume()
            .checked_mul(Scalar::ratio(3, 5).unwrap())
            .unwrap();
        for node in element.nodes() {
            load[3 * node] = load[3 * node].checked_add(force).unwrap();
        }
        for row in 0..12 {
            for column in 0..12 {
                let global = (
                    3 * element.nodes()[row / 3] + row % 3,
                    3 * element.nodes()[column / 3] + column % 3,
                );
                let entry = entries.entry(global).or_default();
                *entry = entry
                    .checked_add(element.stiffness(material, row, column).unwrap())
                    .unwrap();
            }
        }
    }
    let total = load
        .iter()
        .try_fold(Scalar::ZERO, |sum, value| sum.checked_add(*value))
        .unwrap();
    assert!(
        total
            .checked_sub(Scalar::ratio(12, 5).unwrap())
            .unwrap()
            .abs()
            .unwrap()
            .raw()
            < 10_000
    );
    let mut rows = vec![0; dofs + 1];
    let mut columns = Vec::new();
    let mut values = Vec::new();
    for ((row, column), value) in entries {
        rows[row + 1] += 1;
        columns.push(column as u32);
        values.push(value);
    }
    for i in 1..rows.len() {
        rows[i] += rows[i - 1];
    }
    let matrix = StiffnessMatrix::new(&rows, &columns, &values).unwrap();
    let limits = SolverLimits {
        iterations: 500,
        residual: Scalar::from_raw(100),
    };
    let system = MechanicalSystem::new(&elements, &materials, &constraints, limits)
        .unwrap()
        .with_stiffness(matrix)
        .unwrap()
        .with_symmetric_sweep()
        .unwrap();
    let mut displacement = vec![Scalar::ZERO; dofs];
    let [
        trial,
        residual,
        preconditioned,
        direction,
        product,
        diagonal,
    ] = &mut core::array::from_fn::<_, 6, _>(|_| vec![Scalar::ZERO; dofs]);
    let mut work = MechanicsWorkspace {
        trial,
        residual,
        preconditioned,
        direction,
        product,
        diagonal,
    };
    let measured = allocation_counter::measure(|| {
        let report = system
            .solve_loaded(&mut displacement, &load, &mut work)
            .unwrap();
        assert!(report.residual <= limits.residual);
    });
    assert_eq!(measured.count_total, 0);
    let mut error = [Scalar::ZERO; 2];
    for element in &elements {
        let mut centroid = Scalar::ZERO;
        let mut interpolated = [Scalar::ZERO; 3];
        for node in element.nodes() {
            centroid = centroid.checked_add(positions[node][0]).unwrap();
            for (axis, value) in interpolated.iter_mut().enumerate() {
                *value = value.checked_add(displacement[3 * node + axis]).unwrap();
            }
        }
        centroid = centroid.checked_div(Scalar::integer(4).unwrap()).unwrap();
        for (axis, value) in interpolated.into_iter().enumerate() {
            let expected = if axis == 0 {
                exact(centroid)
            } else {
                Scalar::ZERO
            };
            error[0] = error[0].max(
                value
                    .checked_div(Scalar::integer(4).unwrap())
                    .unwrap()
                    .checked_sub(expected)
                    .unwrap()
                    .abs()
                    .unwrap(),
            );
        }
        let exact_strain = Scalar::ONE
            .checked_sub(Scalar::integer(2).unwrap().checked_mul(centroid).unwrap())
            .unwrap();
        for (row, values) in element
            .strain(&displacement)
            .unwrap()
            .into_iter()
            .enumerate()
        {
            for (column, value) in values.into_iter().enumerate() {
                let expected = if row == 0 && column == 0 {
                    exact_strain
                } else {
                    Scalar::ZERO
                };
                error[1] = error[1].max(value.checked_sub(expected).unwrap().abs().unwrap());
            }
        }
    }
    error
}

#[test]
fn manufactured_three_dimensional_elasticity_has_second_order_displacement_and_first_order_strain()
{
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let errors = [2, 4, 8].map(errors);
            for pair in errors.windows(2) {
                for (field, expected) in [4, 2].into_iter().enumerate() {
                    let ratio = pair[0][field].checked_div(pair[1][field]).unwrap();
                    assert!(
                        ratio
                            .checked_sub(Scalar::integer(expected).unwrap())
                            .unwrap()
                            .abs()
                            .unwrap()
                            < Scalar::ratio(1, 100).unwrap(),
                        "errors {errors:?}"
                    );
                }
            }
            assert!(errors[2][0] < Scalar::ratio(1, 250).unwrap());
            assert!(errors[2][1] < Scalar::ratio(1, 15).unwrap());
        })
        .unwrap()
        .join()
        .unwrap();
}
