use std::sync::Arc;

use hearthline_config::physics::{PhysicsProofCase, PhysicsProofConfig};
use hearthline_project::physics::CompiledPhysicsProof;
use hearthline_sim::physics::population::{
    PhysicsProofPopulation, PooledCapacityComparison, PopulationError, PopulationMemoryPlan,
    SnapshotLoadMemoryPlan,
};
use hearthline_sim::physics::{PhysicsProofSession, PhysicsReplay};

const PROOFS: [&str; 4] = [
    include_str!("../../../../project/physics/proofs/receiver-equilibration.yaml"),
    include_str!("../../../../project/physics/proofs/drying-specimen.yaml"),
    include_str!("../../../../project/physics/proofs/drying-deformation.yaml"),
    include_str!("../../../../project/physics/proofs/compressible-receiver.yaml"),
];
const GOLDENS: [&str; 4] = [
    include_str!("../../../../project/physics/replays/receiver-equilibration.json"),
    include_str!("../../../../project/physics/replays/drying-specimen.json"),
    include_str!("../../../../project/physics/replays/drying-deformation.json"),
    include_str!("../../../../project/physics/replays/compressible-receiver.json"),
];

fn model(source: &str) -> Arc<CompiledPhysicsProof> {
    Arc::new(CompiledPhysicsProof::compile(serde_yaml_ng::from_str(source).unwrap()).unwrap())
}

#[test]
fn interleaved_instances_match_independent_sessions_at_every_boundary() {
    for source in PROOFS {
        let model = model(source);
        let mut population = PhysicsProofPopulation::new(model.clone(), 3).unwrap();
        let mut independent = (0..3)
            .map(|_| PhysicsProofSession::new(model.clone()).unwrap())
            .collect::<Vec<_>>();
        for _ in 0..4 {
            for instance in [2, 0, 2, 1, 0, 2, 1] {
                population.step(instance).unwrap();
                independent[instance].step().unwrap();
                for (index, expected) in independent.iter().enumerate() {
                    assert_eq!(population.checkpoint(index).unwrap(), expected.checkpoint());
                }
            }
        }
        assert_eq!(population.checkpoint(0).unwrap().step, 8);
        assert_eq!(population.checkpoint(1).unwrap().step, 8);
        assert_eq!(population.checkpoint(2).unwrap().step, 12);
    }
}

#[test]
fn pooled_construction_and_steps_use_the_measured_storage_contract() {
    for source in PROOFS {
        let model = model(source);
        let memory = PopulationMemoryPlan::compile(&model, 32).unwrap();
        let mut population = None;
        let allocations = allocation_counter::measure(|| {
            population = Some(PhysicsProofPopulation::new(model.clone(), 32).unwrap());
        });
        assert_eq!(
            allocations.bytes_total,
            memory.construction_allocation_bytes
        );
        assert_eq!(
            memory.state_bank_bytes,
            32 * model.storage_layout().state_bytes
        );
        assert!(memory.construction_allocation_bytes < 32 * model.memory().field_buffer_bytes);
        let mut population = population.unwrap();
        assert_eq!(population.instance_count(), 32);
        assert_eq!(*population.memory(), memory);
        let allocations = allocation_counter::measure(|| {
            for _ in 0..3 {
                for instance in [31, 2, 0, 4, 17] {
                    population.step(instance).unwrap();
                }
            }
            assert_eq!(
                population.step(32).unwrap_err(),
                PopulationError::UnknownInstance
            );
        });
        assert_eq!(allocations.count_total, 0);
        assert_eq!(population.checkpoint(31).unwrap().step, 3);
        assert_eq!(population.checkpoint(1).unwrap().step, 0);
    }
}

#[test]
fn rejected_steps_never_publish_partial_fields_to_any_instance() {
    let mut config: PhysicsProofConfig = serde_yaml_ng::from_str(PROOFS[3]).unwrap();
    config.maximum_iterations = 1;
    config.residual_tolerance = "0.000000001".into();
    if let PhysicsProofCase::CompressibleReceiver { flow } = &mut config.case {
        flow.maximum_substeps = 1;
    }
    let model = Arc::new(CompiledPhysicsProof::compile(config).unwrap());
    let mut population = PhysicsProofPopulation::new(model, 4).unwrap();
    let initial = population.checkpoint(0).unwrap();
    let mut errors = [None; 4];
    let allocations = allocation_counter::measure(|| {
        for index in [3, 1, 0, 2] {
            errors[index] = Some(population.step(index).unwrap_err());
        }
    });
    assert_eq!(allocations.count_total, 0);
    for (index, error) in errors.iter().enumerate() {
        assert!(matches!(error, Some(PopulationError::Step(_))));
        assert_eq!(population.checkpoint(index).unwrap(), initial);
    }
}

#[test]
fn restoration_is_instance_local_and_rejections_are_atomic() {
    for source in PROOFS {
        let model = model(source);
        let mut population = PhysicsProofPopulation::new(model.clone(), 3).unwrap();
        let initial = population.checkpoint(0).unwrap();
        for _ in 0..4 {
            population.step(0).unwrap();
        }
        let advanced = population.checkpoint(0).unwrap();
        population.restore(1, &advanced).unwrap();
        assert_eq!(population.checkpoint(1).unwrap(), advanced);
        assert_eq!(population.checkpoint(2).unwrap(), initial);
        let mut bad = advanced.clone();
        bad.temperature_k_raw[0] = -1;
        assert!(population.restore(1, &bad).is_err());
        assert_eq!(population.checkpoint(1).unwrap(), advanced);
        assert_eq!(population.checkpoint(0).unwrap(), advanced);
        assert_eq!(population.checkpoint(2).unwrap(), initial);
        assert!(population.restore(3, &advanced).is_err());
        assert_eq!(
            population.checkpoint(3).unwrap_err(),
            PopulationError::UnknownInstance
        );
        population.step(1).unwrap();
        let mut independent = PhysicsProofSession::new(model).unwrap();
        independent.restore(&advanced).unwrap();
        independent.step().unwrap();
        assert_eq!(population.checkpoint(1).unwrap(), independent.checkpoint());
    }
}

#[test]
fn pooled_golden_replay_continuation_fits_one_mib_stack() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            for (source, golden) in PROOFS.into_iter().zip(GOLDENS) {
                let model = model(source);
                let artifact: PhysicsReplay = serde_json::from_str(golden).unwrap();
                let mut population = PhysicsProofPopulation::new(model, 3).unwrap();
                population.restore(1, &artifact.checkpoints[0]).unwrap();
                for step in 0..artifact.config.steps {
                    population.step(2).unwrap();
                    population.step(0).unwrap();
                    if step >= artifact.checkpoints[0].step {
                        population.step(1).unwrap();
                    }
                }
                for index in 0..3 {
                    assert_eq!(population.checkpoint(index).unwrap(), artifact.final_state);
                }
                let allocations = allocation_counter::measure(|| {
                    assert!(population.step(0).is_err());
                });
                assert_eq!(allocations.count_total, 0);
                assert_eq!(population.checkpoint(0).unwrap(), artifact.final_state);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn oversized_populations_are_rejected_before_allocating_any_fields() {
    let model = model(PROOFS[2]);
    assert!(PopulationMemoryPlan::compile(&model, 0).is_err());
    assert!(PopulationMemoryPlan::compile(&model, u64::MAX).is_err());
    let plan = PopulationMemoryPlan::compile(&model, 1_000_000).unwrap();
    assert!(!plan.within_budget);
    let allocations = allocation_counter::measure(|| {
        assert!(PhysicsProofPopulation::new(model.clone(), 1_000_000).is_err());
        assert!(PhysicsProofPopulation::new(model.clone(), 0).is_err());
    });
    assert!(allocations.bytes_total < 4096);
}

#[test]
fn streamed_snapshots_allocate_no_capture_buffers_and_resume_exactly() {
    use std::io::Cursor;
    for source in PROOFS {
        let model = model(source);
        let mut population = PhysicsProofPopulation::new(model.clone(), 3).unwrap();
        for instance in [0, 2, 2, 0, 2, 1] {
            population.step(instance).unwrap();
        }
        let memory = SnapshotLoadMemoryPlan::compile(&model, 3).unwrap();
        assert!(memory.within_budget);
        let mut bytes = vec![0; (memory.maximum_frame_bytes * 3 + 8192) as usize];
        let mut output = Cursor::new(bytes.as_mut_slice());
        let allocations = allocation_counter::measure(|| {
            population.write_snapshot(&mut output).unwrap();
        });
        assert_eq!(allocations.count_total, 0);
        let written = output.position() as usize;
        let bytes = &bytes[..written];
        let rows = bytes.split(|b| *b == b'\n').collect::<Vec<_>>();
        for instance in 0..3 {
            let checkpoint = serde_json::from_slice(rows[instance + 1]).unwrap();
            assert_eq!(population.checkpoint(instance).unwrap(), checkpoint);
        }
        let mut restored = None;
        let allocations = allocation_counter::measure(|| {
            restored = Some(
                PhysicsProofPopulation::read_snapshot(model.clone(), Cursor::new(bytes)).unwrap(),
            );
        });
        assert!(allocations.bytes_max <= memory.total_numerical_bytes);
        let mut restored = restored.unwrap();
        for instance in [1, 0, 2, 0, 2] {
            population.step(instance).unwrap();
            restored.step(instance).unwrap();
            assert_eq!(
                population.checkpoint(instance).unwrap(),
                restored.checkpoint(instance).unwrap()
            );
        }
    }
}

#[test]
fn compact_state_preserves_full_golden_checkpoint_bytes() {
    for index in [2, 3] {
        let model = model(PROOFS[index]);
        let layout = model.storage_layout();
        assert!(layout.state_bytes < layout.checkpoint_field_bytes);
        let golden: PhysicsReplay = serde_json::from_str(GOLDENS[index]).unwrap();
        let initial = PhysicsProofSession::new(model.clone())
            .unwrap()
            .checkpoint();
        let mut population = PhysicsProofPopulation::new(model, 2).unwrap();
        for _ in 0..golden.config.steps {
            population.step(1).unwrap();
        }
        let mut archive = Vec::new();
        population.write_snapshot(&mut archive).unwrap();
        let rows = archive.split(|b| *b == b'\n').collect::<Vec<_>>();
        let header: serde_json::Value = serde_json::from_slice(rows[0]).unwrap();
        assert_eq!(
            header["values_per_instance"],
            layout.checkpoint_field_bytes / 8
        );
        assert_eq!(rows[1], serde_json::to_vec(&initial).unwrap());
        assert_eq!(rows[2], serde_json::to_vec(&golden.final_state).unwrap());
        assert_eq!(population.checkpoint(0).unwrap(), initial);
        assert_eq!(population.checkpoint(1).unwrap(), golden.final_state);
    }
}

#[test]
fn partial_writes_and_small_read_buffers_preserve_snapshot_hashes() {
    use std::io::{BufReader, Cursor, Write};
    struct SmallWrites(Vec<u8>);
    impl Write for SmallWrites {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let n = bytes.len().min(3);
            self.0.extend_from_slice(&bytes[..n]);
            Ok(n)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let model = model(PROOFS[2]);
    let mut population = PhysicsProofPopulation::new(model.clone(), 2).unwrap();
    population.step(1).unwrap();
    let mut output = SmallWrites(Vec::new());
    population.write_snapshot(&mut output).unwrap();
    let mut direct = Vec::new();
    population.write_snapshot(&mut direct).unwrap();
    assert_eq!(direct, output.0);
    let mut restored = PhysicsProofPopulation::read_snapshot(
        model,
        BufReader::with_capacity(7, Cursor::new(output.0)),
    )
    .unwrap();
    assert_eq!(
        restored.checkpoint(1).unwrap(),
        population.checkpoint(1).unwrap()
    );
}

#[test]
fn corrupt_truncated_reordered_or_mismatched_archives_never_return_a_population() {
    use std::io::Cursor;
    let model = model(PROOFS[3]);
    let mut population = PhysicsProofPopulation::new(model.clone(), 2).unwrap();
    population.step(1).unwrap();
    let untouched = population.checkpoint(1).unwrap();
    let mut bytes = Vec::new();
    population.write_snapshot(&mut bytes).unwrap();
    let rows = bytes
        .split_inclusive(|b| *b == b'\n')
        .map(<[u8]>::to_vec)
        .collect::<Vec<_>>();
    for (key, bad) in [
        ("schema_version", serde_json::json!("99.0.0")),
        ("numerical_contract", serde_json::json!("unknown")),
        ("model_digest", serde_json::json!("invalid")),
        ("instances", serde_json::json!(u64::MAX)),
        ("instances", serde_json::json!(0)),
        ("instances", serde_json::json!(1)),
        ("values_per_instance", serde_json::json!(0)),
        ("unexpected", serde_json::json!(true)),
    ] {
        let mut changed = rows.clone();
        let mut header: serde_json::Value = serde_json::from_slice(&changed[0]).unwrap();
        header[key] = bad;
        changed[0] = serde_json::to_vec(&header).unwrap();
        changed[0].push(b'\n');
        assert!(
            PhysicsProofPopulation::read_snapshot(model.clone(), Cursor::new(changed.concat()))
                .is_err()
        );
    }
    for end in [0, 10, rows[0].len(), bytes.len() - 1] {
        assert!(
            PhysicsProofPopulation::read_snapshot(model.clone(), Cursor::new(&bytes[..end]))
                .is_err()
        );
    }
    let mut reordered = rows.clone();
    reordered.swap(1, 2);
    let result =
        PhysicsProofPopulation::read_snapshot(model.clone(), Cursor::new(reordered.concat()));
    assert!(result.err().unwrap().contains("aggregate digest"));
    let mut changed = rows.clone();
    let mut checkpoint: serde_json::Value = serde_json::from_slice(&changed[1]).unwrap();
    checkpoint["flow"]["conserved_raw"][0][0] = serde_json::json!(-1);
    changed[1] = serde_json::to_vec(&checkpoint).unwrap();
    changed[1].push(b'\n');
    assert!(
        PhysicsProofPopulation::read_snapshot(model.clone(), Cursor::new(changed.concat()))
            .is_err()
    );
    bytes.push(b' ');
    assert!(PhysicsProofPopulation::read_snapshot(model, Cursor::new(bytes)).is_err());
    assert_eq!(population.checkpoint(1).unwrap(), untouched);
}

#[test]
fn capture_failure_preserves_states_and_unrestorable_budgets_are_rejected() {
    use std::io::{Cursor, ErrorKind, Write};
    struct FailedWriter;
    impl Write for FailedWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(ErrorKind::WriteZero.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let source = model(PROOFS[2]);
    let mut population = PhysicsProofPopulation::new(source.clone(), 3).unwrap();
    let initial = population.checkpoint(0).unwrap();
    assert!(population.write_snapshot(FailedWriter).is_err());
    assert_eq!(population.checkpoint(0).unwrap(), initial);
    let memory = PopulationMemoryPlan::compile(&source, 3).unwrap();
    let mut config = source.config().clone();
    config.allocation_budget_bytes = memory.total_numerical_bytes.div_ceil(3) * 4;
    let limited = Arc::new(CompiledPhysicsProof::compile(config).unwrap());
    assert!(
        !SnapshotLoadMemoryPlan::compile(&limited, 3)
            .unwrap()
            .within_budget
    );
    let mut population = PhysicsProofPopulation::new(limited.clone(), 3).unwrap();
    let mut output = Vec::new();
    assert!(population.write_snapshot(&mut output).is_err());
    assert!(output.is_empty());
    let header = serde_json::json!({
        "schema_version": "0.1.0",
        "numerical_contract": hearthline_physics::NUMERICAL_CONTRACT,
        "model_digest": limited.digest(), "instances": 3,
        "values_per_instance": limited.storage_layout().checkpoint_field_bytes / 8,
    });
    let header = format!("{header}\n");
    let allocations = allocation_counter::measure(|| {
        assert!(
            PhysicsProofPopulation::read_snapshot(limited.clone(), Cursor::new(header.as_bytes()))
                .is_err()
        );
    });
    assert!(allocations.bytes_total < 8192);
}

#[test]
fn archive_frame_limits_reject_oversized_sources_before_state_allocation() {
    use std::io::Cursor;
    let mut config: PhysicsProofConfig = serde_yaml_ng::from_str(PROOFS[3]).unwrap();
    config.shape = [75; 3];
    let large = Arc::new(CompiledPhysicsProof::compile(config).unwrap());
    assert!(SnapshotLoadMemoryPlan::compile(&large, 1).is_err());
    let header = serde_json::json!({
        "schema_version": "0.1.0", "numerical_contract": hearthline_physics::NUMERICAL_CONTRACT,
        "model_digest": large.digest(), "instances": 1,
        "values_per_instance": large.storage_layout().checkpoint_field_bytes / 8,
    });
    let header = format!("{header}\n");
    let allocations = allocation_counter::measure(|| {
        assert!(
            PhysicsProofPopulation::read_snapshot(large.clone(), Cursor::new(header.as_bytes()))
                .is_err()
        );
    });
    assert!(allocations.bytes_total < 8192);
    let small = model(PROOFS[0]);
    let oversized_header = vec![b'a'; 4097];
    assert!(
        PhysicsProofPopulation::read_snapshot(small.clone(), Cursor::new(oversized_header))
            .is_err()
    );
    let mut bytes = Vec::new();
    PhysicsProofPopulation::new(small.clone(), 1)
        .unwrap()
        .write_snapshot(&mut bytes)
        .unwrap();
    let header_end = bytes.iter().position(|v| *v == b'\n').unwrap() + 1;
    bytes.truncate(header_end);
    let maximum = SnapshotLoadMemoryPlan::compile(&small, 1)
        .unwrap()
        .maximum_frame_bytes as usize;
    bytes.resize(header_end + maximum + 1, b'0');
    assert!(PhysicsProofPopulation::read_snapshot(small, Cursor::new(bytes)).is_err());
}

#[test]
fn factory_comparison_counts_real_pool_layouts_without_changing_workloads() {
    let config: hearthline_config::physics::capacity::PhysicsCapacityConfig =
        serde_yaml_ng::from_str(include_str!(
            "../../../../project/physics/capacity/factory-baseline.yaml"
        ))
        .unwrap();
    let report = PooledCapacityComparison::compile(config.clone()).unwrap();
    assert_eq!(report.baseline.total_planned_bytes, 7_373_303_672);
    assert_eq!(report.state_bank_bytes, 208_280_112);
    assert_eq!(report.slot_metadata_bytes, 817_080);
    assert_eq!(report.worker_buffer_bytes, 3_937_848);
    assert_eq!(report.immutable_geometry_bytes, 1_544_040);
    assert_eq!(report.archive_decoder_envelope_bytes, 12_783_386);
    assert_eq!(report.total_planned_bytes, 411_911_842);
    assert!(report.within_workload_capacities);
    assert!(report.within_budget);
    report.require_fit().unwrap();
    assert!(!report.baseline.qualification_gaps.is_empty());
    assert_eq!(report.baseline.snapshot_field_bytes, 2_877_994_080);
    let mut reordered = config;
    reordered.profiles.reverse();
    reordered.populations.reverse();
    assert_eq!(
        report,
        PooledCapacityComparison::compile(reordered.clone()).unwrap()
    );
    let profile = &report
        .profiles
        .iter()
        .find(|p| p.profile == "product-block")
        .unwrap();
    assert_eq!(profile.memory.instances, 2778);
    let mut limited = reordered;
    limited.allocation_budget_bytes = report.total_planned_bytes;
    let rejected = PooledCapacityComparison::compile(limited).unwrap();
    assert!(!rejected.within_budget);
    assert!(rejected.require_fit().is_err());
}
