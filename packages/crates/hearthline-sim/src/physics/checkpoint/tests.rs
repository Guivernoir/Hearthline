use super::NumericalCause;

#[test]
fn malformed_numerical_causes_do_not_buffer_arbitrary_maps() {
    let source = format!("{{{}\"kind\":\"overflow\"}}", "\"x\":0,".repeat(500));
    assert!(source.len() < 4096);
    let mut rejected = false;
    let measured = allocation_counter::measure(|| {
        rejected = serde_json::from_str::<NumericalCause>(&source).is_err();
    });
    assert!(
        measured.bytes_max <= 1024,
        "bounded cause decoder used {} bytes",
        measured.bytes_max
    );
    assert!(rejected);
}

#[test]
fn numerical_cause_formats_remain_stable_and_decode_without_allocation() {
    let causes = [
        NumericalCause::Overflow,
        NumericalCause::DivisionByZero,
        NumericalCause::InvalidDecimal,
        NumericalCause::InvalidGeometry,
        NumericalCause::InvalidCoefficient,
        NumericalCause::InsufficientResolution,
        NumericalCause::InvalidState,
        NumericalCause::BufferLength,
        NumericalCause::NonPositiveDefinite,
        NumericalCause::NonConvergence {
            iterations: 7,
            residual_raw: -123,
        },
    ];
    for cause in causes {
        let source = serde_json::to_string(&cause).unwrap();
        let measured = allocation_counter::measure(|| {
            assert_eq!(
                serde_json::from_str::<NumericalCause>(&source).unwrap(),
                cause
            );
        });
        assert_eq!(measured.count_total, 0);
    }
    assert_eq!(
        serde_json::from_str::<NumericalCause>(
            r#"{"residual_raw":-123,"iterations":7,"kind":"non-convergence"}"#
        )
        .unwrap(),
        causes[9]
    );
    for source in [
        r#"{"kind":"overflow","kind":"overflow"}"#,
        r#"{"kind":"overflow","iterations":1}"#,
        r#"{"kind":"overflow","iterations":null}"#,
        r#"{"kind":"overflow","residual_raw":1}"#,
        r#"{"kind":"overflow","unknown":0}"#,
        r#"{"kind":"non-convergence","iterations":1}"#,
        r#"{"kind":"non-convergence","iterations":1,"residual_raw":1,"iterations":2}"#,
        r#"{"kind":"non-convergence","iterations":1,"residual_raw":1,"residual_raw":2}"#,
        r#"{"kind":"non-convergence","iterations":-1,"residual_raw":1}"#,
        r#"{"kind":"non-convergence","iterations":1,"residual_raw":null}"#,
        r#"{"kind":"unknown"}"#,
        r#"{}"#,
        r#"[]"#,
    ] {
        assert!(
            serde_json::from_str::<NumericalCause>(source).is_err(),
            "{source}"
        );
    }
}
