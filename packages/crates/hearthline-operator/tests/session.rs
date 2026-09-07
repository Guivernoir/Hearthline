use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use hearthline_operator::{
    OperatorCommand, OperatorCommandError, OperatorProjection, OperatorSession, ProjectionStatus,
};
use hearthline_project::ProjectCompiler;
use hearthline_sim::{
    ModelRevision, QUANTIZATION_CONTRACT_VERSION, REPLAY_SCHEMA_VERSION, ReplayArtifact,
    ReplayOutcome, RunManifest, SessionCapacityPolicy, SimulationSession,
};

fn revision(value: &str) -> ModelRevision {
    ModelRevision {
        digest: value.into(),
        compiler_version: "0.3.2".into(),
    }
}

#[test]
fn operator_session_submits_commands_without_owning_plant_state() {
    let mut session = OperatorSession::with_capacity(
        "operator-01",
        "operator",
        revision("revision-a"),
        ["operate".into()],
        4,
        3,
    )
    .expect("reviewed queue");
    session
        .submit(
            "revision-a",
            100,
            OperatorCommand::Start {
                target: "water-train".into(),
            },
        )
        .expect("authorized command");
    let command = session.pop_command().expect("queued command");
    assert_eq!(command.sequence, 0);
    assert_eq!(command.model_revision, "revision-a");
    assert!(session.projection().is_none());
}

#[test]
fn operator_session_rejects_stale_or_unauthorized_commands() {
    let mut session = OperatorSession::new(
        "operator-01",
        "operator",
        revision("revision-a"),
        ["operate".into()],
    );
    assert!(matches!(
        session.submit(
            "revision-b",
            0,
            OperatorCommand::Start {
                target: "cell".into(),
            },
        ),
        Err(OperatorCommandError::StaleRevision { .. })
    ));
    assert!(matches!(
        session.submit(
            "revision-a",
            0,
            OperatorCommand::Reset {
                target: "cell".into(),
            },
        ),
        Err(OperatorCommandError::PermissionDenied(_))
    ));
}

#[test]
fn project_runtime_snapshot_projection_and_replay_fit_a_constrained_worker_stack() {
    std::thread::Builder::new()
        .name("foundation-stack-probe".into())
        .stack_size(512 * 1024)
        .spawn(|| {
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../project/config");
            let project = Arc::new(
                ProjectCompiler::new(root)
                    .compile_locked()
                    .expect("locked project on constrained stack"),
            );
            let simulation = SimulationSession::build(
                project.clone(),
                SessionCapacityPolicy::reviewed_default(),
            )
            .expect("runtime construction");
            let snapshot = simulation.snapshot().expect("full project snapshot");
            let projection = OperatorProjection {
                model_revision: project.digest().into(),
                captured_at_us: 0,
                status: ProjectionStatus::Current,
                target: "canonical-project".into(),
                state: BTreeMap::from([("snapshot-digest-bytes".into(), 64)]),
                labels: BTreeMap::new(),
                active_alarms: Vec::new(),
            };
            let mut operator = OperatorSession::new(
                "stack-probe",
                "test",
                simulation.revision().clone(),
                ["operate".into()],
            );
            operator.update_projection(projection);
            assert_eq!(
                operator.projection().unwrap().status,
                ProjectionStatus::Current
            );

            ReplayArtifact {
                schema_version: REPLAY_SCHEMA_VERSION.into(),
                manifest: RunManifest {
                    schema_version: REPLAY_SCHEMA_VERSION.into(),
                    model_digest: project.digest().into(),
                    simulation_version: env!("CARGO_PKG_VERSION").into(),
                    scenario: "stack-probe".into(),
                    quantization_contract: QUANTIZATION_CONTRACT_VERSION.into(),
                    clock_policy: "fixed-step".into(),
                    clock_step_us: 1_000,
                    seed: 0,
                    initial_state_digest: snapshot.digest(),
                    event_limit: 1,
                    limits: BTreeMap::from([("post-seal-allocations".into(), 0)]),
                    expected_outcomes: vec!["stack-probe-passed".into()],
                    inputs: Vec::new(),
                },
                checkpoints: Vec::new(),
                outcome: ReplayOutcome {
                    status: "passed".into(),
                    final_digest: snapshot.digest(),
                    event_count: 0,
                    alarms: Vec::new(),
                    metrics: BTreeMap::new(),
                },
            }
            .normalize()
            .expect("replay normalization");
        })
        .expect("constrained worker")
        .join()
        .expect("constrained worker completed");
}
