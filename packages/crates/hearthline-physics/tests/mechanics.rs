use hearthline_physics::mechanics::{
    IsotropicElasticity, MechanicalSystem, MechanicsWorkspace, Tetrahedron, Vector3,
};
use hearthline_physics::transport::SolverLimits;
use hearthline_physics::{PhysicsError, Scalar};

fn s(value: &str) -> Scalar {
    value.parse().unwrap()
}
fn cube() -> ([Vector3; 8], [Tetrahedron; 6]) {
    let positions = core::array::from_fn(|i| {
        [
            Scalar::integer((i % 2) as i64).unwrap(),
            Scalar::integer(((i / 2) % 2) as i64).unwrap(),
            Scalar::integer((i / 4) as i64).unwrap(),
        ]
    });
    let elements = [
        [0, 1, 3, 7],
        [0, 3, 2, 7],
        [0, 2, 6, 7],
        [0, 6, 4, 7],
        [0, 4, 5, 7],
        [0, 5, 1, 7],
    ]
    .map(|nodes| Tetrahedron::compile(nodes, &positions).unwrap());
    (positions, elements)
}
fn limits() -> SolverLimits {
    SolverLimits {
        iterations: 200,
        residual: s("0.000001"),
    }
}
fn restraints() -> [Option<Scalar>; 24] {
    let mut restraints = [None; 24];
    for index in [0, 1, 2, 4, 5, 8] {
        restraints[index] = Some(Scalar::ZERO);
    }
    restraints
}

#[test]
fn tetrahedra_reproduce_affine_strain_and_reject_inverted_or_flat_geometry() {
    let (positions, elements) = cube();
    let displacement: Vec<_> = positions
        .iter()
        .flat_map(|p| p.map(|x| x.checked_mul(s("0.01")).unwrap()))
        .collect();
    for element in &elements {
        let strain = element.strain(&displacement).unwrap();
        assert_eq!(
            strain,
            [
                [s("0.01"), Scalar::ZERO, Scalar::ZERO],
                [Scalar::ZERO, s("0.01"), Scalar::ZERO],
                [Scalar::ZERO, Scalar::ZERO, s("0.01")]
            ]
        );
        assert_eq!(element.volume(), s("0.166666667"));
    }
    assert!(Tetrahedron::compile([0, 1, 2, 3], &positions).is_err());
    assert!(Tetrahedron::compile([0, 3, 1, 7], &positions).is_err());
    assert!(Tetrahedron::compile([0, 1, 3, 8], &positions).is_err());
    assert!(elements[0].strain(&[]).is_err());
}

#[test]
fn rigid_translation_and_infinitesimal_rotation_generate_no_stress() {
    let (positions, elements) = cube();
    let displacement: Vec<_> = positions
        .iter()
        .flat_map(|p| {
            [
                s("0.1")
                    .checked_sub(s("0.01").checked_mul(p[1]).unwrap())
                    .unwrap(),
                s("0.2")
                    .checked_add(s("0.01").checked_mul(p[0]).unwrap())
                    .unwrap(),
                s("0.3"),
            ]
        })
        .collect();
    let material = IsotropicElasticity::new(s("100"), s("0.25")).unwrap();
    for element in &elements {
        assert_eq!(
            material
                .stress(element.strain(&displacement).unwrap(), Scalar::ZERO)
                .unwrap(),
            [[Scalar::ZERO; 3]; 3]
        );
    }
    for poisson in [s("-1"), s("0.5"), s("0.8")] {
        assert!(IsotropicElasticity::new(s("100"), poisson).is_err());
    }
    assert!(IsotropicElasticity::new(Scalar::ZERO, s("0.25")).is_err());
}

#[test]
fn unconstrained_shrinkage_has_displacement_but_negligible_stress_and_no_allocation() {
    let (positions, elements) = cube();
    let materials = [IsotropicElasticity::new(s("100"), s("0.25")).unwrap(); 6];
    let constraints = restraints();
    let system = MechanicalSystem::new(&elements, &materials, &constraints, limits()).unwrap();
    let mut displacement = [Scalar::ZERO; 24];
    let mut fields = [[Scalar::ZERO; 24]; 6];
    let [
        trial,
        residual,
        preconditioned,
        direction,
        product,
        diagonal,
    ] = &mut fields;
    let mut work = MechanicsWorkspace {
        trial,
        residual,
        preconditioned,
        direction,
        product,
        diagonal,
    };
    let mut result = None;
    let allocation = allocation_counter::measure(|| {
        result = Some(system.solve(
            &mut displacement,
            &[Scalar::ZERO; 24],
            &[s("-0.01"); 6],
            &mut work,
        ));
    });
    let report = result.unwrap().unwrap();
    assert!(report.residual <= limits().residual);
    assert_eq!(allocation.count_total, 0);
    for (node, position) in positions.iter().enumerate() {
        for axis in 0..3 {
            let expected = position[axis].checked_mul(s("-0.01")).unwrap();
            assert!(
                displacement[3 * node + axis]
                    .checked_sub(expected)
                    .unwrap()
                    .abs()
                    .unwrap()
                    < s("0.0000001")
            );
        }
    }
    for (element, material) in elements.iter().zip(materials) {
        for stress in material
            .stress(element.strain(&displacement).unwrap(), s("-0.01"))
            .unwrap()
            .into_iter()
            .flatten()
        {
            assert!(stress.abs().unwrap() < s("0.00001"));
        }
    }
}

#[test]
fn restrained_shrinkage_produces_the_analytic_tensile_stress() {
    let (_, elements) = cube();
    let materials = [IsotropicElasticity::new(s("100"), s("0.25")).unwrap(); 6];
    let constraints = [Some(Scalar::ZERO); 24];
    let system = MechanicalSystem::new(&elements, &materials, &constraints, limits()).unwrap();
    let mut displacement = [Scalar::ZERO; 24];
    let mut fields = [[Scalar::ZERO; 24]; 6];
    let [
        trial,
        residual,
        preconditioned,
        direction,
        product,
        diagonal,
    ] = &mut fields;
    let mut work = MechanicsWorkspace {
        trial,
        residual,
        preconditioned,
        direction,
        product,
        diagonal,
    };
    system
        .solve(
            &mut displacement,
            &[Scalar::ZERO; 24],
            &[s("-0.01"); 6],
            &mut work,
        )
        .unwrap();
    for (element, material) in elements.iter().zip(materials) {
        let stress = material
            .stress(element.strain(&displacement).unwrap(), s("-0.01"))
            .unwrap();
        assert_eq!(
            stress,
            [
                [s("2"), Scalar::ZERO, Scalar::ZERO],
                [Scalar::ZERO, s("2"), Scalar::ZERO],
                [Scalar::ZERO, Scalar::ZERO, s("2")]
            ]
        );
    }
}

#[test]
fn failed_mechanical_solve_does_not_publish_trial_displacements() {
    let (_, elements) = cube();
    let materials = [IsotropicElasticity::new(s("100"), s("0.25")).unwrap(); 6];
    let constraints = restraints();
    let system = MechanicalSystem::new(
        &elements,
        &materials,
        &constraints,
        SolverLimits {
            iterations: 1,
            residual: Scalar::from_raw(1),
        },
    )
    .unwrap();
    let mut displacement = [Scalar::ZERO; 24];
    let mut fields = [[Scalar::ZERO; 24]; 6];
    let [
        trial,
        residual,
        preconditioned,
        direction,
        product,
        diagonal,
    ] = &mut fields;
    let mut work = MechanicsWorkspace {
        trial,
        residual,
        preconditioned,
        direction,
        product,
        diagonal,
    };
    assert!(matches!(
        system.solve(
            &mut displacement,
            &[Scalar::ZERO; 24],
            &[s("-0.01"); 6],
            &mut work
        ),
        Err(PhysicsError::NonConvergence { .. })
    ));
    assert_eq!(displacement, [Scalar::ZERO; 24]);
    assert!(
        system
            .solve(&mut displacement, &[], &[Scalar::ZERO; 6], &mut work)
            .is_err()
    );
    assert!(MechanicalSystem::new(&elements, &[], &constraints, limits()).is_err());
}
