use super::*;
use crate::physics::flow::connected::ConnectedGasSession;
use hearthline_config::physics::connected::ConnectedGasDocument;
use hearthline_project::physics::flow::connected::CompiledConnectedGas;
use hearthline_project::physics::schedule::{
    CompiledNumericalProfile, MIXED_PHYSICS_SCHEDULE_CONTRACT,
};
use std::io::{BufRead, Read};

fn golden() -> serde_json::Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../project/physics/runtime/schedule/mixed.json"
    )))
    .unwrap()
}

fn gas(fails: bool) -> Arc<CompiledConnectedGas> {
    gas_with_budget(fails, None)
}

fn gas_with_budget(fails: bool, budget: Option<u64>) -> Arc<CompiledConnectedGas> {
    let mut source = ConnectedGasDocument::from_yaml(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../project/physics/runtime/connected-source.yaml"
    )))
    .unwrap();
    source.integration.steps = 6;
    if let Some(budget) = budget {
        source.allocation_budget_bytes = budget;
    }
    if fails {
        source.integration.maximum_iterations = 1;
    }
    Arc::new(CompiledConnectedGas::compile_document(&source, DEFAULT_PHYSICS_BUDGET_BYTES).unwrap())
}

fn registrations(fails: bool) -> Vec<PhysicsInstanceRegistration> {
    let air = gas(false);
    let dry = drying();
    [
        (
            "b-air",
            if fails { gas(true) } else { Arc::clone(&air) }.into(),
        ),
        ("c-specimen", dry.into()),
        ("a-air", air.into()),
    ]
    .into_iter()
    .map(|(id, model)| PhysicsInstanceRegistration {
        identity: PhysicsInstanceIdentity::new("factory", "process", id).unwrap(),
        model,
    })
    .collect()
}

pub(in crate::physics) fn plan(fails: bool) -> Arc<CompiledPhysicsSchedule> {
    compile(registrations(fails))
}

enum Reference {
    Proof(Box<PhysicsProofSession>),
    Gas(Box<ConnectedGasSession>),
}
impl Reference {
    fn new(model: &CompiledNumericalProfile) -> Self {
        match model {
            CompiledNumericalProfile::Proof(m) => {
                Self::Proof(Box::new(PhysicsProofSession::new(Arc::clone(m)).unwrap()))
            }
            CompiledNumericalProfile::ConnectedGas(m) => {
                Self::Gas(Box::new(ConnectedGasSession::new(Arc::clone(m)).unwrap()))
            }
        }
    }
    fn step(&mut self) {
        match self {
            Self::Proof(s) => {
                s.step().unwrap();
            }
            Self::Gas(s) => {
                s.step_next().unwrap();
            }
        }
    }
    fn digest(&self) -> [u8; 64] {
        match self {
            Self::Proof(s) => s.state_digest().unwrap(),
            Self::Gas(s) => s.state_digest().unwrap(),
        }
    }
    fn fields(&self) -> crate::physics::NumericalFieldRanges {
        match self {
            Self::Proof(s) => s.field_ranges(),
            Self::Gas(s) => s.ranges(),
        }
    }
}

#[test]
fn mixed_profiles_preserve_order_fields_and_every_restart_boundary_without_step_allocation() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let plan = plan(false);
            assert_eq!(plan.contract(), MIXED_PHYSICS_SCHEDULE_CONTRACT);
            assert_eq!(plan.profiles().len(), 2);
            assert_eq!(plan.total_events(), 15);
            assert_eq!(plan.horizon_us(), 3_000_000);
            let mut reversed = registrations(false);
            reversed.reverse();
            assert_eq!(compile(reversed).digest(), plan.digest());
            let mut reference: Vec<_> = plan
                .instances()
                .iter()
                .map(|b| Reference::new(&plan.profiles()[b.profile]))
                .collect();
            let mut expected = Vec::new();
            for (instance, b) in plan.instances().iter().enumerate() {
                let m = &plan.profiles()[b.profile];
                for step in 1..=m.steps() {
                    expected.push(PhysicsEvent {
                        time_us: u64::from(step) * m.timestep_us(),
                        instance,
                    });
                }
            }
            expected.sort_unstable();
            let mut runtime = None;
            let allocation = allocation_counter::measure(|| {
                runtime = Some(PhysicsSchedule::new(Arc::clone(&plan)).unwrap())
            });
            let mut runtime = runtime.unwrap();
            assert_eq!(
                allocation.bytes_total,
                runtime.memory().construction_allocation_bytes
            );
            let initial = runtime.state_digest().unwrap();
            let mut fourth = initial;
            for (index, event) in expected.into_iter().enumerate() {
                assert_eq!(runtime.next_event(), Some(event));
                reference[event.instance].step();
                let allocation = allocation_counter::measure(|| {
                    let result = runtime.step_next().unwrap();
                    assert_eq!(result.event, event);
                    assert_eq!(result.fields, reference[event.instance].fields());
                    assert_eq!(result.sequence, index as u64);
                    for (instance, r) in reference.iter().enumerate() {
                        assert_eq!(runtime.instance_digest(instance).unwrap(), r.digest());
                    }
                    runtime.write_snapshot(io::sink()).unwrap();
                });
                assert_eq!(allocation.count_total, 0);
                let saved = capture(&mut runtime);
                let mut restored = None;
                let allocation = allocation_counter::measure(|| {
                    restored = Some(
                        PhysicsSchedule::read_snapshot(Arc::clone(&plan), Cursor::new(&saved))
                            .unwrap(),
                    )
                });
                assert!(
                    allocation.bytes_max
                        <= runtime.memory().construction_allocation_bytes
                            + runtime.memory().snapshot_decoder_bytes
                );
                let mut restored = restored.unwrap();
                assert_eq!(capture(&mut restored), saved);
                runtime = restored;
                if index == 3 {
                    fourth = runtime.state_digest().unwrap();
                }
            }
            assert!(runtime.complete());
            assert!(matches!(
                runtime.checkpoint(0),
                Err(ScheduleInstanceError::WrongProfile)
            ));
            let golden = golden();
            assert_eq!(golden["schema_version"], "0.2.0");
            assert_eq!(golden["model_digest"], plan.digest());
            assert_eq!(
                golden["initial_digest"],
                std::str::from_utf8(&initial).unwrap()
            );
            assert_eq!(
                golden["after_four_events_digest"],
                std::str::from_utf8(&fourth).unwrap()
            );
            assert_eq!(
                golden["final_digest"],
                std::str::from_utf8(&runtime.state_digest().unwrap()).unwrap()
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

type Records = (serde_json::Value, Vec<(serde_json::Value, Vec<u8>)>);
fn records(bytes: &[u8]) -> Records {
    let mut input = Cursor::new(bytes);
    let mut row = String::new();
    input.read_line(&mut row).unwrap();
    let header: serde_json::Value = serde_json::from_str(&row).unwrap();
    let mut frames = Vec::new();
    for _ in 0..header["instances"].as_u64().unwrap() {
        row.clear();
        input.read_line(&mut row).unwrap();
        let frame: serde_json::Value = serde_json::from_str(&row).unwrap();
        let mut payload = vec![0; frame["bytes"].as_u64().unwrap() as usize];
        input.read_exact(&mut payload).unwrap();
        frames.push((frame, payload));
    }
    (header, frames)
}
fn rehash((header, frames): Records) -> Vec<u8> {
    let mut output = Vec::new();
    serde_json::to_writer(&mut output, &header).unwrap();
    output.push(b'\n');
    for (frame, bytes) in frames {
        serde_json::to_writer(&mut output, &frame).unwrap();
        output.push(b'\n');
        output.extend_from_slice(&bytes);
    }
    let digest = crate::physics::checkpoint::hex_digest(Sha256::digest(&output).into());
    serde_json::to_writer(
        &mut output,
        &serde_json::json!({"sha256": std::str::from_utf8(&digest).unwrap()}),
    )
    .unwrap();
    output.push(b'\n');
    output
}

#[test]
fn mixed_frames_cannot_forge_kind_length_order_or_chronology() {
    let plan = plan(false);
    let mut runtime = PhysicsSchedule::new(Arc::clone(&plan)).unwrap();
    runtime.step_next().unwrap();
    let saved = capture(&mut runtime);
    for case in 0..7 {
        let (mut header, mut frames) = records(&saved);
        match case {
            0 => frames[0].0["kind"] = "proof".into(),
            1 => frames[0].0["instance"] = 1.into(),
            2 => frames[0].0["bytes"] = u64::MAX.into(),
            3 => frames[0].0["bytes"] = (frames[0].1.len() + 1).into(),
            4 => frames[0].0["bytes"] = (frames[0].1.len() - 1).into(),
            5 => header["schema_version"] = "0.1.0".into(),
            _ => {
                let first = frames[0].1.clone();
                frames[0].1 = frames[1].1.clone();
                frames[1].1 = first;
                for (f, b) in &mut frames {
                    f["bytes"] = b.len().into();
                }
            }
        }
        assert!(
            PhysicsSchedule::read_snapshot(
                Arc::clone(&plan),
                Cursor::new(rehash((header, frames)))
            )
            .is_err(),
            "case {case}"
        );
    }
}

#[test]
fn gas_failure_is_atomic_and_must_match_the_declared_scheduled_fault() {
    let plan = plan(true);
    let mut runtime = PhysicsSchedule::new(Arc::clone(&plan)).unwrap();
    runtime.step_next().unwrap();
    let accepted = runtime.instance_digest(0).unwrap();
    let unattempted = runtime.instance_digest(2).unwrap();
    let allocation = allocation_counter::measure(|| {
        assert!(matches!(
            runtime.step_next(),
            Err(ScheduleError::Failed(ScheduleFailure {
                error: ScheduleInstanceError::Connected { retry: Some(_), .. },
                ..
            }))
        ));
        assert_eq!(runtime.instance_digest(0).unwrap(), accepted);
        assert_eq!(runtime.instance_digest(2).unwrap(), unattempted);
        runtime.write_snapshot(io::sink()).unwrap();
    });
    assert_eq!(allocation.count_total, 0);
    assert_eq!(runtime.committed_events(), 1);
    assert_eq!(runtime.time_us(), 1001);
    let saved = capture(&mut runtime);
    let mut restored =
        PhysicsSchedule::read_snapshot(Arc::clone(&plan), Cursor::new(&saved)).unwrap();
    assert_eq!(restored.failure(), runtime.failure());
    assert_eq!(capture(&mut restored), saved);
    let golden = golden();
    assert_eq!(golden["failure_model_digest"], plan.digest());
    assert_eq!(
        golden["failure_digest"],
        std::str::from_utf8(&runtime.state_digest().unwrap()).unwrap()
    );
    assert_eq!(
        golden["failure"],
        serde_json::to_value(diagnostic::FailureRecord::from(runtime.failure().unwrap())).unwrap()
    );
    for case in 0..5 {
        let (mut header, mut frames) = records(&saved);
        match case {
            0 => header["failed_event"] = serde_json::Value::Null,
            1 => header["failed_event"]["event"]["instance"] = 0.into(),
            2 => header["failed_event"]["diagnostic"]["retry"]["attempts"] = 5.into(),
            3 => header["failed_event"]["event"]["time_us"] = 1002.into(),
            _ => {
                let mut fresh = PhysicsSchedule::new(Arc::clone(&plan)).unwrap();
                frames[1].1.clear();
                fresh.write_instance_snapshot(1, &mut frames[1].1).unwrap();
                frames[1].0["bytes"] = frames[1].1.len().into();
            }
        }
        assert!(
            PhysicsSchedule::read_snapshot(
                Arc::clone(&plan),
                Cursor::new(rehash((header, frames)))
            )
            .is_err()
        );
    }
}

#[test]
fn interrupted_mixed_capture_does_not_change_fields_faults_or_event_order() {
    struct FailsAfter(usize);
    impl io::Write for FailsAfter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.0 == 0 {
                return Err(io::Error::other("injected snapshot output failure"));
            }
            let count = bytes.len().min(self.0);
            self.0 -= count;
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    for fails in [false, true] {
        let plan = plan(fails);
        let mut runtime = PhysicsSchedule::new(Arc::clone(&plan)).unwrap();
        runtime.step_next().unwrap();
        if fails {
            assert!(runtime.step_next().is_err());
        }
        let saved = capture(&mut runtime);
        let event = runtime.next_event();
        let failure = runtime.failure();
        for bytes in [0, 1, saved.len() / 2, saved.len() - 1] {
            assert!(runtime.write_snapshot(FailsAfter(bytes)).is_err());
            assert_eq!(runtime.next_event(), event);
            assert_eq!(runtime.failure(), failure);
            assert_eq!(capture(&mut runtime), saved);
        }
        let mut independent = PhysicsSchedule::read_snapshot(plan, Cursor::new(saved)).unwrap();
        if !fails {
            assert_eq!(run(&mut runtime), run(&mut independent));
        }
        assert_eq!(
            runtime.state_digest().unwrap(),
            independent.state_digest().unwrap()
        );
    }
}

#[test]
fn shared_gas_profiles_cannot_discard_or_unpin_reviewed_budget_evidence() {
    let first = gas(false);
    let second = gas_with_budget(false, Some(first.memory().allocation_budget_bytes * 2));
    assert_eq!(first.digest(), second.digest());
    let registration = |id, model| PhysicsInstanceRegistration {
        identity: PhysicsInstanceIdentity::new("factory", "air", id).unwrap(),
        model,
    };
    let first_plan = compile(vec![registration("supply", Arc::clone(&first).into())]);
    let second_plan = compile(vec![registration("supply", Arc::clone(&second).into())]);
    assert_ne!(first_plan.digest(), second_plan.digest());
    let mut conflicting = vec![
        registration("a-supply", first.into()),
        registration("b-supply", second.into()),
    ];
    for _ in 0..2 {
        assert!(
            CompiledPhysicsSchedule::compile(conflicting.clone(), DEFAULT_PHYSICS_BUDGET_BYTES)
                .unwrap_err()
                .contains("same reviewed budget")
        );
        conflicting.reverse();
    }
}

#[test]
fn gas_only_schedule_enforces_aggregate_and_profile_restore_budgets() {
    let model = gas(false);
    let registration = |model| PhysicsInstanceRegistration {
        identity: PhysicsInstanceIdentity::new("factory", "air", "supply").unwrap(),
        model,
    };
    let plan = compile(vec![registration(Arc::clone(&model).into())]);
    let mut runtime = PhysicsSchedule::new(Arc::clone(&plan)).unwrap();
    runtime.step_next().unwrap();
    let saved = capture(&mut runtime);
    let mut restored = PhysicsSchedule::read_snapshot(plan, Cursor::new(&saved)).unwrap();
    assert_eq!(capture(&mut restored), saved);
    assert!(runtime.write_instance_snapshot(1, io::sink()).is_err());
    assert!(runtime.instance_digest(1).is_err());

    let memory = runtime.memory();
    let tight = Arc::new(
        CompiledPhysicsSchedule::compile(
            vec![registration(Arc::clone(&model).into())],
            memory.total_planned_bytes * 4 / 3 + 4,
        )
        .unwrap(),
    );
    let mut tight_runtime = PhysicsSchedule::new(Arc::clone(&tight)).unwrap();
    assert!(!tight_runtime.memory().snapshot_within_budget);
    assert!(tight_runtime.write_snapshot(io::sink()).is_err());
    assert!(PhysicsSchedule::read_snapshot(tight, Cursor::new(&saved)).is_err());

    let population =
        crate::physics::flow::connected::population::ConnectedPopulationMemory::compile(&model, 1)
            .unwrap();
    let load_peak = population
        .total_planned_bytes
        .max(model.memory().compiler_peak_bytes);
    assert!(load_peak < population.restore_peak_bytes);
    let restricted = gas_with_budget(false, Some(load_peak * 4 / 3 + 4));
    let restricted = compile(vec![registration(restricted.into())]);
    let mut restricted_runtime = PhysicsSchedule::new(Arc::clone(&restricted)).unwrap();
    assert!(restricted_runtime.memory().within_budget);
    assert!(!restricted_runtime.memory().snapshot_within_budget);
    assert!(restricted_runtime.write_snapshot(io::sink()).is_err());
    assert!(PhysicsSchedule::read_snapshot(restricted, Cursor::new(&saved)).is_err());
}
