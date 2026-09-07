use std::fs;
use std::path::{Path, PathBuf};

use hearthline_project::{
    CapacityResource, DraftChange, DraftId, ModelTransactionStore, TransactionError,
};
use walkdir::WalkDir;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn staged_repository() -> tempfile::TempDir {
    let source = repository_root();
    let target = tempfile::tempdir().expect("temporary repository");
    copy_tree(
        &source.join("project/config"),
        &target.path().join("project/config"),
    );
    copy_tree(
        &source.join("project/control"),
        &target.path().join("project/control"),
    );
    for catalog in [
        "packages/web/src/generated/appliance-configs.json",
        "packages/web/src/generated/process-view.json",
    ] {
        let destination = target.path().join(catalog);
        fs::create_dir_all(destination.parent().unwrap()).expect("catalog parent");
        fs::copy(source.join(catalog), destination).expect("generated catalog");
    }
    target
}

fn copy_tree(source: &Path, destination: &Path) {
    for entry in WalkDir::new(source) {
        let entry = entry.expect("source entry");
        let relative = entry.path().strip_prefix(source).expect("relative source");
        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(target).expect("target directory");
        } else {
            fs::copy(entry.path(), target).expect("target file");
        }
    }
}

fn instance_path(name: &str) -> PathBuf {
    PathBuf::from(format!("project/config/instances/{name}.yaml"))
}

#[test]
fn draft_preview_rejects_invalid_parameters_and_reports_source_location() {
    let repository = staged_repository();
    let mut store = ModelTransactionStore::new(repository.path()).expect("transaction store");
    let draft = store.create().expect("draft");
    let path = instance_path("body-preparation-industrial-water");
    let source = fs::read_to_string(repository.path().join(&path))
        .expect("instance")
        .replace("sensor-count: 12", "sensor-count: 99");
    store
        .update(
            draft.id,
            DraftChange {
                path: path.clone(),
                source: Some(source),
            },
        )
        .expect("draft update");
    let preview = store
        .preview(draft.id)
        .expect("failed preview is structured");

    assert!(preview.candidate_revision.is_none());
    assert_eq!(preview.diagnostics[0].path.as_deref(), Some(path.as_path()));
    assert!(preview.diagnostics[0].message.contains("invalid value"));
}

#[test]
fn complete_overlay_previews_topology_capacity_and_commits_generated_authority() {
    let repository = staged_repository();
    let mut store = ModelTransactionStore::new(repository.path()).expect("transaction store");
    let draft = store.create().expect("draft");
    let path = instance_path("body-preparation-industrial-water");
    let source = fs::read_to_string(repository.path().join(&path))
        .expect("instance")
        .replace("pump-count: 4", "pump-count: 5");
    store
        .update(
            draft.id,
            DraftChange {
                path,
                source: Some(source),
            },
        )
        .expect("draft update");
    let preview = store.preview(draft.id).expect("preview");

    assert!(preview.diagnostics.is_empty());
    assert!(
        preview
            .added_objects
            .iter()
            .any(|id| id.ends_with("pump-05"))
    );
    assert!(preview.capacity_deltas.iter().any(|delta| {
        delta.resource == CapacityResource::CellComponents
            && delta.scope == "body-preparation-industrial-water"
            && delta.previous_demand == Some(17)
            && delta.candidate_demand == Some(18)
    }));
    assert_eq!(preview.preview_catalogs.len(), 2);

    let commit = store
        .commit(
            draft.id,
            &draft.base_revision,
            "transaction integration test",
        )
        .expect("atomic commit");
    assert_ne!(commit.previous_revision, commit.revision);
    assert!(
        repository
            .path()
            .join("project/config/model.lock.json")
            .is_file()
    );
    assert!(
        repository
            .path()
            .join("packages/web/src/generated/appliance-configs.json")
            .is_file()
    );
}

#[test]
fn overlay_rejects_id_collisions_capacity_excess_and_stale_revisions() {
    let repository = staged_repository();
    let mut store = ModelTransactionStore::new(repository.path()).expect("transaction store");

    let collision = store.create().expect("collision draft");
    let return_path = instance_path("body-preparation-return-water");
    let duplicate = fs::read_to_string(repository.path().join(&return_path))
        .expect("return instance")
        .replace(
            "id: body-preparation-return-water",
            "id: body-preparation-industrial-water",
        );
    store
        .update(
            collision.id,
            DraftChange {
                path: return_path,
                source: Some(duplicate),
            },
        )
        .expect("collision source");
    assert!(
        store.preview(collision.id).unwrap().diagnostics[0]
            .message
            .contains("duplicate blueprint instance")
    );

    let capacity = store.create().expect("capacity draft");
    let blueprint_path = PathBuf::from("project/config/blueprints/water-distribution-train.yaml");
    let blueprint = fs::read_to_string(repository.path().join(&blueprint_path))
        .expect("blueprint")
        .replace("maximum: 24", "maximum: 200");
    let industrial_path = instance_path("body-preparation-industrial-water");
    let instance = fs::read_to_string(repository.path().join(&industrial_path))
        .expect("instance")
        .replace("sensor-count: 12", "sensor-count: 150");
    for (path, source) in [(blueprint_path, blueprint), (industrial_path, instance)] {
        store
            .update(
                capacity.id,
                DraftChange {
                    path,
                    source: Some(source),
                },
            )
            .expect("capacity source");
    }
    assert!(
        store.preview(capacity.id).unwrap().diagnostics[0]
            .message
            .contains("capacity plan rejected")
    );

    let stale = store.create().expect("stale draft");
    let external = repository
        .path()
        .join(instance_path("body-preparation-return-water"));
    let changed = fs::read_to_string(&external)
        .expect("return source")
        .replace("site: factory", "site: replacement-factory");
    fs::write(external, changed).expect("external model change");
    assert!(matches!(
        store.commit(stale.id, &stale.base_revision, "must be stale"),
        Err(TransactionError::StaleRevision { .. })
    ));
}

#[test]
fn draft_rejects_traversal_oversize_and_excessive_nesting() {
    let repository = staged_repository();
    let mut store = ModelTransactionStore::new(repository.path()).expect("transaction store");
    let draft = store.create().expect("draft");
    assert!(
        store
            .update(
                draft.id,
                DraftChange {
                    path: PathBuf::from("../outside.yaml"),
                    source: Some("id: outside".into()),
                },
            )
            .is_err()
    );
    assert!(
        store
            .update(
                draft.id,
                DraftChange {
                    path: instance_path("oversize"),
                    source: Some("x".repeat(512 * 1024 + 1)),
                },
            )
            .is_err()
    );
    assert!(
        store
            .update(
                draft.id,
                DraftChange {
                    path: instance_path("nested"),
                    source: Some(format!("value: {}", "[".repeat(65))),
                },
            )
            .is_err()
    );
    assert!(store.discard(DraftId(draft.id.0)));
}

#[test]
fn startup_recovery_rolls_back_an_interrupted_multi_file_install() {
    let repository = staged_repository();
    let target = repository.path().join("project/config/recovery-probe.yaml");
    let backup = repository
        .path()
        .join("project/config/recovery-probe.backup");
    let staged = repository
        .path()
        .join("project/config/recovery-probe.staged");
    fs::write(&target, "state: installed\n").expect("installed target");
    fs::write(&backup, "state: original\n").expect("transaction backup");
    let journal = serde_json::json!({
        "schema_version": "0.1.0",
        "entries": [{
            "target": target,
            "staged": staged,
            "backup": backup,
        }],
    });
    fs::write(
        repository.path().join(".hearthline-transaction.json"),
        serde_json::to_vec_pretty(&journal).expect("journal JSON"),
    )
    .expect("interrupted journal");

    ModelTransactionStore::new(repository.path()).expect("startup recovery");

    assert_eq!(
        fs::read_to_string(repository.path().join("project/config/recovery-probe.yaml"))
            .expect("recovered target"),
        "state: original\n"
    );
    assert!(
        !repository
            .path()
            .join(".hearthline-transaction.json")
            .exists()
    );
}

#[test]
fn startup_recovery_rejects_a_journal_outside_transaction_roots() {
    let repository = staged_repository();
    let outside = repository.path().join("outside.txt");
    fs::write(&outside, "must remain\n").expect("outside sentinel");
    let journal = serde_json::json!({
        "schema_version": "0.1.0",
        "entries": [{
            "target": outside,
            "staged": null,
            "backup": repository.path().join("outside.backup"),
        }],
    });
    fs::write(
        repository.path().join(".hearthline-transaction.json"),
        serde_json::to_vec_pretty(&journal).expect("journal JSON"),
    )
    .expect("tampered journal");

    assert!(matches!(
        ModelTransactionStore::new(repository.path()),
        Err(TransactionError::Validation(_))
    ));
    assert_eq!(fs::read_to_string(outside).unwrap(), "must remain\n");
}

#[test]
fn draft_lifecycle_rejects_unknown_empty_reason_and_wrong_revision_operations() {
    let repository = staged_repository();
    let mut store = ModelTransactionStore::new(repository.path()).expect("transaction store");
    let unknown = DraftId(u64::MAX);
    assert!(!store.discard(unknown));
    assert!(matches!(
        store.update(
            unknown,
            DraftChange {
                path: instance_path("missing"),
                source: None,
            },
        ),
        Err(TransactionError::UnknownDraft(id)) if id == unknown
    ));
    assert!(matches!(
        store.preview(unknown),
        Err(TransactionError::UnknownDraft(id)) if id == unknown
    ));
    assert!(matches!(
        store.commit(unknown, "missing", " "),
        Err(TransactionError::Validation(_))
    ));
    assert!(matches!(
        store.commit(unknown, "missing", "unknown draft"),
        Err(TransactionError::UnknownDraft(id)) if id == unknown
    ));

    let draft = store.create().expect("draft");
    assert!(matches!(
        store.commit(draft.id, "wrong-revision", "wrong optimistic token"),
        Err(TransactionError::StaleRevision { expected, .. }) if expected == "wrong-revision"
    ));
}

#[test]
fn deleting_an_instance_commits_atomically_and_updates_generated_authority() {
    let repository = staged_repository();
    let mut store = ModelTransactionStore::new(repository.path()).expect("transaction store");
    let draft = store.create().expect("draft");
    let path = instance_path("body-preparation-return-water");
    assert!(repository.path().join(&path).is_file());
    store
        .update(
            draft.id,
            DraftChange {
                path: path.clone(),
                source: None,
            },
        )
        .expect("stage instance deletion");
    let preview = store.preview(draft.id).expect("deletion preview");
    assert!(preview.diagnostics.is_empty(), "{:?}", preview.diagnostics);
    assert!(!preview.removed_objects.is_empty());
    let commit = store
        .commit(
            draft.id,
            &draft.base_revision,
            "remove test blueprint instance",
        )
        .expect("atomic deletion commit");
    assert!(commit.changed_paths.contains(&path));
    assert!(!repository.path().join(path).exists());
}

#[test]
fn startup_recovery_rejects_schema_escape_and_cross_directory_staging() {
    fn write_journal(repository: &tempfile::TempDir, journal: serde_json::Value) {
        fs::write(
            repository.path().join(".hearthline-transaction.json"),
            serde_json::to_vec_pretty(&journal).expect("journal JSON"),
        )
        .expect("journal fixture");
    }

    let repository = staged_repository();
    let target = repository.path().join("project/config/probe.yaml");
    write_journal(
        &repository,
        serde_json::json!({
            "schema_version": "9.9.9",
            "entries": [{
                "target": target,
                "staged": null,
                "backup": repository.path().join("project/config/probe.backup"),
            }],
        }),
    );
    assert!(matches!(
        ModelTransactionStore::new(repository.path()),
        Err(TransactionError::Validation(message)) if message.contains("unsupported transaction journal schema")
    ));

    let repository = staged_repository();
    write_journal(
        &repository,
        serde_json::json!({
            "schema_version": "0.1.0",
            "entries": [{
                "target": repository.path().join("project/config/../config/probe.yaml"),
                "staged": null,
                "backup": repository.path().join("project/config/probe.backup"),
            }],
        }),
    );
    assert!(matches!(
        ModelTransactionStore::new(repository.path()),
        Err(TransactionError::Validation(message)) if message.contains("not normalized")
    ));

    for (staged, backup) in [
        (
            Some("packages/web/src/generated/probe.staged"),
            "project/config/probe.backup",
        ),
        (None, "packages/web/src/generated/probe.backup"),
    ] {
        let repository = staged_repository();
        write_journal(
            &repository,
            serde_json::json!({
                "schema_version": "0.1.0",
                "entries": [{
                    "target": repository.path().join("project/config/probe.yaml"),
                    "staged": staged.map(|path| repository.path().join(path)),
                    "backup": repository.path().join(backup),
                }],
            }),
        );
        assert!(matches!(
            ModelTransactionStore::new(repository.path()),
            Err(TransactionError::Validation(message))
                if message.contains("must share the target directory")
        ));
    }
}
