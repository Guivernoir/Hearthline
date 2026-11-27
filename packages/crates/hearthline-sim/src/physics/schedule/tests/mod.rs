use std::io::{self, Cursor};
use std::sync::Arc;

use hearthline_config::physics::{PhysicsProofCase, PhysicsProofConfig};
use hearthline_project::physics::schedule::{PhysicsInstanceIdentity, PhysicsInstanceRegistration};
use hearthline_project::physics::{CompiledPhysicsProof, DEFAULT_PHYSICS_BUDGET_BYTES};
use sha2::{Digest, Sha256};

use super::*;
use crate::physics::{PhysicsProofSession, PhysicsReplay};

mod diagnostics;
pub(in crate::physics) mod mixed;
mod previous;

fn compressible(fails: bool) -> Arc<CompiledPhysicsProof> {
    let mut config: PhysicsProofConfig = serde_yaml_ng::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../project/physics/proofs/compressible-receiver.yaml"
    )))
    .unwrap();
    config.steps = 6;
    if fails {
        config.maximum_iterations = 1;
        config.residual_tolerance = "0.000000001".into();
        let PhysicsProofCase::CompressibleReceiver { flow } = &mut config.case else {
            unreachable!()
        };
        flow.maximum_substeps = 1;
    }
    Arc::new(CompiledPhysicsProof::compile(config).unwrap())
}

fn receiver() -> Arc<CompiledPhysicsProof> {
    let mut config: PhysicsProofConfig = serde_yaml_ng::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../project/physics/proofs/receiver-equilibration.yaml"
    )))
    .unwrap();
    config.timestep_s = "0.5".into();
    config.steps = 6;
    Arc::new(CompiledPhysicsProof::compile(config).unwrap())
}

fn drying() -> Arc<CompiledPhysicsProof> {
    let mut config: PhysicsProofConfig = serde_yaml_ng::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../project/physics/proofs/drying-specimen.yaml"
    )))
    .unwrap();
    config.steps = 3;
    Arc::new(CompiledPhysicsProof::compile(config).unwrap())
}

fn registration(id: &str, model: Arc<CompiledPhysicsProof>) -> PhysicsInstanceRegistration {
    PhysicsInstanceRegistration {
        identity: PhysicsInstanceIdentity::new("factory", "body-preparation", id).unwrap(),
        model: model.into(),
    }
}

fn registrations() -> Vec<PhysicsInstanceRegistration> {
    let air = receiver();
    vec![
        registration("receiver-b", Arc::clone(&air)),
        registration("specimen", drying()),
        registration("receiver-a", air),
    ]
}

fn compile(registrations: Vec<PhysicsInstanceRegistration>) -> Arc<CompiledPhysicsSchedule> {
    Arc::new(CompiledPhysicsSchedule::compile(registrations, DEFAULT_PHYSICS_BUDGET_BYTES).unwrap())
}

fn run(schedule: &mut PhysicsSchedule) -> Vec<PhysicsEvent> {
    let mut events = Vec::with_capacity(schedule.plan.total_events() as usize);
    let allocation = allocation_counter::measure(|| {
        while !schedule.complete() {
            let result = schedule.step_next().unwrap();
            assert_eq!(result.sequence + 1, schedule.committed_events());
            events.push(result.event);
        }
    });
    assert_eq!(allocation.count_total, 0);
    events
}

fn capture(schedule: &mut PhysicsSchedule) -> Vec<u8> {
    let bound = schedule.memory.maximum_snapshot_frame_bytes
        * schedule.plan.instances().len() as u64
        + 8192;
    let mut bytes = Vec::with_capacity(bound as usize);
    let allocation = allocation_counter::measure(|| schedule.write_snapshot(&mut bytes).unwrap());
    assert_eq!(allocation.count_total, 0);
    bytes
}

#[test]
fn source_order_does_not_change_profiles_slots_digest_or_event_order() {
    let original = registrations();
    let mut reversed = original.clone();
    reversed.reverse();
    let plan = compile(original);
    let reverse_plan = compile(reversed);
    assert_eq!(plan.digest(), reverse_plan.digest());
    assert_eq!(plan.profiles().len(), 2);
    assert_eq!(plan.total_events(), 15);
    assert_eq!(plan.horizon_us(), 3_000_000);
    assert!(plan.event_capacity() * 3 >= plan.instances().len() * 4);
    for (a, b) in plan.instances().iter().zip(reverse_plan.instances()) {
        assert_eq!(a.identity, b.identity);
        assert_eq!((a.profile, a.slot), (b.profile, b.slot));
    }
    let mut expected = Vec::new();
    for (instance, binding) in plan.instances().iter().enumerate() {
        let model = &plan.profiles()[binding.profile];
        for step in 1..=model.steps() {
            expected.push(PhysicsEvent {
                time_us: u64::from(step) * model.timestep_us(),
                instance,
            });
        }
    }
    expected.sort_unstable();
    let mut middle = PhysicsSchedule::new(Arc::clone(&plan)).unwrap();
    for _ in 0..4 {
        middle.step_next().unwrap();
    }
    let mut first = PhysicsSchedule::new(plan).unwrap();
    let initial_digest = first.state_digest().unwrap();
    let mut second = PhysicsSchedule::new(reverse_plan).unwrap();
    assert_eq!(run(&mut first), expected);
    assert_eq!(run(&mut second), expected);
    assert_eq!(first.time_us(), 3_000_000);
    assert_eq!(capture(&mut first), capture(&mut second));
    let middle_digest = middle.state_digest().unwrap();
    let final_digest = first.state_digest().unwrap();
    let golden: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../project/physics/runtime/schedule/current.json"
    )))
    .unwrap();
    assert_eq!(
        serde_json::json!({
            "schema_version": "0.2.0",
            "model_digest": first.plan.digest(),
            "initial_digest": std::str::from_utf8(&initial_digest).unwrap(),
            "after_four_events_digest": std::str::from_utf8(&middle_digest).unwrap(),
            "final_digest": std::str::from_utf8(&final_digest).unwrap(),
        }),
        golden
    );
    assert!(matches!(first.step_next(), Err(ScheduleError::Complete)));
}

#[test]
fn complete_spatial_fields_match_independent_runs_on_a_constrained_stack() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let replay: PhysicsReplay = serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../project/physics/replays/wet-elastic-zero.json"
            )))
            .unwrap();
            let wet = Arc::new(
                CompiledPhysicsProof::compile_with_resources(
                    replay.config,
                    replay.geometry,
                    replay.material,
                )
                .unwrap(),
            );
            let mut instances = registrations();
            instances.push(registration("compressible", compressible(false)));
            instances.push(registration("wet-solid-a", Arc::clone(&wet)));
            instances.push(registration("wet-solid-b", wet));
            let plan = compile(instances);
            let mut runtime = None;
            let allocation = allocation_counter::measure(|| {
                runtime = Some(PhysicsSchedule::new(Arc::clone(&plan)).unwrap())
            });
            let mut runtime = runtime.unwrap();
            assert_eq!(
                allocation.bytes_total,
                runtime.memory.construction_allocation_bytes
            );
            while runtime.time_us() < 10_000_000 {
                runtime.step_next().unwrap();
            }
            let saved = capture(&mut runtime);
            let mut restored =
                PhysicsSchedule::read_snapshot(Arc::clone(&plan), Cursor::new(&saved)).unwrap();
            assert_eq!(capture(&mut restored), saved);
            assert_eq!(run(&mut runtime), run(&mut restored));
            assert_eq!(
                runtime.state_digest().unwrap(),
                restored.state_digest().unwrap()
            );
            assert_eq!(runtime.time_us(), plan.horizon_us());
            for (instance, binding) in plan.instances().iter().enumerate() {
                let model = &plan.profiles()[binding.profile];
                let mut independent =
                    PhysicsProofSession::new(Arc::clone(model.proof().unwrap())).unwrap();
                for _ in 0..model.steps() {
                    independent.step().unwrap();
                }
                assert_eq!(
                    runtime.checkpoint(instance).unwrap(),
                    independent.checkpoint()
                );
            }
            let allocation = allocation_counter::measure(|| {
                runtime.write_snapshot(io::sink()).unwrap();
                runtime.state_digest().unwrap();
            });
            assert_eq!(allocation.count_total, 0);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn snapshot_continuation_preserves_a_partially_committed_timestamp() {
    let plan = compile(registrations());
    for split in 0..=plan.total_events() {
        let mut runtime = PhysicsSchedule::new(Arc::clone(&plan)).unwrap();
        for _ in 0..split {
            runtime.step_next().unwrap();
        }
        let saved = capture(&mut runtime);
        let mut restored = None;
        let allocation = allocation_counter::measure(|| {
            restored = Some(
                PhysicsSchedule::read_snapshot(Arc::clone(&plan), Cursor::new(&saved)).unwrap(),
            );
        });
        let mut restored = restored.unwrap();
        assert!(
            allocation.bytes_max
                <= runtime.memory.construction_allocation_bytes
                    + runtime.memory.snapshot_decoder_bytes
        );
        assert_eq!(capture(&mut restored), saved);
        assert_eq!(runtime.next_event(), restored.next_event());
        assert_eq!(run(&mut runtime), run(&mut restored));
        assert_eq!(
            runtime.state_digest().unwrap(),
            restored.state_digest().unwrap()
        );
    }
}

#[test]
fn failure_retains_the_next_event_and_replays_the_same_failed_attempt() {
    let healthy = compressible(false);
    let plan = compile(vec![
        registration("a-healthy", healthy),
        registration("b-failed", compressible(true)),
    ]);
    let mut runtime = PhysicsSchedule::new(Arc::clone(&plan)).unwrap();
    runtime.step_next().unwrap();
    let before = [
        runtime.checkpoint(0).unwrap(),
        runtime.checkpoint(1).unwrap(),
    ];
    let pending = runtime.next_event();
    let allocation = allocation_counter::measure(|| {
        assert!(matches!(runtime.step_next(), Err(ScheduleError::Failed(_))));
        let failure = runtime.failure().unwrap();
        assert_eq!(Some(failure.event), pending);
        assert!(
            matches!(runtime.step_next(), Err(ScheduleError::Failed(repeated)) if repeated == failure)
        );
    });
    assert_eq!(allocation.count_total, 0);
    assert_eq!(runtime.committed_events(), 1);
    assert_eq!(runtime.time_us(), 100);
    for (instance, expected) in before.iter().enumerate() {
        assert_eq!(&runtime.checkpoint(instance).unwrap(), expected);
    }
    let expected: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../project/physics/runtime/schedule/failure-current.json"
    )))
    .unwrap();
    let state_digest = runtime.state_digest().unwrap();
    assert_eq!(
        serde_json::json!({
            "schema_version": "0.2.0", "model_digest": runtime.plan.digest(),
            "state_digest": std::str::from_utf8(&state_digest).unwrap(),
            "failure": diagnostic::FailureRecord::from(runtime.failure().unwrap()),
        }),
        expected
    );
    let saved = capture(&mut runtime);
    let forged = rewrite(&saved, |records| {
        records[0]["failed_event"]["diagnostic"]["attempts"] = 99.into()
    });
    assert!(
        PhysicsSchedule::read_snapshot(Arc::clone(&plan), Cursor::new(forged))
            .err()
            .unwrap()
            .contains("did not recur")
    );
    let mut restored = PhysicsSchedule::read_snapshot(plan, Cursor::new(&saved)).unwrap();
    assert_eq!(restored.failure(), runtime.failure());
    assert_eq!(capture(&mut restored), saved);
}

fn rewrite(bytes: &[u8], change: impl FnOnce(&mut [serde_json::Value])) -> Vec<u8> {
    let mut records: Vec<serde_json::Value> = bytes
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    records.pop();
    change(&mut records);
    for child in (2..records.len()).step_by(2) {
        records[child - 1]["bytes"] =
            (serde_json::to_vec(&records[child]).unwrap().len() + 1).into();
    }
    let mut output = Vec::new();
    for record in records {
        serde_json::to_writer(&mut output, &record).unwrap();
        output.push(b'\n');
    }
    let hash = crate::physics::checkpoint::hex_digest(Sha256::digest(&output).into());
    let hash = std::str::from_utf8(&hash).unwrap();
    serde_json::to_writer(&mut output, &serde_json::json!({"sha256": hash})).unwrap();
    output.push(b'\n');
    output
}

#[test]
fn rehashed_individual_checkpoints_cannot_forge_a_scheduler_frontier_or_failure() {
    let plan = compile(registrations());
    let mut runtime = PhysicsSchedule::new(Arc::clone(&plan)).unwrap();
    runtime.step_next().unwrap();
    let saved = capture(&mut runtime);
    let changed = rewrite(&saved, |records| records.swap(2, 4));
    assert!(
        PhysicsSchedule::read_snapshot(Arc::clone(&plan), Cursor::new(changed))
            .err()
            .unwrap()
            .contains("chronological prefix")
    );
    for field in ["time_us", "committed_events"] {
        let changed = rewrite(&saved, |records| records[0][field] = 0.into());
        assert!(PhysicsSchedule::read_snapshot(Arc::clone(&plan), Cursor::new(changed)).is_err());
    }
    let changed = rewrite(&saved, |records| {
        records[0]["failed_event"] = serde_json::json!({
            "event": runtime.next_event().unwrap(), "diagnostic": {"kind": "unknown-instance"}
        })
    });
    assert!(
        PhysicsSchedule::read_snapshot(Arc::clone(&plan), Cursor::new(changed))
            .err()
            .unwrap()
            .contains("did not recur")
    );
    let changed = rewrite(&saved, |records| {
        records[0]["failed_event"] = serde_json::json!({"time_us": 1, "instance": 999})
    });
    assert!(PhysicsSchedule::read_snapshot(Arc::clone(&plan), Cursor::new(changed)).is_err());
    let changed = rewrite(&saved, |records| {
        records[0]["schema_version"] = "99.0.0".into()
    });
    assert!(PhysicsSchedule::read_snapshot(Arc::clone(&plan), Cursor::new(changed)).is_err());
    let mut changed = saved.clone();
    changed.extend_from_slice(b"trailing");
    assert!(PhysicsSchedule::read_snapshot(Arc::clone(&plan), Cursor::new(changed)).is_err());
    assert!(
        PhysicsSchedule::read_snapshot(Arc::clone(&plan), Cursor::new(&saved[..saved.len() - 1]))
            .is_err()
    );
    let changed = rewrite(&saved, |records| {
        records[2]["first_field_raw"][0] = 0.into()
    });
    assert!(PhysicsSchedule::read_snapshot(plan, Cursor::new(changed)).is_err());
}

#[test]
fn compiler_and_runtime_reject_missing_identity_capacity_and_budget_evidence() {
    assert!(PhysicsInstanceIdentity::new("", "cell", "instance").is_err());
    assert!(CompiledPhysicsSchedule::compile(Vec::new(), DEFAULT_PHYSICS_BUDGET_BYTES).is_err());
    assert!(CompiledPhysicsSchedule::compile(registrations(), 0).is_err());
    let mut duplicate = registrations();
    duplicate.push(duplicate[0].clone());
    assert!(CompiledPhysicsSchedule::compile(duplicate, DEFAULT_PHYSICS_BUDGET_BYTES).is_err());
    let small = Arc::new(CompiledPhysicsSchedule::compile(registrations(), 1024).unwrap());
    assert!(!ScheduleMemoryPlan::compile(&small).unwrap().within_budget);
    assert!(PhysicsSchedule::new(small).is_err());
    let plan = compile(registrations());
    let memory = ScheduleMemoryPlan::compile(&plan).unwrap();
    assert!(memory.within_budget && memory.snapshot_within_budget);
    let tight_budget = memory.total_planned_bytes * 4 / 3 + 4;
    let tight = Arc::new(CompiledPhysicsSchedule::compile(registrations(), tight_budget).unwrap());
    let mut runtime = PhysicsSchedule::new(Arc::clone(&tight)).unwrap();
    assert!(!runtime.memory.snapshot_within_budget);
    assert!(runtime.write_snapshot(io::sink()).is_err());
    assert!(PhysicsSchedule::read_snapshot(tight, Cursor::new([])).is_err());
    assert!(runtime.completed_steps(999).is_err());
    assert!(runtime.checkpoint(999).is_err());
}

#[test]
fn failure_diagnostic_schema_preserves_all_numerical_error_variants() {
    use crate::physics::PhysicsStepError;
    use hearthline_physics::{PhysicsError, Scalar};
    let event = PhysicsEvent {
        time_us: 10,
        instance: 0,
    };
    for (cause, kind) in [
        (PhysicsError::Overflow, "overflow"),
        (PhysicsError::DivisionByZero, "division-by-zero"),
        (PhysicsError::InvalidDecimal, "invalid-decimal"),
        (PhysicsError::InvalidGeometry, "invalid-geometry"),
        (PhysicsError::InvalidCoefficient, "invalid-coefficient"),
        (
            PhysicsError::InsufficientResolution,
            "insufficient-resolution",
        ),
        (PhysicsError::InvalidState, "invalid-state"),
        (PhysicsError::BufferLength, "buffer-length"),
        (PhysicsError::NonPositiveDefinite, "non-positive-definite"),
        (
            PhysicsError::NonConvergence {
                iterations: 9,
                residual: Scalar::from_raw(17),
            },
            "non-convergence",
        ),
    ] {
        let record = diagnostic::FailureRecord::from(ScheduleFailure {
            event,
            error: ScheduleInstanceError::Proof(PhysicsStepError::Numerical(cause)),
        });
        let json = serde_json::to_value(record).unwrap();
        assert_eq!(json["diagnostic"]["cause"]["kind"], kind);
        let restored: diagnostic::FailureRecord = serde_json::from_value(json).unwrap();
        assert_eq!(restored, record);
    }
    let record = diagnostic::FailureRecord::from(ScheduleFailure {
        event,
        error: ScheduleInstanceError::UnknownInstance,
    });
    assert_eq!(
        serde_json::to_value(record).unwrap()["diagnostic"]["kind"],
        "unknown-instance"
    );
}
