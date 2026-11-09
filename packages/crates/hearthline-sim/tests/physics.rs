use std::sync::Arc;

use hearthline_config::physics::{PhysicsProofCase, PhysicsProofConfig};
use hearthline_project::physics::{CompiledPhysicsProof, PhysicsMemoryPlan};
use hearthline_sim::physics::clock::{PhysicsPacer, PlaybackRate};
use hearthline_sim::physics::{PhysicsProofSession, PhysicsReplay};

const RECEIVER: &str =
    include_str!("../../../../project/physics/proofs/receiver-equilibration.yaml");
const DRYING: &str = include_str!("../../../../project/physics/proofs/drying-specimen.yaml");
const DEFORMATION: &str =
    include_str!("../../../../project/physics/proofs/drying-deformation.yaml");
const COMPRESSIBLE: &str =
    include_str!("../../../../project/physics/proofs/compressible-receiver.yaml");

fn config(source: &str) -> PhysicsProofConfig {
    serde_yaml_ng::from_str(source).unwrap()
}

#[test]
fn all_proofs_reexecute_and_resume_exactly_on_a_one_mib_stack() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            for source in [RECEIVER, DRYING, DEFORMATION, COMPRESSIBLE] {
                let artifact = PhysicsReplay::record(config(source)).unwrap();
                artifact.verify().unwrap();
                let encoded = serde_json::to_string(&artifact).unwrap();
                let decoded: PhysicsReplay = serde_json::from_str(&encoded).unwrap();
                assert_eq!(artifact, decoded);
                let compiled = Arc::new(CompiledPhysicsProof::compile(config(source)).unwrap());
                let mut resumed = PhysicsProofSession::new(compiled).unwrap();
                let checkpoint = &artifact.checkpoints[0];
                resumed.restore(checkpoint).unwrap();
                for _ in checkpoint.step..artifact.config.steps {
                    resumed.step().unwrap();
                }
                assert_eq!(resumed.checkpoint(), artifact.final_state);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn compressible_receiver_evolves_momentum_and_heat_while_conserving_closed_inventory() {
    use hearthline_physics::{Scalar, flow::Conserved};
    let input = config(COMPRESSIBLE);
    let model = CompiledPhysicsProof::compile(input.clone()).unwrap();
    let artifact = PhysicsReplay::record(input).unwrap();
    let initial = artifact.initial.flow.as_ref().unwrap();
    let final_state = artifact.final_state.flow.as_ref().unwrap();
    assert_eq!(model.memory().field_buffer_bytes, 24 * 232);
    let inventory = |raw: &[[i64; 5]]| {
        model
            .flow()
            .unwrap()
            .solver()
            .inventory(
                &raw.iter()
                    .map(|c| Conserved::from_components(c.map(Scalar::from_raw)))
                    .collect::<Vec<_>>(),
            )
            .unwrap()
    };
    let before = inventory(&initial.conserved_raw);
    let after = inventory(&final_state.conserved_raw);
    assert!(
        after
            .density
            .checked_sub(before.density)
            .unwrap()
            .abs()
            .unwrap()
            < "0.000001".parse().unwrap()
    );
    assert!(
        after
            .total_energy
            .checked_sub(before.total_energy)
            .unwrap()
            .abs()
            .unwrap()
            < "0.000001".parse().unwrap()
    );
    assert!(
        initial
            .conserved_raw
            .iter()
            .all(|cell| cell[1..4] == [0; 3])
    );
    let charging = artifact.checkpoints[0].flow.as_ref().unwrap();
    assert!(charging.conserved_raw.iter().any(|cell| cell[1] > 0));
    // Closed volumes can reverse flow following pressure-wave reflection.
    assert!(final_state.conserved_raw.iter().any(|cell| cell[1] != 0));
    assert!(artifact.final_state.first_field_raw[0] < artifact.initial.first_field_raw[0]);
    assert!(artifact.final_state.first_field_raw[5] > artifact.initial.first_field_raw[5]);
    assert_ne!(
        artifact.final_state.temperature_k_raw,
        artifact.initial.temperature_k_raw
    );
    let report = final_state.last_step.unwrap();
    assert_eq!(
        report.solver_time_us,
        model.flow().unwrap().solver_timestep_us()
    );
    assert_eq!(report.boundary_exchange_raw[0], 0);
    assert_eq!(report.boundary_exchange_raw[4], 0);
    assert!(report.successful_sweeps > 0);
    assert_eq!(artifact.final_state.time_us, 2000);
}

#[test]
fn rejected_receiver_retry_rolls_back_clock_inventory_readings_and_solver_metadata() {
    let mut input = config(COMPRESSIBLE);
    input.maximum_iterations = 1;
    input.residual_tolerance = "0.000000001".into();
    if let PhysicsProofCase::CompressibleReceiver { flow } = &mut input.case {
        flow.maximum_substeps = 1;
    }
    let mut session =
        PhysicsProofSession::new(Arc::new(CompiledPhysicsProof::compile(input).unwrap())).unwrap();
    let before = session.checkpoint();
    let mut failure = None;
    let allocations = allocation_counter::measure(|| {
        failure = Some(session.step().unwrap_err());
    });
    assert_eq!(allocations.count_total, 0);
    let error = failure.unwrap().to_string();
    assert!(error.contains("rejected after 1 attempts"), "{error}");
    assert_eq!(session.checkpoint(), before);
    let mut artifact = PhysicsReplay::record(config(COMPRESSIBLE)).unwrap();
    artifact.checkpoints[0].flow.as_mut().unwrap().conserved_raw[3][1] += 1;
    let error = artifact.verify().unwrap_err();
    assert!(error.contains("cell 3 flow component 1"), "{error}");
}

#[test]
fn receiver_reference_scales_and_retry_limits_are_validated_before_allocation() {
    for change in 0..9 {
        let mut input = config(COMPRESSIBLE);
        if let PhysicsProofCase::CompressibleReceiver { flow } = &mut input.case {
            match change {
                0 => flow.length_scale_m = "0".into(),
                1 => flow.heat_capacity_ratio = "1".into(),
                2 => flow.receiver_start_x = 0,
                3 => flow.receiver_start_x = 6,
                4 => flow.maximum_substeps = 3,
                5 => flow.minimum_solver_step_us = u64::MAX,
                6 => flow.dynamic_viscosity_pa_s = "0.000000001".into(),
                7 => flow.wall_temperature_k = Some("0".into()),
                _ => flow.initial_temperature_k = "-1".into(),
            }
        }
        assert!(
            CompiledPhysicsProof::compile(input).is_err(),
            "accepted alteration {change}"
        );
    }
}

#[test]
fn receiver_pacing_and_headless_execution_produce_identical_conserved_fields() {
    let expected = PhysicsReplay::record(config(COMPRESSIBLE)).unwrap();
    for rate in [
        PlaybackRate::One,
        PlaybackRate::Two,
        PlaybackRate::Five,
        PlaybackRate::Ten,
    ] {
        let model = Arc::new(CompiledPhysicsProof::compile(config(COMPRESSIBLE)).unwrap());
        let mut session = PhysicsProofSession::new(model.clone()).unwrap();
        let mut pacer = PhysicsPacer::new(model.timestep_us()).unwrap();
        pacer.set_rate(rate);
        for _ in 0..model.config().steps {
            while !pacer.step_due() {
                pacer.accrue(73).unwrap();
            }
            session.step().unwrap();
            pacer.commit_step().unwrap();
        }
        assert_eq!(session.checkpoint(), expected.final_state);
    }
}

#[test]
fn post_construction_physics_steps_allocate_nothing_and_memory_is_measured() {
    for source in [RECEIVER, DRYING, DEFORMATION, COMPRESSIBLE] {
        let compiled = Arc::new(CompiledPhysicsProof::compile(config(source)).unwrap());
        let mut session = None;
        let measured = allocation_counter::measure(|| {
            session = Some(PhysicsProofSession::new(compiled.clone()).unwrap());
        });
        assert_eq!(measured.bytes_total, compiled.memory().field_buffer_bytes);
        let mut session = session.unwrap();
        let kernel = match &compiled.config().case {
            hearthline_config::physics::PhysicsProofCase::CompressibleReceiver { .. } => {
                hearthline_config::physics::capacity::CapacityKernel::CompressibleFlow
            }
            hearthline_config::physics::PhysicsProofCase::DryingSpecimen {
                mechanics: Some(_),
                ..
            } => hearthline_config::physics::capacity::CapacityKernel::DryingElasticity,
            _ => hearthline_config::physics::capacity::CapacityKernel::ScalarTransport,
        };
        let layout = hearthline_project::physics::PhysicsStorageLayout::compile(
            compiled.config().shape,
            kernel,
        )
        .unwrap();
        let snapshot = session.checkpoint();
        let mechanical_fields = snapshot
            .mechanics
            .as_ref()
            .map_or(0, |m| m.displacement_raw.len() + m.stress_raw.len());
        let gas_fields = snapshot
            .flow
            .as_ref()
            .map_or(0, |f| f.conserved_raw.len() * 5);
        let field_values = snapshot.first_field_raw.len()
            + snapshot.temperature_k_raw.len()
            + mechanical_fields
            + gas_fields;
        assert_eq!(layout.checkpoint_field_bytes, field_values as u64 * 8);
        assert!(layout.state_bytes <= layout.checkpoint_field_bytes);
        assert_eq!(
            layout.instance_buffer_bytes().unwrap(),
            compiled.memory().field_buffer_bytes
        );
        let measured = allocation_counter::measure(|| {
            for _ in 0..compiled.config().steps {
                session.step().unwrap();
            }
        });
        assert_eq!(measured.count_total, 0);
    }
}

#[test]
fn factory_resource_planning_does_not_allocate_projected_spatial_fields() {
    use hearthline_config::physics::capacity::PhysicsCapacityConfig;
    use hearthline_project::physics::capacity::PhysicsCapacityReport;

    let mut input: PhysicsCapacityConfig = serde_yaml_ng::from_str(include_str!(
        "../../../../project/physics/capacity/factory-baseline.yaml"
    ))
    .unwrap();
    for profile in &mut input.profiles {
        profile.shape = [1000; 3];
    }
    let mut result = None;
    let measured = allocation_counter::measure(|| {
        result = Some(PhysicsCapacityReport::compile(input).unwrap());
    });
    let report = result.unwrap();
    assert!(!report.within_budget);
    assert!(report.total_planned_bytes > 1_000_000_000_000);
    assert!(measured.bytes_total < 1024 * 1024);
}

#[test]
fn drying_fields_drive_deformation_and_support_stress_without_a_quality_score() {
    let artifact = PhysicsReplay::record(config(DEFORMATION)).unwrap();
    let initial = artifact.initial.mechanics.as_ref().unwrap();
    assert!(initial.displacement_raw.iter().all(|v| *v == 0));
    assert!(initial.stress_raw.iter().all(|v| *v == 0));
    let final_state = artifact.final_state.mechanics.as_ref().unwrap();
    assert_eq!(final_state.displacement_raw.len(), 64 * 3);
    assert_eq!(final_state.stress_raw.len(), 27 * 6 * 6);
    assert!(final_state.displacement_raw[3 * 63] < 0);
    assert!(final_state.stress_raw.iter().any(|v| *v > 1000));
    for node in (0..64).step_by(4) {
        assert_eq!(
            &final_state.displacement_raw[3 * node..3 * node + 3],
            &[0; 3]
        );
    }
    let mut uncoupled = config(DEFORMATION);
    if let PhysicsProofCase::DryingSpecimen { mechanics, .. } = &mut uncoupled.case {
        let mechanics = mechanics.as_mut().unwrap();
        mechanics.moisture_expansion_per_kg_kg = "0".into();
        mechanics.thermal_expansion_per_k = "0".into();
    }
    let independent = PhysicsReplay::record(uncoupled).unwrap();
    assert_eq!(
        independent.final_state.first_field_raw,
        artifact.final_state.first_field_raw
    );
    assert_eq!(
        independent.final_state.temperature_k_raw,
        artifact.final_state.temperature_k_raw
    );
    assert!(
        independent
            .final_state
            .mechanics
            .unwrap()
            .displacement_raw
            .iter()
            .all(|v| *v == 0)
    );
}

#[test]
fn failed_mechanics_rolls_back_the_entire_drying_step_and_replay_reports_deformation() {
    let mut input = config(DEFORMATION);
    if let PhysicsProofCase::DryingSpecimen { mechanics, .. } = &mut input.case {
        let mechanics = mechanics.as_mut().unwrap();
        mechanics.maximum_iterations = 1;
        mechanics.residual_tolerance = "0.000000001".into();
    }
    let mut session =
        PhysicsProofSession::new(Arc::new(CompiledPhysicsProof::compile(input).unwrap())).unwrap();
    let before = session.checkpoint();
    assert!(session.step().is_err());
    assert_eq!(session.checkpoint(), before);
    let mut artifact = PhysicsReplay::record(config(DEFORMATION)).unwrap();
    artifact.checkpoints[0]
        .mechanics
        .as_mut()
        .unwrap()
        .displacement_raw[5] += 1;
    let error = artifact.verify().unwrap_err();
    assert!(
        error.contains("displacement entity 1 component 2"),
        "{error}"
    );
}

#[test]
fn mechanical_geometry_is_accounted_before_compilation_and_inverted_materials_are_rejected() {
    let mut input = config(DEFORMATION);
    let compiled = CompiledPhysicsProof::compile(input.clone()).unwrap();
    assert!(compiled.memory().immutable_mesh_bytes > 0);
    assert!(compiled.memory().field_buffer_bytes > 27 * 96);
    let mechanical = compiled.mechanics().unwrap();
    assert_eq!(mechanical.positions().len(), 64);
    assert_eq!(mechanical.elements().len(), 162);
    let volume: i64 = mechanical.elements().iter().map(|e| e.volume().raw()).sum();
    assert!((volume - 1_000_000_000).abs() < 1000);
    input.allocation_budget_bytes = compiled.memory().field_buffer_bytes;
    assert!(CompiledPhysicsProof::compile(input).is_err());
    for change in 0..4 {
        let mut input = config(DEFORMATION);
        if let PhysicsProofCase::DryingSpecimen { mechanics, .. } = &mut input.case {
            let mechanics = mechanics.as_mut().unwrap();
            match change {
                0 => mechanics.poisson_ratio = "0.5".into(),
                1 => mechanics.length_scale_m = "0".into(),
                2 => mechanics.maximum_iterations = 0,
                _ => mechanics.young_modulus_pa = "-1".into(),
            }
        }
        assert!(CompiledPhysicsProof::compile(input).is_err());
    }
}

#[test]
fn receiver_equalization_conserves_mass_and_does_not_create_pressure() {
    let artifact = PhysicsReplay::record(config(RECEIVER)).unwrap();
    assert!(artifact.final_state.first_field_raw[0] < artifact.initial.first_field_raw[0]);
    assert!(artifact.final_state.first_field_raw.last() > artifact.initial.first_field_raw.last());
    let minimum = *artifact.initial.first_field_raw.iter().min().unwrap();
    let maximum = *artifact.initial.first_field_raw.iter().max().unwrap();
    assert!(
        artifact
            .final_state
            .first_field_raw
            .iter()
            .all(|p| *p >= minimum && *p <= maximum)
    );
    assert!(artifact.final_state.balance_error_raw[0].abs() < 10_000);
}

#[test]
fn surface_drying_is_spatial_and_latent_heat_cools_the_specimen() {
    let artifact = PhysicsReplay::record(config(DRYING)).unwrap();
    let initial = &artifact.initial;
    let final_state = &artifact.final_state;
    assert!(
        final_state.first_field_raw.iter().sum::<i64>()
            < initial.first_field_raw.iter().sum::<i64>()
    );
    assert!(final_state.first_field_raw[0] < final_state.first_field_raw[13]);
    assert!(final_state.temperature_k_raw[0] < initial.temperature_k_raw[0]);
    assert!(final_state.balance_error_raw[0].abs() < 100_000);
    assert!(final_state.balance_error_raw[1].abs() < 1_000_000);
}

#[test]
fn condensation_adds_water_and_releases_latent_heat() {
    let mut input = config(DRYING);
    if let PhysicsProofCase::DryingSpecimen {
        equilibrium_moisture_kg_kg,
        ..
    } = &mut input.case
    {
        *equilibrium_moisture_kg_kg = "0.4".into();
    }
    let artifact = PhysicsReplay::record(input).unwrap();
    assert!(artifact.final_state.first_field_raw[0] > artifact.initial.first_field_raw[0]);
    assert!(artifact.final_state.temperature_k_raw[0] > artifact.initial.temperature_k_raw[0]);
}

#[test]
fn rejected_coupled_step_does_not_advance_fields_clock_or_ledgers() {
    let mut input = config(DRYING);
    input.maximum_iterations = 1;
    input.residual_tolerance = "0.000000001".into();
    let mut session =
        PhysicsProofSession::new(Arc::new(CompiledPhysicsProof::compile(input).unwrap())).unwrap();
    let before = session.checkpoint();
    assert!(session.step().is_err());
    assert_eq!(session.checkpoint(), before);
}

#[test]
fn replay_rejects_tampering_and_locates_first_divergent_spatial_field() {
    let baseline = PhysicsReplay::record(config(DRYING)).unwrap();
    let mut artifact = baseline.clone();
    artifact.checkpoints[0].temperature_k_raw[5] += 1;
    let error = artifact.verify().unwrap_err();
    assert!(error.contains("cell 5 field temperature_k"), "{error}");
    for change in 0..5 {
        let mut artifact = baseline.clone();
        match change {
            0 => artifact.numerical_contract = "unknown".into(),
            1 => artifact.model_digest.clear(),
            2 => artifact.initial.time_us = 1,
            3 => artifact.final_state.step -= 1,
            _ => artifact.checkpoints.push(artifact.checkpoints[0].clone()),
        }
        assert!(artifact.verify().is_err());
    }
    let mut session = PhysicsProofSession::new(Arc::new(
        CompiledPhysicsProof::compile(config(DRYING)).unwrap(),
    ))
    .unwrap();
    let before = session.checkpoint();
    let mut bad = baseline.checkpoints[0].clone();
    bad.first_field_raw.pop();
    assert!(session.restore(&bad).is_err());
    assert_eq!(before, session.checkpoint());
}

#[test]
fn source_newlines_and_mapping_order_do_not_change_model_digest() {
    let original = CompiledPhysicsProof::compile(config(RECEIVER)).unwrap();
    let crlf = CompiledPhysicsProof::compile(config(&RECEIVER.replace('\n', "\r\n"))).unwrap();
    assert_eq!(original.digest(), crlf.digest());
    let mut source: serde_yaml_ng::Value = serde_yaml_ng::from_str(RECEIVER).unwrap();
    let value = source.as_mapping_mut().unwrap().remove("id").unwrap();
    source.as_mapping_mut().unwrap().insert("id".into(), value);
    let reordered =
        CompiledPhysicsProof::compile(serde_yaml_ng::from_value(source).unwrap()).unwrap();
    assert_eq!(original.digest(), reordered.digest());
}

#[test]
fn capacity_and_config_rejections_happen_before_field_allocation() {
    assert!(PhysicsMemoryPlan::compile(0, 100).is_err());
    assert!(PhysicsMemoryPlan::compile(u64::MAX, 100).is_err());
    assert!(PhysicsMemoryPlan::compile(1, u64::MAX).is_err());
    assert!(PhysicsMemoryPlan::compile(1, 127).is_err());
    let accepted = PhysicsMemoryPlan::compile(1, 128).unwrap();
    assert_eq!(accepted.field_buffer_bytes, 96);
    assert_eq!(accepted.reserve_bytes, 32);
    for change in 0..7 {
        let mut input = config(RECEIVER);
        match change {
            0 => input.shape = [u32::MAX; 3],
            1 => input.timestep_s = "0.000000001".into(),
            2 => input.allocation_budget_bytes = 100,
            3 => input.steps = 0,
            4 => input.schema_version = "99".into(),
            5 => input.residual_tolerance = "-1".into(),
            _ => input.allocation_budget_bytes = u64::MAX,
        }
        assert!(CompiledPhysicsProof::compile(input).is_err());
    }
    assert!(
        serde_yaml_ng::from_str::<PhysicsProofConfig>(&format!("{RECEIVER}\nunknown: true\n"))
            .is_err()
    );
}

#[test]
fn pacing_changes_wall_time_not_solver_results_and_never_drops_backlog() {
    let baseline = PhysicsReplay::record(config(DRYING)).unwrap();
    for rate in [
        PlaybackRate::One,
        PlaybackRate::Two,
        PlaybackRate::Five,
        PlaybackRate::Ten,
    ] {
        let model = Arc::new(CompiledPhysicsProof::compile(config(DRYING)).unwrap());
        let mut session = PhysicsProofSession::new(model.clone()).unwrap();
        let mut pacer = PhysicsPacer::new(model.timestep_us()).unwrap();
        pacer.set_rate(rate);
        pacer.accrue(10_000_000).unwrap();
        // A busy solver processes one step, leaving the rest queued.
        session.step().unwrap();
        pacer.commit_step().unwrap();
        assert!(pacer.pending_credit_us() >= 9_000_000);
        pacer.set_paused(true);
        let credit = pacer.pending_credit_us();
        pacer.accrue(100_000_000).unwrap();
        assert_eq!(pacer.pending_credit_us(), credit);
        assert!(!pacer.step_due());
        pacer.request_single_step().unwrap();
        session.step().unwrap();
        pacer.commit_step().unwrap();
        pacer.set_paused(false);
        for _ in 2..model.config().steps {
            if !pacer.step_due() {
                pacer.accrue(1_000_000).unwrap();
            }
            session.step().unwrap();
            pacer.commit_step().unwrap();
        }
        assert_eq!(session.checkpoint(), baseline.final_state);
        assert_eq!(pacer.completed_us(), baseline.final_state.time_us);
    }
    assert!(PhysicsPacer::new(0).is_err());
    let mut pacer = PhysicsPacer::new(1).unwrap();
    assert!(pacer.commit_step().is_err());
    assert!(pacer.request_single_step().is_err());
    pacer.set_rate(PlaybackRate::Ten);
    assert!(pacer.accrue(u64::MAX).is_err());
    assert_eq!(pacer.pending_credit_us(), 0);
}
