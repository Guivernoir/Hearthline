use super::super::diagnostic::Diagnostic;

#[test]
fn typed_schedule_diagnostics_decode_without_buffering_maps() {
    for source in [
        r#"{"kind":"unknown-instance"}"#,
        r#"{"kind":"wrong-profile"}"#,
        r#"{"kind":"completed-instance"}"#,
        r#"{"kind":"numerical","cause":{"kind":"overflow"}}"#,
        r#"{"kind":"flow","cause":{"kind":"invalid-state"},"attempts":3,"smallest_solver_step_us":10}"#,
        r#"{"kind":"connected","cause":{"kind":"invalid-state"},"retry":null}"#,
        r#"{"kind":"connected","cause":{"kind":"non-convergence","iterations":1,"residual_raw":30},"retry":{"attempts":2,"substeps":2,"completed_substeps":1,"smallest_step_raw":10}}"#,
    ] {
        let diagnostic: Diagnostic = serde_json::from_str(source).unwrap();
        assert_eq!(serde_json::to_string(&diagnostic).unwrap(), source);
        let measured = allocation_counter::measure(|| {
            assert_eq!(
                serde_json::from_str::<Diagnostic>(source).unwrap(),
                diagnostic
            );
        });
        assert_eq!(measured.count_total, 0);
    }
    let source = format!(
        "{{{}\"kind\":\"unknown-instance\"}}",
        "\"x\":0,".repeat(500)
    );
    assert!(source.len() < 4096);
    let measured = allocation_counter::measure(|| {
        assert!(serde_json::from_str::<Diagnostic>(&source).is_err());
    });
    assert!(measured.bytes_max <= 1024, "{measured:?}");
}

#[test]
fn schedule_diagnostics_reject_unknown_duplicate_and_inconsistent_fields() {
    for source in [
        r#"{"kind":"wrong-profile","kind":"wrong-profile"}"#,
        r#"{"kind":"wrong-profile","cause":{"kind":"overflow"}}"#,
        r#"{"kind":"numerical"}"#,
        r#"{"kind":"numerical","cause":{"kind":"overflow"},"attempts":1}"#,
        r#"{"kind":"numerical","cause":{"kind":"overflow"},"cause":{"kind":"overflow"}}"#,
        r#"{"kind":"flow","cause":{"kind":"overflow"},"attempts":1}"#,
        r#"{"kind":"flow","cause":{"kind":"overflow"},"attempts":-1,"smallest_solver_step_us":1}"#,
        r#"{"kind":"connected","cause":{"kind":"overflow"},"attempts":1}"#,
        r#"{"kind":"connected","cause":{"kind":"overflow"},"retry":{"attempts":1}}"#,
        r#"{"kind":"connected","cause":{"kind":"overflow"},"retry":{"attempts":1,"substeps":1,"completed_substeps":0,"smallest_step_raw":1,"unknown":0}}"#,
        r#"{"kind":"unknown-instance","unknown":0}"#,
        r#"{"kind":"unknown"}"#,
        r#"{}"#,
        r#"[]"#,
    ] {
        assert!(
            serde_json::from_str::<Diagnostic>(source).is_err(),
            "{source}"
        );
    }
}
