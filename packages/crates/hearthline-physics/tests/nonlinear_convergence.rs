use hearthline_physics::Scalar;
use hearthline_physics::material::PropertyTable;
use hearthline_physics::transport::nonlinear::{
    NonlinearDiffusion, NonlinearLimits, NonlinearWorkspace, SurfaceBoundary,
};
use hearthline_physics::transport::{Boundary, Grid3d, SolverLimits, TransportGrid};

fn s(value: &str) -> Scalar {
    value.parse().unwrap()
}

fn manufactured_error(n: usize) -> Scalar {
    let h = Scalar::ratio(1, n as i64).unwrap();
    let grid = Grid3d::new([n; 3], [h; 3]).unwrap();
    let volume = grid.volume().unwrap();
    let storage = vec![volume; grid.count()];
    let knots = [[s("0"), s("1")], [s("1"), s("2")]];
    let options = NonlinearLimits {
        // On this normalized mesh, k<=2 and h<=1/2 give diagonal<=12.125.
        // A billionth-resolution state cannot meet a one-billionth equation
        // residual for every row. Ten billionths exceed that rounding floor;
        // the outer budget remains far below the spatial-error acceptance.
        linear: SolverLimits {
            iterations: 1000,
            residual: s("0.00000001"),
        },
        iterations: 100,
        residual: s("0.0000001"),
        relaxation: s("0.75"),
    };
    let system = NonlinearDiffusion::new(
        TransportGrid::Dense(grid),
        &storage,
        grid.conductance(s("1")).unwrap(),
        PropertyTable::new(&knots).unwrap(),
        SurfaceBoundary::FixedValue(Scalar::ZERO),
        options,
    )
    .unwrap();
    let mut sources = vec![Scalar::ZERO; grid.count()];
    let mut exact = sources.clone();
    // u=16*x(1-x)*y(1-y)*z(1-z), k(u)=1+u; all surface values zero.
    // The independent source is -((1+u)*laplacian(u) + |gradient(u)|^2).
    for cell in 0..grid.count() {
        let positions = [cell % n, (cell / n) % n, cell / (n * n)]
            .map(|i| Scalar::ratio((2 * i + 1) as i64, (2 * n) as i64).unwrap());
        let terms = positions.map(|x| x.checked_mul(s("1").checked_sub(x).unwrap()).unwrap());
        exact[cell] = s("16")
            .checked_mul(terms[0])
            .unwrap()
            .checked_mul(terms[1])
            .unwrap()
            .checked_mul(terms[2])
            .unwrap();
        let mut negative_laplacian = Scalar::ZERO;
        let mut gradient_squared = Scalar::ZERO;
        for axis in 0..3 {
            let tangential = terms[(axis + 1) % 3]
                .checked_mul(terms[(axis + 2) % 3])
                .unwrap();
            negative_laplacian = negative_laplacian
                .checked_add(s("32").checked_mul(tangential).unwrap())
                .unwrap();
            let gradient = s("16")
                .checked_mul(
                    s("1")
                        .checked_sub(s("2").checked_mul(positions[axis]).unwrap())
                        .unwrap(),
                )
                .unwrap()
                .checked_mul(tangential)
                .unwrap();
            gradient_squared = gradient_squared
                .checked_add(gradient.checked_mul(gradient).unwrap())
                .unwrap();
        }
        sources[cell] = negative_laplacian
            .checked_mul(s("1").checked_add(exact[cell]).unwrap())
            .unwrap()
            .checked_sub(gradient_squared)
            .unwrap()
            .checked_mul(volume)
            .unwrap();
    }
    let mut state = vec![Scalar::ZERO; grid.count()];
    let mut trial = state.clone();
    let mut linear = state.clone();
    let mut coefficients = vec![[Scalar::ZERO; 3]; grid.count()];
    let mut boundary = vec![Boundary::default(); grid.count()];
    let mut work = NonlinearWorkspace {
        trial: &mut trial,
        linear: &mut linear,
        cell_conductance: &mut coefficients,
        boundary: &mut boundary,
    };
    for _ in 0..8 {
        system
            .solve(s("1"), &mut state, &sources, &mut work)
            .unwrap();
    }
    state
        .into_iter()
        .zip(exact)
        .map(|(actual, expected)| actual.checked_sub(expected).unwrap().abs().unwrap())
        .max()
        .unwrap()
}

#[test]
fn nonlinear_three_dimensional_manufactured_solution_converges_under_refinement() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let errors = [2, 4, 8].map(manufactured_error);
            assert!(
                errors[1] < errors[0].checked_mul(s("0.4")).unwrap(),
                "{errors:?}"
            );
            assert!(
                errors[2] < errors[1].checked_mul(s("0.4")).unwrap(),
                "{errors:?}"
            );
            assert!(errors[2] < s("0.006"), "{errors:?}");
        })
        .unwrap()
        .join()
        .unwrap();
}
