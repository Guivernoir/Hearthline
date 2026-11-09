use hearthline_physics::Scalar;
use hearthline_physics::material::caloric::{CaloricKnot, CaloricLaw};
use hearthline_physics::transport::enthalpy::{
    EnthalpyLimits, EnthalpySystem, EnthalpyWorkspace, WaterTransport,
};
use hearthline_physics::transport::{Boundary, Grid3d, TransportGrid};

fn s(value: &str) -> Scalar {
    value.parse().unwrap()
}

fn spatial_error(n: usize, wet: bool) -> Scalar {
    let spacing = Scalar::ratio(1, n as i64).unwrap();
    let grid = Grid3d::new([n; 3], [spacing; 3]).unwrap();
    let volume = grid.volume().unwrap();
    let mass = vec![volume; grid.count()];
    let mut caloric = [CaloricKnot::default(); 2];
    let law = CaloricLaw::compile(
        &[[s("0"), s("1")], [s("1"), s("2")]],
        Scalar::ZERO,
        &mut caloric,
    )
    .unwrap();
    let mut exact = vec![Scalar::ZERO; grid.count()];
    let mut source = exact.clone();
    let mut boundary = vec![Boundary::default(); grid.count()];
    // Steady T=16*x(1-x)*y(1-y)*z(1-z), k=1, cp(T)=1+T.
    // The manufactured source is -laplacian(T); cp affects the transient,
    // not the independent steady solution or its spatial discretization error.
    for cell in 0..grid.count() {
        let positions = [cell % n, cell / n % n, cell / (n * n)]
            .map(|index| Scalar::ratio((2 * index + 1) as i64, (2 * n) as i64).unwrap());
        let terms = positions.map(|x| x.checked_mul(s("1").checked_sub(x).unwrap()).unwrap());
        exact[cell] = s("16")
            .checked_mul(terms[0])
            .unwrap()
            .checked_mul(terms[1])
            .unwrap()
            .checked_mul(terms[2])
            .unwrap();
        let mut forcing = Scalar::ZERO;
        for axis in 0..3 {
            forcing = forcing
                .checked_add(
                    s("32")
                        .checked_mul(terms[(axis + 1) % 3])
                        .unwrap()
                        .checked_mul(terms[(axis + 2) % 3])
                        .unwrap(),
                )
                .unwrap();
        }
        source[cell] = forcing.checked_mul(volume).unwrap();
        for face in 0..6 {
            if grid.face_neighbor(cell, face).unwrap().is_none() {
                boundary[cell].conductance = boundary[cell]
                    .conductance
                    .checked_add(s("2").checked_mul(spacing).unwrap())
                    .unwrap();
            }
        }
    }
    let system = EnthalpySystem::new(
        TransportGrid::Dense(grid),
        &mass,
        law,
        grid.conductance(s("1")).unwrap(),
        &boundary,
        EnthalpyLimits {
            sweeps: 1000,
            local_iterations: 64,
            residual: s("0.00000002"),
        },
    )
    .unwrap();
    let mut state = vec![Scalar::ZERO; grid.count()];
    let moisture = vec![s("0.5"); grid.count()];
    let coefficients = vec![[Scalar::ZERO; 3]; grid.count()];
    let moisture_boundary = vec![Boundary::default(); grid.count()];
    let mut liquid_knots = [CaloricKnot::default(); 2];
    let liquid = CaloricLaw::compile(
        &[[s("0"), s("2")], [s("1"), s("3")]],
        Scalar::ZERO,
        &mut liquid_knots,
    )
    .unwrap();
    let system = if wet {
        system
            .with_water(WaterTransport {
                liquid,
                previous_moisture: &moisture,
                current_moisture: &moisture,
                conductance: &coefficients,
                boundary: &moisture_boundary,
                inlet_temperature: Scalar::ZERO,
            })
            .unwrap()
    } else {
        system
    };
    let mut trial = state.clone();
    let mut old_enthalpy = state.clone();
    let mut work = EnthalpyWorkspace {
        trial: &mut trial,
        old_enthalpy: &mut old_enthalpy,
    };
    let measured = allocation_counter::measure(|| {
        for _ in 0..8 {
            system
                .solve(s("1"), &mut state, &source, &mut work)
                .unwrap();
        }
    });
    assert_eq!(measured.count_total, 0);
    state
        .into_iter()
        .zip(exact)
        .map(|(a, b)| a.checked_sub(b).unwrap().abs().unwrap())
        .max()
        .unwrap()
}

#[test]
fn three_dimensional_enthalpy_transport_converges_under_spatial_refinement_on_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            for wet in [false, true] {
                let error = [2, 4, 8].map(|n| spatial_error(n, wet));
                assert!(
                    error[1] < error[0].checked_mul(s("0.4")).unwrap(),
                    "{error:?}"
                );
                assert!(
                    error[2] < error[1].checked_mul(s("0.4")).unwrap(),
                    "{error:?}"
                );
                assert!(error[2] < s("0.006"), "{error:?}");
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
