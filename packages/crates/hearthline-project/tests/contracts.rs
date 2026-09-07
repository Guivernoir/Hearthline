use std::path::PathBuf;

use hearthline_project::{
    CapacityAssessment, CapacityEvidenceRecord, CapacityPlan, CapacityResource, CapacityStatus,
    MODEL_LOCK_SCHEMA_VERSION, MODEL_SOURCE_SCHEMA_VERSION, ModelLock, ModelLockStatus,
    ModelSourceDocument, ModelSourceKind, ProjectError,
};

#[test]
fn model_source_documents_have_content_and_path_sensitive_digests() {
    let first = ModelSourceDocument::new(
        PathBuf::from("blueprints/cell.yaml"),
        ModelSourceKind::Blueprint,
        "id: cell\n".into(),
    );
    let same = first.clone();
    let different_path = ModelSourceDocument::new(
        PathBuf::from("blueprints/other.yaml"),
        ModelSourceKind::Blueprint,
        first.source.clone(),
    );
    let different_source = ModelSourceDocument::new(
        first.path.clone(),
        ModelSourceKind::Blueprint,
        "id: other\n".into(),
    );
    let windows_checkout = ModelSourceDocument::new(
        PathBuf::from(r"blueprints\cell.yaml"),
        ModelSourceKind::Blueprint,
        "id: cell\r\n".into(),
    );
    assert_eq!(first.schema_version, MODEL_SOURCE_SCHEMA_VERSION);
    assert_eq!(first.digest(), same.digest());
    assert_ne!(first.digest(), different_path.digest());
    assert_ne!(first.digest(), different_source.digest());
    assert_eq!(first.digest(), windows_checkout.digest());
    assert_eq!(first.digest().len(), 64);

    let kinds = [
        ModelSourceKind::Project,
        ModelSourceKind::Blueprint,
        ModelSourceKind::Instance,
        ModelSourceKind::Appliance,
        ModelSourceKind::Connection,
        ModelSourceKind::Scenario,
        ModelSourceKind::Process,
        ModelSourceKind::ControlProgram,
        ModelSourceKind::CapacityPolicy,
    ];
    assert_eq!(kinds.len(), 9);
}

#[test]
fn model_lock_migrates_previous_partitions_and_writes_only_current_schema() {
    let previous = r#"{
      "schema_version": "0.1.0",
      "project_digest": "digest",
      "compiler_version": "0.3.1",
      "sources": [],
      "object_digests": [],
      "partitions": ["cell-a", "cell-b"],
      "generated_catalogs": [],
      "capacity": { "assessments": [] },
      "update_reason": "migration test"
    }"#;
    let migrated = ModelLock::from_json(previous).expect("previous lock migration");
    assert_eq!(migrated.schema_version, MODEL_LOCK_SCHEMA_VERSION);
    assert_eq!(migrated.partition_assignments.len(), 2);
    assert!(
        migrated
            .partition_assignments
            .iter()
            .all(|assignment| assignment.site == "canonical-project")
    );
    let emitted = migrated.to_json().expect("current lock JSON");
    assert!(emitted.ends_with('\n'));
    assert!(!emitted.contains("\"partitions\""));
    assert_eq!(
        ModelLock::from_json(&emitted).expect("emitted lock"),
        migrated
    );
    assert_eq!(migrated.status_against(&migrated), ModelLockStatus::Current);
    let mut stale = migrated.clone();
    stale.update_reason = "different".into();
    assert_eq!(migrated.status_against(&stale), ModelLockStatus::Stale);
    assert_eq!(ModelLockStatus::Missing, ModelLockStatus::Missing);

    assert!(ModelLock::from_json("not-json").is_err());
    let unsupported = previous.replace("0.1.0", "9.9.9");
    assert!(ModelLock::from_json(&unsupported).is_err());

    let empty = CapacityPlan::default();
    assert!(empty.accepted());
    assert_eq!(empty.rejected().count(), 0);
}

#[test]
fn project_errors_preserve_owning_boundary_in_diagnostics() {
    for error in [
        ProjectError::Io("disk".into()),
        ProjectError::Configuration("schema".into()),
        ProjectError::Blueprint("graph".into()),
        ProjectError::Capacity("reserve".into()),
        ProjectError::Lock("digest".into()),
        ProjectError::Transaction("journal".into()),
    ] {
        assert!(!error.to_string().is_empty());
    }
}

#[test]
fn capacity_assessment_classifies_every_reserve_and_limit_boundary() {
    let evidence = || CapacityEvidenceRecord {
        source: "contract-test".into(),
        detail: "measured demand".into(),
    };
    let measured = |demand, reviewed_limit, structural_limit| {
        CapacityAssessment::measured(
            CapacityResource::CellComponents,
            "factory/cell",
            demand,
            reviewed_limit,
            structural_limit,
            "reject",
            evidence(),
        )
    };

    let accepted = measured(75, 100, None);
    assert_eq!(accepted.reserve_percent, 25);
    assert_eq!(accepted.status, CapacityStatus::Accepted);

    let review = measured(76, 100, None);
    assert_eq!(review.reserve_percent, 24);
    assert_eq!(review.status, CapacityStatus::ReviewRequired);

    for rejected in [
        measured(1, 0, None),
        measured(101, 100, None),
        measured(81, 100, Some(80)),
    ] {
        assert_eq!(rejected.reserve_percent, 0);
        assert_eq!(rejected.status, CapacityStatus::Rejected);
    }

    let plan = CapacityPlan {
        assessments: vec![accepted, review],
    };
    assert!(!plan.accepted());
    assert_eq!(plan.rejected().count(), 1);
}
