use hearthline_physics::mechanics::{
    IsotropicElasticity, MechanicalSystem, MechanicsWorkspace, StiffnessMatrix, Tetrahedron,
};
use hearthline_physics::transport::SolverLimits;
use hearthline_physics::{PhysicsError, Scalar};

fn s(value: &str) -> Scalar {
    value.parse().unwrap()
}

fn tetrahedron() -> Tetrahedron {
    Tetrahedron::compile(
        [0, 1, 2, 3],
        &[
            [Scalar::ZERO; 3],
            [Scalar::ONE, Scalar::ZERO, Scalar::ZERO],
            [Scalar::ZERO, Scalar::ONE, Scalar::ZERO],
            [Scalar::ZERO, Scalar::ZERO, Scalar::ONE],
        ],
    )
    .unwrap()
}

fn restraints() -> [Option<Scalar>; 12] {
    let mut constraints = [None; 12];
    for index in [0, 1, 2, 4, 5, 8] {
        constraints[index] = Some(Scalar::ZERO);
    }
    constraints
}

fn assemble(
    element: Tetrahedron,
    material: IsotropicElasticity,
) -> ([u32; 13], [u32; 144], [Scalar; 144]) {
    (
        core::array::from_fn(|i| (12 * i) as u32),
        core::array::from_fn(|i| (i % 12) as u32),
        core::array::from_fn(|i| element.stiffness(material, i / 12, i % 12).unwrap()),
    )
}

#[test]
fn element_matrix_is_symmetric_and_matches_independent_stress_integration() {
    let element = tetrahedron();
    for poisson in ["-0.1", "0.25", "0.49"] {
        let material = IsotropicElasticity::new(s("100"), s(poisson)).unwrap();
        let (rows, columns, values) = assemble(element, material);
        let matrix = StiffnessMatrix::new(&rows, &columns, &values).unwrap();
        let materials = [material];
        let elements = [element];
        let constraints = restraints();
        let system = MechanicalSystem::new(
            &elements,
            &materials,
            &constraints,
            SolverLimits {
                iterations: 100,
                residual: s("0.000001"),
            },
        )
        .unwrap();
        let displacement =
            core::array::from_fn::<_, 12, _>(|i| Scalar::from_raw((i as i64 - 5) * 125_000));
        let mut assembled = [Scalar::ZERO; 12];
        let mut integrated = [Scalar::ZERO; 12];
        matrix.apply(&displacement, &mut assembled).unwrap();
        system.apply(&displacement, &mut integrated).unwrap();
        for (a, b) in assembled.iter().zip(integrated) {
            assert!(a.checked_sub(b).unwrap().abs().unwrap().raw() <= 2);
        }
        // Translation and infinitesimal rotation are rigid modes, not strain.
        let rigid = [
            s("0.1"),
            s("0.2"),
            s("0.3"),
            s("0.1"),
            s("0.21"),
            s("0.3"),
            s("0.09"),
            s("0.2"),
            s("0.3"),
            s("0.1"),
            s("0.2"),
            s("0.3"),
        ];
        matrix.apply(&rigid, &mut assembled).unwrap();
        assert!(assembled.iter().all(|v| v.abs().unwrap().raw() <= 1));
    }
    assert_eq!(
        element.stiffness(IsotropicElasticity::new(s("1"), s("0.25")).unwrap(), 12, 0),
        Err(PhysicsError::BufferLength)
    );
}

#[test]
fn compiled_solve_has_analytic_free_shrinkage_without_allocation_or_false_residual() {
    let elements = [tetrahedron()];
    let materials = [IsotropicElasticity::new(s("100"), s("0.25")).unwrap()];
    let (rows, columns, values) = assemble(elements[0], materials[0]);
    let matrix = StiffnessMatrix::new(&rows, &columns, &values).unwrap();
    let constraints = restraints();
    let limits = SolverLimits {
        iterations: 100,
        residual: s("0.000001"),
    };
    let system = MechanicalSystem::new(&elements, &materials, &constraints, limits)
        .unwrap()
        .with_stiffness(matrix)
        .unwrap();
    let mut displacement = [Scalar::ZERO; 12];
    let mut fields = [[Scalar::ZERO; 12]; 6];
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
    let mut report = None;
    let allocations = allocation_counter::measure(|| {
        report = Some(
            system
                .solve(
                    &mut displacement,
                    &[Scalar::ZERO; 12],
                    &[s("-0.01")],
                    &mut work,
                )
                .unwrap(),
        );
    });
    assert_eq!(allocations.count_total, 0);
    assert!(report.unwrap().iterations > 0);
    assert!(report.unwrap().residual <= limits.residual);
    for (index, value) in displacement.into_iter().enumerate() {
        let expected = if [3, 7, 11].contains(&index) {
            s("-0.01")
        } else {
            Scalar::ZERO
        };
        assert!(value.checked_sub(expected).unwrap().abs().unwrap() < s("0.0000001"));
    }
    let strict = MechanicalSystem::new(
        &elements,
        &materials,
        &constraints,
        SolverLimits {
            iterations: 1,
            residual: Scalar::from_raw(1),
        },
    )
    .unwrap()
    .with_stiffness(matrix)
    .unwrap();
    let before = displacement;
    assert!(
        strict
            .solve(
                &mut displacement,
                &[s("0.001"); 12],
                &[s("-0.02")],
                &mut work
            )
            .is_err()
    );
    assert_eq!(displacement, before);
}

#[test]
fn sparse_rows_round_only_after_wide_accumulation_and_reject_overflow() {
    let values = [s("0.5"); 4];
    let matrix = StiffnessMatrix::new(&[0, 2, 4], &[0, 1, 0, 1], &values).unwrap();
    let mut result = [Scalar::ZERO; 2];
    matrix
        .apply(&[Scalar::from_raw(1); 2], &mut result)
        .unwrap();
    assert_eq!(result, [Scalar::from_raw(1); 2]);
    assert_eq!(
        matrix.apply(&[], &mut result),
        Err(PhysicsError::BufferLength)
    );
    let values = [Scalar::from_raw(i64::MAX); 9];
    let matrix =
        StiffnessMatrix::new(&[0, 3, 6, 9], &[0, 1, 2, 0, 1, 2, 0, 1, 2], &values).unwrap();
    assert_eq!(
        matrix.apply(&[Scalar::from_raw(i64::MAX); 3], &mut [Scalar::ZERO; 3]),
        Err(PhysicsError::Overflow)
    );
}

#[test]
fn sparse_validation_rejects_invalid_offsets_columns_diagonal_and_asymmetry() {
    for (rows, columns, values) in [
        (vec![0], vec![], vec![]),
        (vec![1, 2], vec![0, 0], vec![Scalar::ONE; 2]),
        (vec![0, 3, 2], vec![0, 1], vec![Scalar::ONE; 2]),
        (vec![0, 0], vec![], vec![]),
        (vec![0, 1], vec![1], vec![Scalar::ONE]),
        (vec![0, 2], vec![0, 0], vec![Scalar::ONE; 2]),
        (vec![0, 1], vec![0], vec![]),
        (vec![0, 1], vec![0], vec![Scalar::ZERO]),
        (
            vec![0, 2, 4],
            vec![0, 1, 0, 1],
            vec![s("2"), s("1"), s("0.5"), s("2")],
        ),
    ] {
        assert!(StiffnessMatrix::new(&rows, &columns, &values).is_err());
    }
}

#[test]
fn preconditioners_preserve_analytic_equilibrium_prepared_loads_and_checked_failure() {
    for young in ["100", "1000"] {
        for (sparse, sweep) in [(false, false), (true, false), (true, true)] {
            let elements = [tetrahedron()];
            let materials = [IsotropicElasticity::new(s(young), s("0.25")).unwrap()];
            let (rows, columns, values) = assemble(elements[0], materials[0]);
            let matrix = StiffnessMatrix::new(&rows, &columns, &values).unwrap();
            let constraints = restraints();
            let limits = SolverLimits {
                iterations: 100,
                residual: s("0.000001"),
            };
            let mut system = MechanicalSystem::new(&elements, &materials, &constraints, limits)
                .unwrap()
                .with_normalized_preconditioner();
            if sparse {
                system = system.with_stiffness(matrix).unwrap();
            }
            if sweep {
                system = system.with_symmetric_sweep().unwrap();
            }
            let mut displacement = [Scalar::ZERO; 12];
            let mut fields = [[Scalar::ZERO; 12]; 6];
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
            let mut report = None;
            let measured = allocation_counter::measure(|| {
                report = Some(
                    system
                        .solve(
                            &mut displacement,
                            &[Scalar::ZERO; 12],
                            &[s("-0.00001")],
                            &mut work,
                        )
                        .unwrap(),
                );
            });
            assert_eq!(measured.count_total, 0);
            assert!(report.unwrap().residual <= limits.residual);
            let expected = displacement;
            let mut load = [Scalar::ZERO; 12];
            let measured = allocation_counter::measure(|| {
                system
                    .assemble_load(&[Scalar::ZERO; 12], &[s("-0.00001")], &mut load)
                    .unwrap();
                displacement.fill(Scalar::ZERO);
                assert_eq!(
                    system
                        .solve_loaded(&mut displacement, &load, &mut work)
                        .unwrap(),
                    report.unwrap()
                );
                assert_eq!(displacement, expected);
            });
            assert_eq!(measured.count_total, 0);
            assert_eq!(
                system.assemble_load(&[], &[Scalar::ZERO], &mut load),
                Err(PhysicsError::BufferLength)
            );
            assert_eq!(
                system.assemble_load(&load, &[], &mut []),
                Err(PhysicsError::BufferLength)
            );
            assert_eq!(
                system.solve_loaded(&mut displacement, &[], &mut work),
                Err(PhysicsError::BufferLength)
            );
            assert_eq!(displacement, expected);
            for (index, value) in displacement.into_iter().enumerate() {
                let expected = if [3, 7, 11].contains(&index) {
                    s("-0.00001")
                } else {
                    Scalar::ZERO
                };
                assert!(value.checked_sub(expected).unwrap().abs().unwrap() < s("0.0000001"));
            }
            let mut strict = MechanicalSystem::new(
                &elements,
                &materials,
                &constraints,
                SolverLimits {
                    iterations: 1,
                    residual: Scalar::from_raw(1),
                },
            )
            .unwrap()
            .with_stiffness(matrix)
            .unwrap()
            .with_normalized_preconditioner();
            if sweep {
                strict = strict.with_symmetric_sweep().unwrap();
            }
            let before = displacement;
            assert!(
                strict
                    .solve(
                        &mut displacement,
                        &[s("0.001"); 12],
                        &[s("-0.02")],
                        &mut work
                    )
                    .is_err()
            );
            assert_eq!(displacement, before);
        }
    }
}

#[test]
fn symmetric_sweep_preserves_nonzero_prescribed_displacements() {
    let elements = [tetrahedron()];
    let materials = [IsotropicElasticity::new(s("1000"), s("0.25")).unwrap()];
    let (rows, columns, values) = assemble(elements[0], materials[0]);
    let matrix = StiffnessMatrix::new(&rows, &columns, &values).unwrap();
    let expected = core::array::from_fn::<_, 12, _>(|i| Scalar::from_raw((i as i64 + 1) * 100_000));
    let mut constraints = restraints();
    for (constraint, value) in constraints.iter_mut().zip(expected) {
        if constraint.is_some() {
            *constraint = Some(value);
        }
    }
    let system = MechanicalSystem::new(
        &elements,
        &materials,
        &constraints,
        SolverLimits {
            iterations: 100,
            residual: s("0.000001"),
        },
    )
    .unwrap();
    assert!(
        MechanicalSystem::new(
            &elements,
            &materials,
            &constraints,
            SolverLimits {
                iterations: 100,
                residual: s("0.000001")
            }
        )
        .unwrap()
        .with_symmetric_sweep()
        .is_err()
    );
    let system = system
        .with_stiffness(matrix)
        .unwrap()
        .with_symmetric_sweep()
        .unwrap();
    let mut load = [Scalar::ZERO; 12];
    system.apply(&expected, &mut load).unwrap();
    let mut displacement = [Scalar::ZERO; 12];
    let [
        trial,
        residual,
        preconditioned,
        direction,
        product,
        diagonal,
    ] = &mut [[Scalar::ZERO; 12]; 6];
    let mut work = MechanicsWorkspace {
        trial,
        residual,
        preconditioned,
        direction,
        product,
        diagonal,
    };
    system
        .solve_loaded(&mut displacement, &load, &mut work)
        .unwrap();
    for i in 0..12 {
        if constraints[i].is_some() {
            assert_eq!(displacement[i], expected[i]);
        } else {
            assert!(
                displacement[i]
                    .checked_sub(expected[i])
                    .unwrap()
                    .abs()
                    .unwrap()
                    <= s("0.0000001")
            );
        }
    }
}
