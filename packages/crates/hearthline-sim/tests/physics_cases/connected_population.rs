use std::io::{self, Cursor};
use std::sync::Arc;

use hearthline_config::physics::connected::ConnectedGasDocument;
use hearthline_project::physics::DEFAULT_PHYSICS_BUDGET_BYTES;
use hearthline_project::physics::flow::connected::CompiledConnectedGas;
use hearthline_sim::physics::flow::connected::population::{
    ConnectedGasPopulation, ConnectedPopulationError, ConnectedPopulationMemory,
    ConnectedPopulationWork,
};
use hearthline_sim::physics::flow::connected::{ConnectedGasSession, ConnectedStepError};
use serde_json::Value;

fn golden() -> Value {
    let value: Value = serde_json::from_str(include_str!(
        "../../../../../project/physics/runtime/connected-population.json"
    ))
    .unwrap();
    assert_eq!(value["schema_version"], "0.1.0");
    value
}

fn model(fail: bool, budget: Option<u64>) -> Arc<CompiledConnectedGas> {
    let mut source = ConnectedGasDocument::from_yaml(include_str!(
        "../../../../../project/physics/runtime/connected-source.yaml"
    ))
    .unwrap();
    if fail {
        source.integration.maximum_iterations = 1;
    }
    if let Some(budget) = budget {
        source.allocation_budget_bytes = budget;
    }
    Arc::new(CompiledConnectedGas::compile_document(&source, DEFAULT_PHYSICS_BUDGET_BYTES).unwrap())
}

#[test]
fn pooled_domains_match_independent_sessions_after_every_interleaved_step_without_allocation() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let model = model(false, None);
            let golden = golden();
            assert_eq!(model.digest(), golden["model_digest"]);
            assert_eq!(golden["instances"], 3);
            let mut pool = ConnectedGasPopulation::new(Arc::clone(&model), 3).unwrap();
            let mut independent: Vec<_> = (0..3)
                .map(|_| ConnectedGasSession::new(Arc::clone(&model)).unwrap())
                .collect();
            assert_ne!(
                pool.fields(0).unwrap().as_ptr(),
                pool.fields(1).unwrap().as_ptr()
            );
            let initial = pool.state_digest().unwrap();
            assert_eq!(
                initial,
                golden["initial_digest"].as_str().unwrap().as_bytes()
            );
            assert_eq!(
                golden["interleaved_slots"],
                serde_json::json!([2, 0, 2, 1, 1, 0, 2, 0, 1])
            );
            for (sequence, slot) in [2, 0, 2, 1, 1, 0, 2, 0, 1].into_iter().enumerate() {
                let expected = independent[slot].step_next().unwrap();
                let measured = allocation_counter::measure(|| {
                    assert_eq!(pool.step(slot).unwrap(), expected);
                    pool.write_snapshot(io::sink()).unwrap();
                    pool.state_digest().unwrap();
                });
                assert_eq!(measured.count_total, 0);
                for (slot, direct) in independent.iter().enumerate() {
                    assert_eq!(pool.completed_steps(slot).unwrap(), direct.step());
                    assert_eq!(pool.fields(slot).unwrap(), direct.fields());
                    assert_eq!(pool.ranges(slot).unwrap(), direct.ranges());
                    assert_eq!(
                        pool.instance_digest(slot).unwrap(),
                        direct.state_digest().unwrap()
                    );
                }
                let mut snapshot = Vec::new();
                pool.write_snapshot(&mut snapshot).unwrap();
                let mut restored = ConnectedGasPopulation::read_snapshot(
                    Arc::clone(&model),
                    Cursor::new(snapshot),
                )
                .unwrap();
                assert_eq!(
                    restored.state_digest().unwrap(),
                    pool.state_digest().unwrap()
                );
                pool = restored;
                if sequence == 3 {
                    assert_eq!(golden["intermediate_after_events"], sequence + 1);
                    assert_eq!(
                        pool.state_digest().unwrap(),
                        golden["intermediate_digest"].as_str().unwrap().as_bytes()
                    );
                }
            }
            assert_eq!(
                pool.state_digest().unwrap(),
                golden["final_digest"].as_str().unwrap().as_bytes()
            );
            assert_eq!(
                pool.step(0),
                Err(ConnectedPopulationError::Step(ConnectedStepError::Complete))
            );
            assert_eq!(pool.step(3), Err(ConnectedPopulationError::UnknownInstance));
            assert!(
                pool.write_instance_snapshot(usize::MAX, io::sink())
                    .is_err()
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn fault_latches_remain_per_instance_when_the_shared_workspace_changes_owners() {
    let model = model(true, None);
    let golden = golden();
    assert_eq!(model.digest(), golden["failure_model_digest"]);
    assert_eq!(golden["failed_slot"], 0);
    let initial = ConnectedGasSession::new(Arc::clone(&model))
        .unwrap()
        .state_digest()
        .unwrap();
    let mut pool = ConnectedGasPopulation::new(Arc::clone(&model), 3).unwrap();
    let measured = allocation_counter::measure(|| {
        assert!(pool.step(0).is_err());
        assert!(pool.failure(0).unwrap().is_some());
        assert_eq!(pool.failure(1).unwrap(), None);
        assert_eq!(pool.instance_digest(1).unwrap(), initial);
        assert!(pool.step(0).is_err());
        pool.write_snapshot(io::sink()).unwrap();
    });
    assert_eq!(measured.count_total, 0);
    assert_eq!(pool.retry_failure(0).unwrap().unwrap().attempts, 7);
    assert_eq!(pool.retry_failure(1).unwrap(), None);
    let before = pool.state_digest().unwrap();
    assert_eq!(
        before,
        golden["failure_digest"].as_str().unwrap().as_bytes()
    );
    let mut bytes = Vec::new();
    pool.write_snapshot(&mut bytes).unwrap();
    let mut restored = ConnectedGasPopulation::read_snapshot(model, Cursor::new(bytes)).unwrap();
    assert_eq!(restored.state_digest().unwrap(), before);
    assert!(restored.failure(0).unwrap().is_some());
    assert_eq!(restored.failure(1).unwrap(), None);
    assert_eq!(restored.instance_digest(1).unwrap(), initial);
    assert!(restored.step(0).is_err());
    assert_eq!(restored.completed_steps(0).unwrap(), 0);
}

#[test]
fn memory_plans_count_the_bank_shared_workspace_and_transactional_restore_peak() {
    let model = model(false, None);
    let memory = ConnectedPopulationMemory::compile(&model, 16).unwrap();
    let mut pool = None;
    let measured = allocation_counter::measure(|| {
        pool = Some(ConnectedGasPopulation::new(Arc::clone(&model), 16).unwrap())
    });
    let mut pool = pool.unwrap();
    assert!(measured.bytes_max as u64 <= memory.total_planned_bytes);
    assert_eq!(pool.memory(), &memory);
    let work = ConnectedPopulationWork::compile(&model, 16).unwrap();
    assert_eq!(pool.work(), &work);
    assert_eq!(work.physical_steps, 16 * u64::from(model.profile().steps));
    assert_eq!(
        work.maximum_solver_evaluations,
        work.physical_steps * model.evaluation_bound()
    );
    assert_eq!(
        work.maximum_restore_solver_evaluations,
        16 * model.evaluation_bound()
    );
    assert!(ConnectedPopulationWork::compile(&model, 0).is_err());
    assert!(ConnectedPopulationWork::compile(&model, u64::MAX).is_err());
    assert!(
        ConnectedPopulationWork::compile(&model, u64::MAX / u64::from(model.profile().steps))
            .is_err()
    );
    assert_eq!(memory.state_bank_bytes, 16 * model.memory().field_bytes / 4);
    assert!(
        memory.total_planned_bytes
            < model.memory().immutable_bytes + 16 * model.memory().field_bytes
    );
    let mut bytes = Vec::new();
    pool.write_snapshot(&mut bytes).unwrap();
    let measured = allocation_counter::measure(|| {
        let mut restored =
            ConnectedGasPopulation::read_snapshot(Arc::clone(&model), Cursor::new(&bytes)).unwrap();
        assert_eq!(
            restored.state_digest().unwrap(),
            pool.state_digest().unwrap()
        );
    });
    assert!(measured.bytes_max as u64 <= memory.restore_peak_bytes);
    assert!(ConnectedPopulationMemory::compile(&model, 0).is_err());
    assert!(ConnectedPopulationMemory::compile(&model, u64::MAX).is_err());
    assert!(ConnectedGasPopulation::new(Arc::clone(&model), usize::MAX).is_err());
    let too_small = (memory.total_planned_bytes * 4).div_ceil(3) - 1;
    let constrained = model_with_budget(too_small);
    assert!(ConnectedGasPopulation::new(constrained, 16).is_err());
}
fn model_with_budget(budget: u64) -> Arc<CompiledConnectedGas> {
    model(false, Some(budget))
}

#[test]
fn rejected_slot_restore_does_not_publish_fields_or_clear_an_existing_fault() {
    let model = model(true, None);
    let mut pool = ConnectedGasPopulation::new(Arc::clone(&model), 2).unwrap();
    assert!(pool.step(0).is_err());
    let before = pool.state_digest().unwrap();
    let mut invalid = Vec::new();
    pool.write_instance_snapshot(1, &mut invalid).unwrap();
    invalid[0] = b'!';
    assert!(pool.restore_instance(0, Cursor::new(&invalid)).is_err());
    assert_eq!(pool.state_digest().unwrap(), before);
    assert!(pool.restore_instance(2, Cursor::new(&invalid)).is_err());
    let mut valid = Vec::new();
    pool.write_instance_snapshot(0, &mut valid).unwrap();
    pool.restore_instance(1, Cursor::new(valid)).unwrap();
    assert_eq!(
        pool.instance_digest(0).unwrap(),
        pool.instance_digest(1).unwrap()
    );
    assert!(pool.failure(1).unwrap().is_some());
}

#[test]
fn snapshot_framing_order_digest_counts_and_trailing_input_are_validated() {
    let model = model(false, None);
    let mut pool = ConnectedGasPopulation::new(Arc::clone(&model), 2).unwrap();
    pool.step(1).unwrap();
    let mut bytes = Vec::new();
    pool.write_snapshot(&mut bytes).unwrap();
    let original = std::str::from_utf8(&bytes).unwrap();
    let frame: Value = serde_json::from_str(original.lines().nth(1).unwrap()).unwrap();
    let length = frame["bytes"].as_u64().unwrap();
    for (key, value) in [
        ("instance", Value::from(1)),
        ("bytes", Value::from(0)),
        ("bytes", Value::from(u64::MAX)),
        ("bytes", Value::from(length - 1)),
        ("bytes", Value::from(length + 1)),
    ] {
        let mut rows: Vec<_> = original.lines().map(str::to_owned).collect();
        let mut frame: Value = serde_json::from_str(&rows[1]).unwrap();
        frame[key] = value;
        rows[1] = serde_json::to_string(&frame).unwrap();
        assert!(
            ConnectedGasPopulation::read_snapshot(
                Arc::clone(&model),
                Cursor::new(rows.join("\n") + "\n")
            )
            .is_err()
        );
    }
    for count in [0, u64::MAX] {
        let mut rows: Vec<_> = original.lines().map(str::to_owned).collect();
        let mut header: Value = serde_json::from_str(&rows[0]).unwrap();
        header["instances"] = Value::from(count);
        rows[0] = serde_json::to_string(&header).unwrap();
        assert!(
            ConnectedGasPopulation::read_snapshot(
                Arc::clone(&model),
                Cursor::new(rows.join("\n") + "\n")
            )
            .is_err()
        );
    }
    let wrong_contract = original.replace(
        "\"normalized_child_schema\":\"0.2.0\"",
        "\"normalized_child_schema\":\"99.0.0\"",
    );
    assert!(
        ConnectedGasPopulation::read_snapshot(Arc::clone(&model), Cursor::new(wrong_contract))
            .is_err()
    );
    let mut broken = bytes.clone();
    let index = broken.len() - 5;
    broken[index] ^= 1;
    assert!(
        ConnectedGasPopulation::read_snapshot(Arc::clone(&model), Cursor::new(broken)).is_err()
    );
    assert!(
        ConnectedGasPopulation::read_snapshot(
            Arc::clone(&model),
            Cursor::new(&bytes[..bytes.len() - 1])
        )
        .is_err()
    );
    bytes.push(b'X');
    assert!(ConnectedGasPopulation::read_snapshot(model, Cursor::new(bytes)).is_err());
}

#[test]
fn output_failure_does_not_mutate_any_accepted_instance() {
    struct RejectAfter(usize);
    impl io::Write for RejectAfter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.0 == 0 {
                return Err(io::Error::other("injected output failure"));
            }
            let count = bytes.len().min(self.0);
            self.0 -= count;
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut pool = ConnectedGasPopulation::new(model(false, None), 2).unwrap();
    pool.step(1).unwrap();
    let before = pool.state_digest().unwrap();
    assert!(pool.write_snapshot(RejectAfter(500)).is_err());
    assert_eq!(pool.state_digest().unwrap(), before);
}
