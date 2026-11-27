use super::*;

#[test]
fn previous_schedule_archives_migrate_with_identical_fields_and_failure_state() {
    let old: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../project/physics/runtime/schedule/previous-golden.json"
    )))
    .unwrap();
    let failed: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../project/physics/runtime/schedule/previous-failure-golden.json"
    )))
    .unwrap();
    let records = [
        (
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../project/physics/runtime/schedule/v1/initial.ndjson"
            ))
            .as_slice(),
            0,
            false,
            &old["initial_digest"],
        ),
        (
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../project/physics/runtime/schedule/v1/fourth.ndjson"
            ))
            .as_slice(),
            4,
            false,
            &old["after_four_events_digest"],
        ),
        (
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../project/physics/runtime/schedule/v1/final.ndjson"
            ))
            .as_slice(),
            15,
            false,
            &old["final_digest"],
        ),
        (
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../project/physics/runtime/schedule/v1/failure.ndjson"
            ))
            .as_slice(),
            1,
            true,
            &failed["state_digest"],
        ),
    ];
    for (bytes, events, is_failure, expected_hash) in records {
        let digest = crate::physics::checkpoint::hex_digest(Sha256::digest(bytes).into());
        assert_eq!(
            std::str::from_utf8(&digest).unwrap(),
            expected_hash.as_str().unwrap()
        );
        let plan = if is_failure {
            compile(vec![
                registration("a-healthy", compressible(false)),
                registration("b-failed", compressible(true)),
            ])
        } else {
            compile(registrations())
        };
        let expected_model = if is_failure {
            &failed["model_digest"]
        } else {
            &old["model_digest"]
        };
        assert_eq!(plan.digest(), expected_model.as_str().unwrap());
        let mut direct = PhysicsSchedule::new(Arc::clone(&plan)).unwrap();
        for _ in 0..events {
            direct.step_next().unwrap();
        }
        if is_failure {
            assert!(direct.step_next().is_err());
        }
        let mut restored = PhysicsSchedule::read_snapshot(plan, Cursor::new(bytes)).unwrap();
        for instance in 0..restored.plan().instances().len() {
            assert_eq!(
                restored.checkpoint(instance).unwrap(),
                direct.checkpoint(instance).unwrap()
            );
        }
        assert_eq!(restored.failure(), direct.failure());
        assert_eq!(restored.next_event(), direct.next_event());
        assert_eq!(
            restored.state_digest().unwrap(),
            direct.state_digest().unwrap()
        );
        let upgraded = capture(&mut restored);
        let header: serde_json::Value =
            serde_json::from_slice(upgraded.split(|b| *b == b'\n').next().unwrap()).unwrap();
        assert_eq!(header["schema_version"], "0.2.0");
        assert_eq!(capture(&mut direct), upgraded);
    }
}
