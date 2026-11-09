use super::*;

#[test]
fn symmetric_sweep_matches_independent_two_by_two_factor_inverse() {
    // A=[4 2;2 4]; M=(D+L)D^-1(D+U)=[4 2;2 5].
    // Thus 4*M^-1=[1.25 -0.5;-0.5 1], exactly representable here.
    let values = [4, 2, 2, 4].map(|v| Scalar::integer(v).unwrap());
    let matrix = StiffnessMatrix::new(&[0, 2, 4], &[0, 1, 0, 1], &values).unwrap();
    let diagonal = [values[0]; 2];
    for (input, expected) in [
        (
            [Scalar::ONE, Scalar::ZERO],
            [Scalar::ratio(5, 4).unwrap(), Scalar::ratio(-1, 2).unwrap()],
        ),
        (
            [Scalar::ZERO, Scalar::ONE],
            [Scalar::ratio(-1, 2).unwrap(), Scalar::ONE],
        ),
    ] {
        let mut output = [Scalar::from_raw(i64::MAX); 2];
        matrix
            .symmetric_sweep(&input, &diagonal, &[None; 2], values[0], &mut output)
            .unwrap();
        assert_eq!(output, expected);
    }
    let mut output = [Scalar::from_raw(i64::MAX); 2];
    matrix
        .symmetric_sweep(
            &[Scalar::ONE; 2],
            &diagonal,
            &[Some(Scalar::ONE), None],
            values[0],
            &mut output,
        )
        .unwrap();
    assert_eq!(output, [Scalar::ZERO, Scalar::ONE]);
}

#[test]
fn sweep_rounds_ties_once_and_reports_wide_accumulation_overflow() {
    let values = [Scalar::integer(2).unwrap()];
    let matrix = StiffnessMatrix::new(&[0, 1], &[0], &values).unwrap();
    for (raw, expected) in [(1, 0), (3, 2), (-1, 0), (-3, -2)] {
        let mut output = [Scalar::ZERO];
        matrix
            .symmetric_sweep(
                &[Scalar::from_raw(raw)],
                &values,
                &[None],
                Scalar::ONE,
                &mut output,
            )
            .unwrap();
        assert_eq!(output[0].raw(), expected);
    }
    let max = Scalar::from_raw(i64::MAX);
    let values = [max, max, max, max];
    let matrix = StiffnessMatrix::new(&[0, 2, 4], &[0, 1, 0, 1], &values).unwrap();
    assert_eq!(
        matrix.symmetric_sweep(
            &[max, max.checked_neg().unwrap()],
            &[max; 2],
            &[None; 2],
            max,
            &mut [Scalar::ZERO; 2]
        ),
        Err(PhysicsError::Overflow)
    );
}
