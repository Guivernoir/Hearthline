use std::io::{self, Cursor};
use std::sync::Arc;
use std::time::{Duration, Instant};

use hearthline_model::ComponentId;
use hearthline_physics::flow::FlowBoundary;
use hearthline_physics::transport::SolverLimits;
use hearthline_physics::{PhysicsError, Scalar};
use hearthline_project::physics::flow::connected::{
    CompiledConnectedGas, ConnectedGasProfile, GasReference, GasRegion, RegionInitialGas,
};
use hearthline_project::physics::flow::mesh::{FlowPatch, FlowPatchConnection};
use hearthline_project::physics::schedule::PhysicsInstanceIdentity;
use hearthline_sim::physics::clock::PlaybackRate;
use hearthline_sim::physics::flow::connected::{ConnectedGasSession, ConnectedStepError};
use hearthline_sim::physics::worker::{
    ClockCommand, CommandReply, PhysicsWorker, PhysicsWorkerClient, WorkerMemoryPlan, WorkerPolicy,
};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[path = "connected_retries.rs"]
mod retries;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Golden {
    schema_version: String,
    model_digest: String,
    initial_digest: String,
    after_eight_steps_digest: String,
    final_digest: String,
    failure_model_digest: String,
    failure_digest: String,
}
fn golden() -> Golden {
    let golden: Golden = serde_json::from_str(include_str!(
        "../../../../../project/physics/runtime/connected-current.json"
    ))
    .unwrap();
    assert_eq!(golden.schema_version, "0.2.0");
    golden
}

#[test]
fn previous_snapshot_schema_migrates_without_changing_physical_fields() {
    let old: Golden = serde_json::from_str(include_str!(
        "../../../../../project/physics/runtime/connected-golden.json"
    ))
    .unwrap();
    assert_eq!(old.schema_version, "0.1.0");
    let records = [
        (
            include_bytes!("../../../../../project/physics/runtime/connected-v1/initial.ndjson")
                .as_slice(),
            0,
            false,
            old.initial_digest,
        ),
        (
            include_bytes!("../../../../../project/physics/runtime/connected-v1/eighth.ndjson")
                .as_slice(),
            8,
            false,
            old.after_eight_steps_digest,
        ),
        (
            include_bytes!("../../../../../project/physics/runtime/connected-v1/final.ndjson")
                .as_slice(),
            20,
            false,
            old.final_digest,
        ),
        (
            include_bytes!("../../../../../project/physics/runtime/connected-v1/failure.ndjson")
                .as_slice(),
            0,
            true,
            old.failure_digest,
        ),
    ];
    for (bytes, step, failed, expected) in records {
        let footer: Value =
            serde_json::from_str(std::str::from_utf8(bytes).unwrap().lines().last().unwrap())
                .unwrap();
        assert_eq!(footer["sha256"], expected);
        let model = model(failed);
        assert_eq!(
            model.digest(),
            if failed {
                &old.failure_model_digest
            } else {
                &old.model_digest
            }
        );
        let restored =
            ConnectedGasSession::read_snapshot(Arc::clone(&model), Cursor::new(bytes)).unwrap();
        let mut direct = ConnectedGasSession::new(model).unwrap();
        for _ in 0..step {
            direct.step_next().unwrap();
        }
        if failed {
            assert!(direct.step_next().is_err());
        }
        assert_eq!(restored.fields(), direct.fields());
        assert_eq!(
            restored.state_digest().unwrap(),
            direct.state_digest().unwrap()
        );
        let mut current = Vec::new();
        restored.write_snapshot(&mut current).unwrap();
        let header: Value = serde_json::from_str(
            std::str::from_utf8(&current)
                .unwrap()
                .lines()
                .next()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(header["schema_version"], "0.2.0");
    }
}

fn s(value: &str) -> Scalar {
    value.parse().unwrap()
}
fn identity(id: &str) -> PhysicsInstanceIdentity {
    PhysicsInstanceIdentity::new("factory", "air", id).unwrap()
}
fn inputs() -> (
    Vec<GasRegion>,
    Vec<FlowPatchConnection>,
    Vec<RegionInitialGas>,
    ConnectedGasProfile,
) {
    let names = [
        ("supply", "0.37", "2"),
        ("pipe", "0.71", "1"),
        ("receiver", "1.13", "1"),
    ];
    let regions = names
        .iter()
        .map(|(name, width, _)| GasRegion {
            identity: identity(name),
            shape: [2; 3],
            spacing_m: [s(width), Scalar::ONE, Scalar::ONE],
            boundaries: [FlowBoundary::AdiabaticSlipWall; 6],
        })
        .collect();
    let initial = names
        .iter()
        .map(|(name, _, pressure)| RegionInitialGas {
            identity: identity(name),
            density_kg_m3: s(pressure),
            pressure_pa: s(pressure),
            velocity_m_s: [Scalar::ZERO; 3],
        })
        .collect();
    let connections = [
        ("supply-pipe", "supply", "pipe"),
        ("pipe-receiver", "pipe", "receiver"),
    ]
    .map(|(id, a, b)| FlowPatchConnection {
        id: ComponentId::new(id).unwrap(),
        first: FlowPatch {
            region: identity(a),
            face: 1,
        },
        second: FlowPatch {
            region: identity(b),
            face: 0,
        },
    })
    .into();
    let profile = ConnectedGasProfile {
        reference: GasReference {
            length_m: Scalar::ONE,
            velocity_m_s: Scalar::ONE,
            density_kg_m3: Scalar::ONE,
            gas_constant_j_kg_k: Scalar::ONE,
        },
        heat_capacity_ratio: s("1.4"),
        dynamic_viscosity_pa_s: s("0.01"),
        thermal_conductivity_w_m_k: s("0.02"),
        timestep_us: 1000,
        steps: 20,
        limits: SolverLimits {
            iterations: 400,
            residual: s("0.00000005"),
        },
    };
    (regions, connections, initial, profile)
}
fn model(fail: bool) -> Arc<CompiledConnectedGas> {
    let (regions, connections, initial, mut profile) = inputs();
    if fail {
        profile.limits.iterations = 1;
        profile.limits.residual = Scalar::from_raw(1);
    }
    Arc::new(
        CompiledConnectedGas::compile(regions, connections, initial, profile, 16 * 1024 * 1024)
            .unwrap(),
    )
}
fn finish(session: &mut ConnectedGasSession) {
    while !session.complete() {
        session.step_next().unwrap();
    }
}
fn command(client: &PhysicsWorkerClient, revision: u64, command: ClockCommand) -> CommandReply {
    let ticket = client.try_submit(revision, command).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(reply) = client.try_take_reply(ticket).unwrap() {
            return reply;
        }
        assert!(Instant::now() < deadline);
        let status = client.projection().unwrap();
        client
            .wait_for_change(status.generation, Duration::from_millis(10))
            .unwrap();
    }
}
fn complete(client: &PhysicsWorkerClient) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let status = client.projection().unwrap();
        assert!(status.fault.is_none(), "{status:?}");
        if status.complete {
            return;
        }
        assert!(Instant::now() < deadline);
        client
            .wait_for_change(status.generation, Duration::from_millis(10))
            .unwrap();
    }
}

#[test]
fn source_order_and_si_settings_are_reflected_in_model_identity() {
    let base = model(false);
    let (mut regions, mut connections, mut initial, profile) = inputs();
    regions.reverse();
    connections.reverse();
    initial.reverse();
    let reordered =
        CompiledConnectedGas::compile(regions, connections, initial, profile, 16 * 1024 * 1024)
            .unwrap();
    assert_eq!(base.digest(), reordered.digest());
    let mut reordered_session = ConnectedGasSession::new(Arc::new(reordered)).unwrap();
    finish(&mut reordered_session);
    assert_eq!(
        reordered_session.state_digest().unwrap(),
        golden().final_digest.as_bytes()
    );
    for variant in 0..12 {
        let (regions, connections, mut initial, mut profile) = inputs();
        match variant {
            0 => profile.heat_capacity_ratio = s("1.3"),
            1 => profile.dynamic_viscosity_pa_s = s("0.03"),
            2 => profile.thermal_conductivity_w_m_k = s("0.03"),
            3 => profile.timestep_us = 2000,
            4 => profile.limits.iterations = 399,
            5 => initial[0].pressure_pa = s("2.1"),
            6 => profile.steps = 21,
            7 => profile.limits.residual = s("0.0000001"),
            8 => profile.reference.length_m = s("2"),
            9 => profile.reference.velocity_m_s = s("2"),
            10 => profile.reference.density_kg_m3 = s("2"),
            _ => profile.reference.gas_constant_j_kg_k = s("2"),
        }
        let changed =
            CompiledConnectedGas::compile(regions, connections, initial, profile, 16 * 1024 * 1024)
                .unwrap();
        assert_ne!(base.digest(), changed.digest());
    }
    let (mut regions, _, mut initial, mut profile) = inputs();
    regions.truncate(1);
    initial.truncate(1);
    regions[0].shape = [1; 3];
    regions[0].spacing_m = [s("0.0001"); 3];
    profile.reference.length_m = s("0.0001");
    let small =
        CompiledConnectedGas::compile(regions, vec![], initial, profile, 16 * 1024 * 1024).unwrap();
    assert_eq!(
        small.mesh().regions()[0].grid.volume().unwrap(),
        Scalar::ONE
    );
    let (mut regions, _, mut initial, mut profile) = inputs();
    regions.truncate(1);
    initial.truncate(1);
    profile.reference = GasReference {
        length_m: s("2"),
        velocity_m_s: s("10"),
        density_kg_m3: s("3"),
        gas_constant_j_kg_k: s("2"),
    };
    initial[0].density_kg_m3 = s("3");
    initial[0].pressure_pa = s("300");
    let scaled = Arc::new(
        CompiledConnectedGas::compile(regions, vec![], initial, profile, 16 * 1024 * 1024).unwrap(),
    );
    let session = ConnectedGasSession::new(scaled).unwrap();
    assert_eq!(session.ranges().first_field.minimum_raw, s("300").raw());
    assert_eq!(session.ranges().temperature_k.minimum_raw, s("50").raw());
}

#[test]
fn invalid_coefficients_initial_conditions_and_horizons_reject_compilation() {
    for case in 0..9 {
        let (mut regions, connections, mut initial, mut profile) = inputs();
        match case {
            0 => profile.dynamic_viscosity_pa_s = Scalar::from_raw(-1),
            1 => profile.thermal_conductivity_w_m_k = Scalar::from_raw(-1),
            2 => profile.timestep_us = u64::MAX,
            3 => profile.steps = 0,
            4 => profile.limits.iterations = 0,
            5 => {
                initial.pop();
            }
            6 => initial[0].identity = identity("unknown"),
            7 => initial[0].density_kg_m3 = Scalar::ZERO,
            _ => {
                regions[0].boundaries[0] = FlowBoundary::NoSlipWall {
                    velocity: [Scalar::from_raw(1), Scalar::ZERO, Scalar::ZERO],
                    temperature: None,
                }
            }
        }
        assert!(
            CompiledConnectedGas::compile(regions, connections, initial, profile, 16 * 1024 * 1024)
                .is_err()
        );
    }
    let (regions, connections, initial, profile) = inputs();
    assert!(CompiledConnectedGas::compile(regions, connections, initial, profile, 1).is_err());
}

#[test]
fn full_state_snapshots_continue_exactly_at_every_boundary_without_capture_allocations() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let model = model(false);
            let mut session = ConnectedGasSession::new(Arc::clone(&model)).unwrap();
            let golden = golden();
            assert_eq!(model.digest(), golden.model_digest);
            assert_eq!(
                session.state_digest().unwrap(),
                golden.initial_digest.as_bytes()
            );
            let mut expected = ConnectedGasSession::new(Arc::clone(&model)).unwrap();
            finish(&mut expected);
            let final_digest = expected.state_digest().unwrap();
            for step in 0..=model.profile().steps {
                assert_eq!(session.step(), step);
                if step == 8 {
                    assert_eq!(
                        session.state_digest().unwrap(),
                        golden.after_eight_steps_digest.as_bytes()
                    );
                }
                let allocation = allocation_counter::measure(|| {
                    session.write_snapshot(io::sink()).unwrap();
                    session.state_digest().unwrap();
                });
                assert_eq!(allocation.count_total, 0);
                let mut bytes = Vec::new();
                session.write_snapshot(&mut bytes).unwrap();
                let mut restored =
                    ConnectedGasSession::read_snapshot(Arc::clone(&model), Cursor::new(bytes))
                        .unwrap();
                assert_eq!(
                    session.state_digest().unwrap(),
                    restored.state_digest().unwrap()
                );
                let allocation = allocation_counter::measure(|| finish(&mut restored));
                assert_eq!(allocation.count_total, 0);
                assert_eq!(restored.state_digest().unwrap(), final_digest);
                if !session.complete() {
                    session.step_next().unwrap();
                }
            }
            assert_eq!(final_digest, golden.final_digest.as_bytes());
        })
        .unwrap()
        .join()
        .unwrap();
}

fn rehash(lines: &mut [Value]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for line in &lines[..lines.len() - 1] {
        serde_json::to_writer(&mut bytes, line).unwrap();
        bytes.push(b'\n');
    }
    let digest: String = Sha256::digest(&bytes)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect();
    *lines.last_mut().unwrap() = serde_json::json!({ "sha256": digest });
    serde_json::to_writer(&mut bytes, lines.last().unwrap()).unwrap();
    bytes.push(b'\n');
    bytes
}

#[test]
fn rehashed_archives_cannot_forge_clock_inventory_initial_state_or_failure() {
    let model = model(false);
    let mut session = ConnectedGasSession::new(Arc::clone(&model)).unwrap();
    session.step_next().unwrap();
    let mut bytes = Vec::new();
    session.write_snapshot(&mut bytes).unwrap();
    let original: Vec<Value> = std::str::from_utf8(&bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    for case in 0..11 {
        let mut lines = original.clone();
        match case {
            0 => lines[0]["time_us"] = 0.into(),
            1 => lines[0]["cells"] = 0.into(),
            2 => lines[1]["index"] = 1.into(),
            3 => lines[1]["inventory_raw"][0] = 0.into(),
            4 => lines[0]["exchange_raw"][0] = 1.into(),
            5 => lines[0]["failure"] = serde_json::json!({ "kind": "overflow" }),
            6 => lines[0]["last_report"]["iterations"] = 0.into(),
            7 => lines[0]["schema_version"] = "99.0.0".into(),
            8 => lines[0]["model_digest"] = "wrong".into(),
            9 => lines[0]["last_report"]["boundary_exchange_raw"][0] = 1.into(),
            _ => {
                lines[0]["step"] = 0.into();
                lines[0]["time_us"] = 0.into();
                lines[0]["last_report"] = Value::Null;
            }
        }
        assert!(
            ConnectedGasSession::read_snapshot(Arc::clone(&model), Cursor::new(rehash(&mut lines)))
                .is_err(),
            "case {case}"
        );
    }
    assert!(
        ConnectedGasSession::read_snapshot(
            Arc::clone(&model),
            Cursor::new(&bytes[..bytes.len() - 1])
        )
        .is_err()
    );
    bytes.extend_from_slice(b"extra\n");
    assert!(ConnectedGasSession::read_snapshot(Arc::clone(&model), Cursor::new(bytes)).is_err());
    assert!(ConnectedGasSession::read_snapshot(model, Cursor::new(vec![b' '; 4097])).is_err());
}

#[test]
fn failed_numerics_retain_fields_clock_and_failure_across_snapshot_and_worker_restart() {
    let model = model(true);
    let mut session = ConnectedGasSession::new(Arc::clone(&model)).unwrap();
    let fields = session.fields().to_vec();
    let ranges = session.ranges();
    let allocation = allocation_counter::measure(|| {
        assert!(matches!(
            session.step_next(),
            Err(ConnectedStepError::Failed(
                PhysicsError::NonConvergence { .. }
            ))
        ))
    });
    assert_eq!(allocation.count_total, 0);
    let golden = golden();
    assert_eq!(model.digest(), golden.failure_model_digest);
    assert_eq!(
        session.state_digest().unwrap(),
        golden.failure_digest.as_bytes()
    );
    assert_eq!(session.fields(), fields);
    assert_eq!(session.ranges(), ranges);
    assert_eq!(session.time_us(), 0);
    let mut bytes = Vec::new();
    session.write_snapshot(&mut bytes).unwrap();
    let restored = ConnectedGasSession::read_snapshot(model, Cursor::new(bytes)).unwrap();
    assert_eq!(restored.failure(), session.failure());
    assert_eq!(
        restored.state_digest().unwrap(),
        session.state_digest().unwrap()
    );
    let worker =
        PhysicsWorker::spawn_connected_session(restored, WorkerPolicy::reviewed_default()).unwrap();
    assert!(worker.client().projection().unwrap().fault.is_some());
    assert!(
        command(&worker.client(), 0, ClockCommand::Resume)
            .result
            .is_err()
    );
    assert_eq!(
        worker.shutdown().unwrap().state_digest().unwrap(),
        session.state_digest().unwrap()
    );
}

#[test]
fn connected_worker_rates_and_manual_restart_preserve_the_full_state_digest() {
    let model = model(false);
    let mut direct = ConnectedGasSession::new(Arc::clone(&model)).unwrap();
    finish(&mut direct);
    let expected = direct.state_digest().unwrap();
    for mode in [
        ClockCommand::Headless,
        ClockCommand::SetRate(PlaybackRate::One),
        ClockCommand::SetRate(PlaybackRate::Two),
        ClockCommand::SetRate(PlaybackRate::Five),
        ClockCommand::SetRate(PlaybackRate::Ten),
    ] {
        let worker =
            PhysicsWorker::spawn_connected(Arc::clone(&model), WorkerPolicy::reviewed_default())
                .unwrap();
        let client = worker.client();
        command(&client, 0, ClockCommand::SingleStep)
            .result
            .unwrap();
        assert_eq!(client.projection().unwrap().time_us, 1000);
        let session = worker.shutdown().unwrap();
        let worker =
            PhysicsWorker::spawn_connected_session(session, WorkerPolicy::reviewed_default())
                .unwrap();
        let client = worker.client();
        command(&client, 0, mode).result.unwrap();
        command(&client, 1, ClockCommand::Resume).result.unwrap();
        complete(&client);
        assert_eq!(client.projection().unwrap().automatic_completed_us, 19_000);
        assert_eq!(worker.shutdown().unwrap().state_digest().unwrap(), expected);
    }
}

#[test]
fn measured_compiler_and_runtime_allocations_fit_declared_memory_and_worker_adds_its_overhead() {
    let mut peak = 0;
    let allocation = allocation_counter::measure(|| {
        let model = model(false);
        let session = ConnectedGasSession::new(Arc::clone(&model)).unwrap();
        peak = session
            .memory()
            .runtime_bytes
            .max(model.memory().compiler_peak_bytes);
        let worker =
            WorkerMemoryPlan::compile_connected(&model, WorkerPolicy::reviewed_default()).unwrap();
        assert_eq!(worker.numerical_bytes, session.memory().runtime_bytes);
        assert!(worker.planned_bytes >= session.memory().runtime_bytes + 1024 * 1024);
        std::hint::black_box(session);
    });
    assert!(
        allocation.bytes_max <= peak,
        "{} > {peak}",
        allocation.bytes_max
    );
    let model = model(false);
    let mut session = ConnectedGasSession::new(Arc::clone(&model)).unwrap();
    finish(&mut session);
    let mut bytes = Vec::new();
    session.write_snapshot(&mut bytes).unwrap();
    let memory = *session.memory();
    drop(session);
    let allocation = allocation_counter::measure(|| {
        std::hint::black_box(
            ConnectedGasSession::read_snapshot(model, Cursor::new(&bytes)).unwrap(),
        );
    });
    assert!(allocation.bytes_max <= memory.restore_bytes);

    let (regions, connections, initial, profile) = inputs();
    let small = Arc::new(
        CompiledConnectedGas::compile(regions, connections, initial, profile, 128 * 1024).unwrap(),
    );
    assert!(ConnectedGasSession::new(Arc::clone(&small)).is_ok());
    assert!(PhysicsWorker::spawn_connected(small, WorkerPolicy::reviewed_default()).is_err());
}
