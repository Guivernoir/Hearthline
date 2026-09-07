use std::collections::BTreeMap;

use hearthline_model::{ComponentId, PartitionMessage, Text};
use hearthline_sim::{
    CELL_SNAPSHOT_SCHEMA_VERSION, CONDUIT_OVERLOAD_SCENARIO, CellRegistration, CellSnapshot,
    ComponentSnapshot, ConduitConfig, ConduitOverflow, ContractScenarioError, ProjectSnapshot,
    QUANTIZATION_CONTRACT_VERSION, REPLAY_SCHEMA_VERSION, ReplayArtifact, ReplayCheckpoint,
    ReplayDivergence, ReplayError, ReplayOutcome, ReplayVerifier, RunInput, RunManifest,
    SnapshotValue, StableScheduler, run_conduit_overload_contract,
};

fn artifact(schema: &str, component_digest: &str) -> ReplayArtifact {
    ReplayArtifact {
        schema_version: schema.into(),
        manifest: RunManifest {
            schema_version: schema.into(),
            model_digest: "a".repeat(64),
            simulation_version: "0.3.2".into(),
            scenario: "test-scenario".into(),
            quantization_contract: QUANTIZATION_CONTRACT_VERSION.into(),
            clock_policy: "fixed-step".into(),
            clock_step_us: 1_000,
            seed: 0,
            initial_state_digest: "b".repeat(64),
            event_limit: 10,
            limits: BTreeMap::from([("post-seal-allocations".into(), 0)]),
            expected_outcomes: vec!["test-passed".into()],
            inputs: Vec::new(),
        },
        checkpoints: vec![ReplayCheckpoint {
            event_index: 1,
            time_us: 1_000,
            project_digest: "c".repeat(64),
            component_digests: BTreeMap::from([(
                "cell/controller".into(),
                component_digest.into(),
            )]),
            field_digests: BTreeMap::from([(
                "cell/controller".into(),
                BTreeMap::from([("mode".into(), component_digest.into())]),
            )]),
            snapshot: None,
        }],
        outcome: ReplayOutcome {
            status: "passed".into(),
            final_digest: "d".repeat(64),
            event_count: 1,
            alarms: Vec::new(),
            metrics: BTreeMap::new(),
        },
    }
}

#[test]
fn previous_replay_schema_migrates_in_memory() {
    let migrated = artifact("0.2.0", "e").normalize().expect("previous schema");
    assert_eq!(migrated.schema_version, REPLAY_SCHEMA_VERSION);
    assert_eq!(migrated.manifest.schema_version, REPLAY_SCHEMA_VERSION);
}

#[test]
fn verifier_names_first_divergent_component_and_field() {
    let expected = artifact(REPLAY_SCHEMA_VERSION, "expected")
        .normalize()
        .unwrap();
    let actual = artifact(REPLAY_SCHEMA_VERSION, "actual")
        .normalize()
        .unwrap();
    let error = ReplayVerifier::verify(&expected, &actual).expect_err("must diverge");
    let ReplayError::Diverged(divergence) = error else {
        panic!("expected structured divergence");
    };
    assert_eq!(divergence.event_index, 1);
    assert_eq!(divergence.component, "cell/controller");
    assert_eq!(divergence.field, "mode");
}

#[test]
fn manifest_normalization_orders_inputs_and_rejects_nondeterministic_contracts() {
    let mut candidate = artifact(REPLAY_SCHEMA_VERSION, "state");
    candidate.manifest.inputs = vec![
        RunInput::Fault {
            at_us: 20,
            target: "pump".into(),
            fault: "stalled".into(),
            active: true,
        },
        RunInput::Command {
            at_us: 10,
            source: "operator".into(),
            target: "controller".into(),
            command: "start".into(),
            values: BTreeMap::new(),
        },
    ];
    let normalized = candidate.normalize().expect("normalized manifest");
    assert_eq!(normalized.manifest.inputs[0].at_us(), 10);
    assert_eq!(normalized.manifest.inputs[1].at_us(), 20);
    assert_eq!(normalized.manifest.digest().len(), 64);
    assert_eq!(normalized.digest().len(), 64);

    let mut invalid = artifact(REPLAY_SCHEMA_VERSION, "state");
    invalid.manifest.model_digest = "short".into();
    assert!(matches!(invalid.normalize(), Err(ReplayError::Invalid(_))));

    let mut invalid = artifact(REPLAY_SCHEMA_VERSION, "state");
    invalid.manifest.quantization_contract = "unknown".into();
    assert!(matches!(invalid.normalize(), Err(ReplayError::Invalid(_))));

    for (clock_step_us, event_limit) in [(0, 1), (1, 0)] {
        let mut invalid = artifact(REPLAY_SCHEMA_VERSION, "state");
        invalid.manifest.clock_step_us = clock_step_us;
        invalid.manifest.event_limit = event_limit;
        assert!(matches!(invalid.normalize(), Err(ReplayError::Invalid(_))));
    }

    let mut invalid = artifact(REPLAY_SCHEMA_VERSION, "state");
    invalid.manifest.clock_policy = "wall-clock".into();
    assert!(matches!(invalid.normalize(), Err(ReplayError::Invalid(_))));

    let invalid = artifact("9.9.9", "state");
    assert!(matches!(invalid.normalize(), Err(ReplayError::Invalid(_))));
}

#[test]
fn replay_artifacts_persist_migrate_and_reject_duplicate_checkpoints() {
    let temporary = tempfile::tempdir().expect("temporary replay folder");
    let path = temporary.path().join("replay.json");
    let expected = artifact(PREVIOUS_SCHEMA, "persisted")
        .normalize()
        .expect("previous replay migrates");
    expected.write(&path).expect("write replay");
    assert_eq!(ReplayArtifact::load(&path).expect("load replay"), expected);

    let missing = ReplayArtifact::load(temporary.path().join("missing.json"));
    assert!(matches!(missing, Err(ReplayError::Io(_))));
    let malformed = temporary.path().join("malformed.json");
    std::fs::write(&malformed, "not-json").expect("malformed fixture");
    assert!(matches!(
        ReplayArtifact::load(malformed),
        Err(ReplayError::Invalid(_))
    ));

    let mut duplicate = artifact(REPLAY_SCHEMA_VERSION, "state");
    duplicate.checkpoints.push(duplicate.checkpoints[0].clone());
    assert!(matches!(
        duplicate.normalize(),
        Err(ReplayError::Invalid(_))
    ));
}

const PREVIOUS_SCHEMA: &str = "0.2.0";

#[test]
fn checkpoint_capture_hashes_component_fields_from_full_snapshots() {
    let site = Text::try_new("factory").expect("site");
    let cell = Text::try_new("forming").expect("cell");
    let mut scheduler = StableScheduler::new();
    scheduler
        .register_cell(CellRegistration {
            site: site.clone(),
            cell: cell.clone(),
            component_demand: 1,
            link_demand: 0,
            operational: true,
        })
        .expect("cell registration");
    scheduler.seal().expect("scheduler seal");
    let snapshot = ProjectSnapshot::capture(
        "a".repeat(64),
        &scheduler,
        vec![CellSnapshot {
            schema_version: CELL_SNAPSHOT_SCHEMA_VERSION.into(),
            site,
            cell,
            captured_at_us: 0,
            components: vec![ComponentSnapshot {
                schema_version: CELL_SNAPSHOT_SCHEMA_VERSION.into(),
                component: "controller".into(),
                component_kind: "virtual-controller".into(),
                state: BTreeMap::from([
                    ("enabled".into(), SnapshotValue::Boolean(true)),
                    ("phase".into(), SnapshotValue::Text("run".into())),
                ]),
            }],
        }],
    )
    .expect("project snapshot");
    let checkpoint = ReplayCheckpoint::capture(7, &snapshot);
    assert_eq!(checkpoint.event_index, 7);
    assert_eq!(checkpoint.component_digests.len(), 1);
    assert_eq!(
        checkpoint.field_digests["factory/forming/controller"].len(),
        2
    );

    for corrupt in [
        |checkpoint: &mut ReplayCheckpoint| checkpoint.time_us = 1,
        |checkpoint: &mut ReplayCheckpoint| checkpoint.project_digest = "0".repeat(64),
        |checkpoint: &mut ReplayCheckpoint| {
            checkpoint
                .component_digests
                .insert("factory/forming/controller".into(), "0".repeat(64));
        },
        |checkpoint: &mut ReplayCheckpoint| {
            checkpoint.field_digests.insert(
                "factory/forming/controller".into(),
                BTreeMap::from([("enabled".into(), "0".repeat(64))]),
            );
        },
    ] {
        let mut invalid = artifact(REPLAY_SCHEMA_VERSION, "state");
        invalid.checkpoints = vec![checkpoint.clone()];
        corrupt(&mut invalid.checkpoints[0]);
        assert!(matches!(
            invalid.normalize(),
            Err(ReplayError::Invalid(detail)) if detail.contains("full snapshot")
        ));
    }
}

#[test]
fn project_snapshot_digest_covers_complete_pending_message_payloads() {
    fn snapshot(counter: u64) -> ProjectSnapshot {
        let site = Text::try_new("factory").unwrap();
        let source = Text::try_new("source").unwrap();
        let destination = Text::try_new("destination").unwrap();
        let mut scheduler = StableScheduler::new();
        for cell in [&source, &destination] {
            scheduler
                .register_cell(CellRegistration {
                    site: site.clone(),
                    cell: cell.clone(),
                    component_demand: 1,
                    link_demand: 1,
                    operational: true,
                })
                .unwrap();
        }
        scheduler
            .add_conduit(ConduitConfig {
                id: "source-to-destination".into(),
                source_site: site.clone(),
                source_cell: source.clone(),
                destination_site: site.clone(),
                destination_cell: destination.clone(),
                latency_us: 10,
                queue_capacity: 4,
                reviewed_burst: 3,
                overflow: ConduitOverflow::RejectNewest,
            })
            .unwrap();
        scheduler.seal().unwrap();
        scheduler
            .send(
                &site,
                &source,
                &site,
                &destination,
                PartitionMessage::Heartbeat {
                    source: ComponentId::new("pump-01").unwrap(),
                    counter,
                },
            )
            .unwrap();
        ProjectSnapshot::capture("same-model", &scheduler, Vec::new()).unwrap()
    }

    let first = snapshot(1);
    let second = snapshot(999);
    assert_ne!(
        first.conduits[0].queue_digest,
        second.conduits[0].queue_digest
    );
    assert_ne!(first.digest(), second.digest());
}

#[test]
fn project_snapshot_reader_migrates_every_nested_previous_schema() {
    let site = Text::try_new("factory").unwrap();
    let cell = Text::try_new("cell").unwrap();
    let mut scheduler = StableScheduler::new();
    scheduler
        .register_cell(CellRegistration {
            site: site.clone(),
            cell: cell.clone(),
            component_demand: 1,
            link_demand: 0,
            operational: true,
        })
        .unwrap();
    scheduler.seal().unwrap();
    let current = ProjectSnapshot::capture(
        "a".repeat(64),
        &scheduler,
        vec![CellSnapshot {
            schema_version: "0.1.0".into(),
            site: site.clone(),
            cell: cell.clone(),
            captured_at_us: 0,
            components: vec![ComponentSnapshot {
                schema_version: "0.1.0".into(),
                component: "sensor".into(),
                component_kind: "field-sensor".into(),
                state: BTreeMap::from([(
                    "samples".into(),
                    SnapshotValue::IntegerList(vec![1, 2, 3]),
                )]),
            }],
        }],
    )
    .expect("previous nested snapshot migration");
    let json = current.to_json().expect("snapshot JSON");
    assert!(json.ends_with('\n'));
    assert_eq!(ProjectSnapshot::from_json(&json).unwrap(), current);
    assert!(ProjectSnapshot::from_json("not-json").is_err());

    let mut invalid = current.clone();
    invalid.schema_version = "9.9.9".into();
    assert!(invalid.to_json().is_err());
    let mut invalid = current.clone();
    invalid.cells[0].components[0].schema_version = "9.9.9".into();
    assert!(invalid.normalize().is_err());
    let mut duplicate = current.clone();
    duplicate.cells.push(duplicate.cells[0].clone());
    assert!(duplicate.normalize().is_err());
    let mut duplicate = current.cells[0].clone();
    duplicate.components.push(duplicate.components[0].clone());
    assert!(duplicate.normalize().is_err());
}

#[test]
fn verifier_reports_every_divergence_boundary() {
    let expected = artifact(REPLAY_SCHEMA_VERSION, "same").normalize().unwrap();
    assert_eq!(
        ReplayVerifier::verify(&expected, &expected)
            .expect("identical replay")
            .status,
        "passed"
    );

    let mut actual = expected.clone();
    actual.manifest.scenario = "different".into();
    assert_divergence(
        ReplayVerifier::verify(&expected, &actual),
        "manifest",
        "normalized-run-manifest",
    );

    let mut actual = expected.clone();
    actual.checkpoints[0].project_digest = "different".into();
    assert_divergence(
        ReplayVerifier::verify(&expected, &actual),
        "project",
        "checkpoint",
    );

    let mut actual = expected.clone();
    actual.checkpoints[0]
        .component_digests
        .insert("cell/controller".into(), "changed".into());
    actual.checkpoints[0]
        .field_digests
        .get_mut("cell/controller")
        .expect("component fields")
        .clear();
    assert_divergence(
        ReplayVerifier::verify(&expected, &actual),
        "cell/controller",
        "mode",
    );

    let mut expected_extra = expected.clone();
    expected_extra.checkpoints[0]
        .field_digests
        .insert("cell/controller".into(), BTreeMap::new());
    let mut actual_extra = expected_extra.clone();
    actual_extra.checkpoints[0].field_digests.insert(
        "cell/controller".into(),
        BTreeMap::from([("extra".into(), "x".into())]),
    );
    actual_extra.checkpoints[0]
        .component_digests
        .insert("cell/controller".into(), "changed".into());
    assert_divergence(
        ReplayVerifier::verify(&expected_extra, &actual_extra),
        "cell/controller",
        "extra",
    );

    let mut actual = expected.clone();
    actual.checkpoints[0].component_digests.clear();
    assert_divergence(
        ReplayVerifier::verify(&expected, &actual),
        "cell/controller",
        "component-state",
    );

    let mut actual = expected.clone();
    actual.checkpoints.clear();
    assert!(matches!(
        ReplayVerifier::verify(&expected, &actual),
        Err(ReplayError::Invalid(_))
    ));

    let mut actual = expected.clone();
    actual.outcome.final_digest = "changed".into();
    assert_divergence(
        ReplayVerifier::verify(&expected, &actual),
        "project",
        "final-outcome",
    );

    for error in [
        ReplayError::Io("disk".into()),
        ReplayError::Invalid("schema".into()),
        ReplayError::Diverged(ReplayDivergence {
            event_index: 1,
            component: "component".into(),
            field: "field".into(),
            expected: "a".into(),
            actual: "b".into(),
        }),
    ] {
        assert!(!error.to_string().is_empty());
    }
}

fn assert_divergence(result: Result<ReplayOutcome, ReplayError>, component: &str, field: &str) {
    let Err(ReplayError::Diverged(divergence)) = result else {
        panic!("expected structured replay divergence");
    };
    assert_eq!(divergence.component, component);
    assert_eq!(divergence.field, field);
}

#[test]
fn component_digest_includes_full_runtime_state() {
    let component = |runtime_state: &str| ComponentSnapshot {
        schema_version: CELL_SNAPSHOT_SCHEMA_VERSION.into(),
        component: "edge-router".into(),
        component_kind: "router".into(),
        state: BTreeMap::from([
            ("neighbor-count".into(), SnapshotValue::Integer(1)),
            (
                "runtime-state".into(),
                SnapshotValue::Text(runtime_state.into()),
            ),
        ]),
    };

    let first = component(r#"{"neighbors":[{"address":"10.0.0.1"}]}"#);
    let second = component(r#"{"neighbors":[{"address":"10.0.0.2"}]}"#);
    assert_ne!(first.digest(), second.digest());
}

#[test]
fn overload_contract_records_each_policy_and_replays_deterministically() {
    let model = "a".repeat(64);
    let first = run_conduit_overload_contract(&model, "0.3.2").expect("overload contract");
    let second = run_conduit_overload_contract(&model, "0.3.2").expect("repeat contract");
    assert_eq!(first, second);
    assert_eq!(first.manifest.scenario, CONDUIT_OVERLOAD_SCENARIO);
    assert_eq!(first.outcome.status, "passed");
    assert_eq!(first.outcome.event_count, 20);
    assert_eq!(first.outcome.metrics["reject-rejected"], 1);
    assert_eq!(first.outcome.metrics["drop-dropped"], 1);
    assert_eq!(first.outcome.metrics["coalesce-coalesced"], 1);
    assert_eq!(first.outcome.metrics["shutdown-stopped"], 1);
    assert_eq!(first.outcome.metrics["shutdown-recoveries"], 1);
    ReplayVerifier::verify(&first, &second).expect("deterministic overload replay");

    for error in [
        ContractScenarioError::Contract("failed".into()),
        ContractScenarioError::Replay(ReplayError::Invalid("invalid".into())),
        ContractScenarioError::Scheduler(hearthline_sim::SchedulerError::NotSealed),
    ] {
        assert!(!error.to_string().is_empty());
    }
}
