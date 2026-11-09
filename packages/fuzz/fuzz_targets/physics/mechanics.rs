use hearthline_physics::Scalar;
use hearthline_physics::mechanics::{
    IsotropicElasticity, MechanicalSystem, MechanicsWorkspace, StiffnessMatrix, Tetrahedron,
};
use hearthline_physics::transport::SolverLimits;

pub fn exercise(input: &[u8]) {
    let byte = |i| input.get(i).copied().unwrap_or(0);
    let positions = [
        [Scalar::ZERO; 3],
        [Scalar::ONE, Scalar::ZERO, Scalar::ZERO],
        [Scalar::ZERO, Scalar::ONE, Scalar::ZERO],
        [Scalar::ZERO, Scalar::ZERO, Scalar::ONE],
    ];
    let elements = [Tetrahedron::compile([0, 1, 2, 3], &positions).unwrap()];
    let materials = [IsotropicElasticity::new(
        Scalar::integer(i64::from(byte(0)) * 4 + 1).unwrap(),
        Scalar::ratio(i64::from(byte(1) % 50), 100).unwrap(),
    )
    .unwrap()];
    let rows = core::array::from_fn::<_, 13, _>(|i| (12 * i) as u32);
    let columns = core::array::from_fn::<_, 144, _>(|i| (i % 12) as u32);
    let values = core::array::from_fn::<_, 144, _>(|i| {
        elements[0].stiffness(materials[0], i / 12, i % 12).unwrap()
    });
    let matrix = StiffnessMatrix::new(&rows, &columns, &values).unwrap();
    let mut constraints = [None; 12];
    for i in [0, 1, 2, 4, 5, 8] {
        constraints[i] = Some(Scalar::from_raw(i64::from(byte(i)) * 1000));
    }
    let limits = SolverLimits {
        iterations: u32::from(byte(2) % 32) + 1,
        residual: Scalar::from_raw(i64::from(byte(3)) + 1),
    };
    let mut system = MechanicalSystem::new(&elements, &materials, &constraints, limits).unwrap();
    if byte(4) & 1 != 0 {
        system = system.with_stiffness(matrix).unwrap();
        if byte(4) & 2 != 0 {
            system = system.with_symmetric_sweep().unwrap();
        }
    } else if byte(4) & 2 != 0 {
        system = system.with_normalized_preconditioner();
    }
    let external = core::array::from_fn::<_, 12, _>(|i| {
        Scalar::from_raw((i64::from(byte(i + 5)) - 128) * 1000)
    });
    let eigenstrain = [Scalar::from_raw((i64::from(byte(17)) - 128) * 100)];
    let initial = [Scalar::ZERO; 12];
    let mut displacement = initial;
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
    let result = system.solve(&mut displacement, &external, &eigenstrain, &mut work);
    if let Ok(report) = result {
        assert!(report.iterations <= limits.iterations && report.residual <= limits.residual);
        for i in 0..12 {
            if let Some(prescribed) = constraints[i] {
                assert_eq!(displacement[i], prescribed);
            }
        }
    } else {
        assert_eq!(displacement, initial);
    }
    let mut load = [Scalar::ZERO; 12];
    system
        .assemble_load(&external, &eigenstrain, &mut load)
        .unwrap();
    let mut prepared = initial;
    assert_eq!(system.solve_loaded(&mut prepared, &load, &mut work), result);
    assert_eq!(prepared, displacement);
}
