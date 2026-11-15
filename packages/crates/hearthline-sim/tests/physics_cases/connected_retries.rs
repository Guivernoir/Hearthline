use super::*;
use hearthline_project::physics::flow::connected::GasRetryConfig;
use hearthline_sim::physics::worker::WorkerFault;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RetryGolden {
    schema_version: String,
    model_digest: String,
    states: [String; 4],
    substeps: u32,
    attempts: u32,
    failure_model_digest: String,
    failure_digest: String,
}
fn retry_golden() -> RetryGolden {
    let golden: RetryGolden = serde_json::from_str(include_str!(
        "../../../../../project/physics/runtime/connected-retry.json"
    ))
    .unwrap();
    assert_eq!(golden.schema_version, "0.2.0");
    golden
}

fn retry_model(fail: bool) -> Arc<CompiledConnectedGas> {
    let (regions, connections, initial, mut profile) = inputs();
    profile.timestep_us = 1001;
    profile.steps = 3;
    profile.limits.iterations = if fail { 1 } else { 14 };
    profile.limits.residual = s("0.0000001");
    Arc::new(
        CompiledConnectedGas::compile_with_retries(
            regions,
            connections,
            initial,
            profile,
            16 * 1024 * 1024,
            GasRetryConfig {
                maximum_substeps: 64,
                minimum_timestep_us: 1,
            },
        )
        .unwrap(),
    )
}

#[test]
fn retried_session_preserves_physical_time_and_continues_full_snapshots_without_allocation() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let model = retry_model(false);
            let golden = retry_golden();
            assert_eq!(model.digest(), golden.model_digest);
            assert_eq!(model.evaluation_bound(), 127 * 2 * 14);
            let mut session = ConnectedGasSession::new(Arc::clone(&model)).unwrap();
            assert_eq!(session.state_digest().unwrap(), golden.states[0].as_bytes());
            for index in 1..=3 {
                let allocation = allocation_counter::measure(|| {
                    let report = session.step_next().unwrap();
                    assert_eq!(report.substeps, golden.substeps);
                    assert_eq!(report.attempts, golden.attempts);
                    assert_eq!(report.completed, model.solver_dt());
                    assert_eq!(session.last_advance(), Some(report));
                    session.write_snapshot(io::sink()).unwrap();
                });
                assert_eq!(allocation.count_total, 0);
                assert_eq!(session.time_us(), index * 1001);
                assert_eq!(
                    session.state_digest().unwrap(),
                    golden.states[index as usize].as_bytes()
                );
                let mut bytes = Vec::new();
                session.write_snapshot(&mut bytes).unwrap();
                let restored =
                    ConnectedGasSession::read_snapshot(Arc::clone(&model), Cursor::new(bytes))
                        .unwrap();
                assert_eq!(
                    restored.state_digest().unwrap(),
                    session.state_digest().unwrap()
                );
                assert_eq!(restored.last_advance(), session.last_advance());
                session = restored;
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn exhausted_retry_metadata_survives_restore_and_worker_failure() {
    let model = retry_model(true);
    let mut session = ConnectedGasSession::new(Arc::clone(&model)).unwrap();
    let initial = session.fields().to_vec();
    let allocation = allocation_counter::measure(|| {
        assert!(session.step_next().is_err());
    });
    assert_eq!(allocation.count_total, 0);
    assert_eq!(session.fields(), initial);
    assert_eq!(session.time_us(), 0);
    let failure = session.retry_failure().unwrap();
    assert_eq!(failure.attempts, 7);
    assert_eq!(failure.substeps, 64);
    let golden = retry_golden();
    assert_eq!(model.digest(), golden.failure_model_digest);
    assert_eq!(
        session.state_digest().unwrap(),
        golden.failure_digest.as_bytes()
    );
    let mut bytes = Vec::new();
    session.write_snapshot(&mut bytes).unwrap();
    let restored = ConnectedGasSession::read_snapshot(model, Cursor::new(bytes)).unwrap();
    assert_eq!(restored.retry_failure(), Some(failure));
    assert_eq!(
        restored.state_digest().unwrap(),
        session.state_digest().unwrap()
    );
    let worker =
        PhysicsWorker::spawn_connected_session(restored, WorkerPolicy::reviewed_default()).unwrap();
    assert_eq!(
        worker.client().projection().unwrap().fault,
        Some(WorkerFault::Connected(failure))
    );
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
fn snapshot_of_a_late_failed_attempt_retains_only_the_accepted_origin() {
    let (_, _, _, mut profile) = inputs();
    profile.timestep_us = 1_000_000;
    profile.steps = 1;
    profile.limits = SolverLimits {
        iterations: 24,
        residual: s("0.0000001"),
    };
    profile.dynamic_viscosity_pa_s = Scalar::ZERO;
    profile.thermal_conductivity_w_m_k = Scalar::ZERO;
    let regions = [("left", "0.37"), ("right", "0.71")]
        .map(|(name, width)| {
            let mut boundaries = [FlowBoundary::AdiabaticSlipWall; 6];
            if name == "left" {
                boundaries[0] = FlowBoundary::Reservoir(hearthline_physics::flow::Primitive {
                    density: s("2"),
                    pressure: s("2"),
                    velocity: [Scalar::ONE, Scalar::ZERO, Scalar::ZERO],
                });
            }
            GasRegion {
                identity: identity(name),
                shape: [1; 3],
                spacing_m: [s(width), Scalar::ONE, Scalar::ONE],
                boundaries,
            }
        })
        .to_vec();
    let initial = ["left", "right"]
        .map(|name| RegionInitialGas {
            identity: identity(name),
            density_kg_m3: Scalar::ONE,
            pressure_pa: Scalar::ONE,
            velocity_m_s: [Scalar::ZERO; 3],
        })
        .to_vec();
    let connections = vec![FlowPatchConnection {
        id: ComponentId::new("feed").unwrap(),
        first: FlowPatch {
            region: identity("left"),
            face: 1,
        },
        second: FlowPatch {
            region: identity("right"),
            face: 0,
        },
    }];
    let model = Arc::new(
        CompiledConnectedGas::compile_with_retries(
            regions,
            connections,
            initial,
            profile,
            16 * 1024 * 1024,
            GasRetryConfig {
                maximum_substeps: 4,
                minimum_timestep_us: 1,
            },
        )
        .unwrap(),
    );
    let mut session = ConnectedGasSession::new(Arc::clone(&model)).unwrap();
    let initial = session.fields().to_vec();
    assert!(session.step_next().is_err());
    let failure = session.retry_failure().unwrap();
    assert_eq!(
        (
            failure.attempts,
            failure.substeps,
            failure.completed_substeps
        ),
        (3, 4, 2)
    );
    assert_eq!(
        failure.cause,
        PhysicsError::NonConvergence {
            iterations: 24,
            residual: Scalar::from_raw(103)
        }
    );
    assert_eq!(session.fields(), initial);
    assert_eq!(session.time_us(), 0);
    assert_eq!(
        session.exchange(),
        hearthline_physics::flow::connected::CellInventory::default()
    );
    let mut bytes = Vec::new();
    session.write_snapshot(&mut bytes).unwrap();
    let restored = ConnectedGasSession::read_snapshot(model, Cursor::new(bytes)).unwrap();
    assert_eq!(restored.retry_failure(), Some(failure));
    assert_eq!(
        restored.state_digest().unwrap(),
        session.state_digest().unwrap()
    );
}

#[test]
fn retry_policies_are_validated_and_change_model_identity() {
    for (maximum_substeps, minimum_timestep_us) in [(0, 1), (3, 1), (2, 0), (2, 1001)] {
        let (regions, connections, initial, profile) = inputs();
        assert!(
            CompiledConnectedGas::compile_with_retries(
                regions,
                connections,
                initial,
                profile,
                16 * 1024 * 1024,
                GasRetryConfig {
                    maximum_substeps,
                    minimum_timestep_us
                }
            )
            .is_err()
        );
    }
    let (regions, connections, initial, mut profile) = inputs();
    profile.reference.length_m = s("3");
    profile.timestep_us = 3000;
    let error = CompiledConnectedGas::compile_with_retries(
        regions,
        connections,
        initial,
        profile,
        16 * 1024 * 1024,
        GasRetryConfig {
            maximum_substeps: 2,
            minimum_timestep_us: 1,
        },
    )
    .unwrap_err();
    assert!(error.contains("retry minimum"), "{error}");
    let original = model(false);
    for policy in [
        GasRetryConfig {
            maximum_substeps: 1,
            minimum_timestep_us: 1,
        },
        GasRetryConfig {
            maximum_substeps: 2,
            minimum_timestep_us: 1,
        },
        GasRetryConfig {
            maximum_substeps: 2,
            minimum_timestep_us: 2,
        },
    ] {
        let (regions, connections, initial, profile) = inputs();
        let compiled = CompiledConnectedGas::compile_with_retries(
            regions,
            connections,
            initial,
            profile,
            16 * 1024 * 1024,
            policy,
        )
        .unwrap();
        assert_ne!(compiled.digest(), original.digest());
    }
}

#[test]
fn every_pacing_mode_resumes_a_retried_horizon_to_the_same_full_state() {
    let model = retry_model(false);
    let golden = retry_golden();
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
        assert_eq!(client.projection().unwrap().time_us, 1001);
        let work = client
            .projection()
            .unwrap()
            .fields
            .unwrap()
            .connected_work
            .unwrap();
        assert_eq!(work.attempts, golden.attempts);
        assert_eq!(work.substeps, golden.substeps);
        assert_eq!(work.accepted_iterations, 112);
        assert_eq!(work.accepted_evaluations, 120);
        assert_eq!(work.evaluation_bound, model.evaluation_bound());
        let session = worker.shutdown().unwrap();
        assert_eq!(session.state_digest().unwrap(), golden.states[1].as_bytes());
        let mut snapshot = Vec::new();
        session.write_snapshot(&mut snapshot).unwrap();
        drop(session);
        let restored =
            ConnectedGasSession::read_snapshot(Arc::clone(&model), Cursor::new(snapshot)).unwrap();
        let worker =
            PhysicsWorker::spawn_connected_session(restored, WorkerPolicy::reviewed_default())
                .unwrap();
        let client = worker.client();
        assert_eq!(
            client.projection().unwrap().fields.unwrap().connected_work,
            Some(work)
        );
        command(&client, 0, mode).result.unwrap();
        command(&client, 1, ClockCommand::Resume).result.unwrap();
        complete(&client);
        assert_eq!(client.projection().unwrap().automatic_completed_us, 2002);
        assert_eq!(
            worker.shutdown().unwrap().state_digest().unwrap(),
            golden.states[3].as_bytes()
        );
    }
}

#[test]
fn rehashed_retry_metadata_cannot_forge_time_work_exchange_or_failure() {
    let model = retry_model(false);
    let mut session = ConnectedGasSession::new(Arc::clone(&model)).unwrap();
    session.step_next().unwrap();
    let mut bytes = Vec::new();
    session.write_snapshot(&mut bytes).unwrap();
    let original: Vec<Value> = std::str::from_utf8(&bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    for variant in 0..14 {
        let mut lines = original.clone();
        match variant {
            0 => lines[0]["advance"]["substeps"] = 0.into(),
            1 => lines[0]["advance"]["substeps"] = 3.into(),
            2 => lines[0]["advance"]["substeps"] = 128.into(),
            3 => lines[0]["advance"]["attempts"] = 0.into(),
            4 => lines[0]["advance"]["completed_raw"] = 1.into(),
            5 => lines[0]["advance"]["accepted_iterations"] = u64::MAX.into(),
            6 => lines[0]["advance"]["accepted_evaluations"] = u64::MAX.into(),
            7 => lines[0]["advance"]["maximum_residual_raw"] = i64::MAX.into(),
            8 => lines[0]["advance"]["boundary_exchange_raw"][0] = 1.into(),
            9 => lines[0]["advance"] = Value::Null,
            10 => lines[0]["advance_contract"] = "wrong".into(),
            11 => lines[0]["schema_version"] = "0.1.0".into(),
            12 => lines[0]["advance"]["accepted_evaluations"] = 220.into(),
            _ => lines[0]["last_report"]["evaluations"] = 20.into(),
        }
        assert!(
            ConnectedGasSession::read_snapshot(Arc::clone(&model), Cursor::new(rehash(&mut lines)))
                .is_err(),
            "variant {variant}"
        );
    }
    let model = retry_model(true);
    let mut session = ConnectedGasSession::new(Arc::clone(&model)).unwrap();
    assert!(session.step_next().is_err());
    let mut bytes = Vec::new();
    session.write_snapshot(&mut bytes).unwrap();
    let original: Vec<Value> = std::str::from_utf8(&bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    for variant in 0..6 {
        let mut lines = original.clone();
        match variant {
            0 => lines[0]["retry_failure"]["attempts"] = 6.into(),
            1 => lines[0]["retry_failure"]["substeps"] = 32.into(),
            2 => lines[0]["retry_failure"]["completed_substeps"] = 1.into(),
            3 => lines[0]["retry_failure"]["smallest_step_raw"] = 1.into(),
            4 => lines[0]["retry_failure"] = Value::Null,
            _ => lines[0]["failure"] = Value::Null,
        }
        assert!(
            ConnectedGasSession::read_snapshot(Arc::clone(&model), Cursor::new(rehash(&mut lines)))
                .is_err(),
            "failure variant {variant}"
        );
    }
}
