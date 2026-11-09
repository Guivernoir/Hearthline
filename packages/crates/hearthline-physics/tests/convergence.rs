use hearthline_physics::Scalar;
use hearthline_physics::transport::{Boundary, Grid3d, SolverLimits, TransportSystem};

fn s(value: &str) -> Scalar {
    value.parse().unwrap()
}

fn manufactured_poisson_error(n: usize) -> Scalar {
    let h = Scalar::ratio(1, n as i64).unwrap();
    let grid = Grid3d::new([n; 3], [h; 3]).unwrap();
    let storage = vec![grid.volume().unwrap(); grid.count()];
    let source = vec![grid.volume().unwrap().checked_mul(s("6")).unwrap(); grid.count()];
    let mut boundaries = vec![Boundary::default(); grid.count()];
    let mut exact = vec![Scalar::ZERO; grid.count()];
    for cell in 0..grid.count() {
        let coordinates = [cell % n, (cell / n) % n, cell / (n * n)];
        let mut terms = [Scalar::ZERO; 3];
        for axis in 0..3 {
            let x = Scalar::ratio(2 * coordinates[axis] as i64 + 1, 2 * n as i64).unwrap();
            terms[axis] = x.checked_mul(Scalar::ONE.checked_sub(x).unwrap()).unwrap();
            exact[cell] = exact[cell].checked_add(terms[axis]).unwrap();
        }
        let mut faces = 0;
        let mut sum = Scalar::ZERO;
        for axis in 0..3 {
            if coordinates[axis] == 0 || coordinates[axis] + 1 == n {
                faces += 1;
                sum = sum
                    .checked_add(exact[cell].checked_sub(terms[axis]).unwrap())
                    .unwrap();
            }
        }
        if faces > 0 {
            boundaries[cell] = Boundary {
                conductance: h.checked_mul(Scalar::integer(2 * faces).unwrap()).unwrap(),
                ambient: sum.checked_div(Scalar::integer(faces).unwrap()).unwrap(),
            };
        }
    }
    let system = TransportSystem::new(
        grid,
        &storage,
        grid.conductance(Scalar::ONE).unwrap(),
        &boundaries,
        SolverLimits {
            iterations: 1000,
            residual: s("0.00000001"),
        },
    )
    .unwrap();
    let mut field = vec![Scalar::ZERO; grid.count()];
    let mut work = field.clone();
    for _ in 0..15 {
        system
            .solve(Scalar::ONE, &mut field, &source, &mut work)
            .unwrap();
    }
    field
        .iter()
        .zip(exact)
        .map(|(a, b)| a.checked_sub(b).unwrap().abs().unwrap())
        .max()
        .unwrap()
}

#[test]
fn three_dimensional_manufactured_solution_converges_quadratically_in_space() {
    let coarse = manufactured_poisson_error(2);
    let medium = manufactured_poisson_error(4);
    let fine = manufactured_poisson_error(8);
    assert!(
        medium < coarse.checked_mul(s("0.3")).unwrap(),
        "{coarse:?}, {medium:?}"
    );
    assert!(
        fine < medium.checked_mul(s("0.3")).unwrap(),
        "{medium:?}, {fine:?}"
    );
    assert!(fine < s("0.02"));
}
