use std::{io::Cursor, sync::Arc};

use hearthline_config::physics::{MechanicsInitialGuess, PhysicsProofCase, PhysicsProofConfig};
use hearthline_project::physics::CompiledPhysicsProof;
use hearthline_sim::physics::clock::{PhysicsPacer, PlaybackRate};
use hearthline_sim::physics::population::{PhysicsProofPopulation, PopulationMemoryPlan};
use hearthline_sim::physics::{PhysicsProofSession, PhysicsReplay};

fn config() -> PhysicsProofConfig {
    let mut config: PhysicsProofConfig = serde_yaml_ng::from_str(include_str!(
        "../../../../../project/physics/proofs/drying-deformation.yaml"
    ))
    .unwrap();
    let PhysicsProofCase::DryingSpecimen { mechanics, .. } = &mut config.case else {
        unreachable!()
    };
    mechanics.as_mut().unwrap().initial_guess = MechanicsInitialGuess::Zero;
    config
}

#[test]
fn legacy_refined_scaling_failure_is_not_hidden_by_a_larger_limit() {
    let mut config: PhysicsProofConfig = serde_yaml_ng::from_str(include_str!(
        "../../../../../project/physics/proofs/basin-wet-reconstructed.yaml"
    ))
    .unwrap();
    let PhysicsProofCase::DryingSpecimen { mechanics, .. } = &mut config.case else {
        unreachable!()
    };
    mechanics.as_mut().unwrap().maximum_iterations = 2000;
    mechanics.as_mut().unwrap().initial_guess = MechanicsInitialGuess::PreviousStep;
    let geometry = serde_yaml_ng::from_str(include_str!(
        "../../../../../project/physics/geometry/original-basin.yaml"
    ))
    .unwrap();
    let material = serde_yaml_ng::from_str(include_str!(
        "../../../../../project/physics/materials/synthetic-wet-drying.yaml"
    ))
    .unwrap();
    let model = Arc::new(
        CompiledPhysicsProof::compile_with_resources(config, Some(geometry), Some(material))
            .unwrap(),
    );
    let mut session = PhysicsProofSession::new(model).unwrap();
    let initial = session.checkpoint();
    let mut error = None;
    let measured = allocation_counter::measure(|| {
        error = session.step().err();
    });
    assert_eq!(measured.count_total, 0);
    assert_eq!(
        error,
        Some(hearthline_sim::physics::PhysicsStepError::Numerical(
            hearthline_physics::PhysicsError::NonConvergence {
                iterations: 2000,
                residual: hearthline_physics::Scalar::from_raw(131_667),
            }
        ))
    );
    assert_eq!(session.checkpoint(), initial);
}

#[test]
fn ssor_refined_equilibrium_completes_and_resumes_all_source_steps() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let config: PhysicsProofConfig = serde_yaml_ng::from_str(include_str!(
                "../../../../../project/physics/proofs/basin-wet-reconstructed.yaml"
            ))
            .unwrap();
            let geometry = serde_yaml_ng::from_str(include_str!(
                "../../../../../project/physics/geometry/original-basin.yaml"
            ))
            .unwrap();
            let material = serde_yaml_ng::from_str(include_str!(
                "../../../../../project/physics/materials/synthetic-wet-drying.yaml"
            ))
            .unwrap();
            let model = Arc::new(
                CompiledPhysicsProof::compile_with_resources(
                    config,
                    Some(geometry),
                    Some(material),
                )
                .unwrap(),
            );
            assert_eq!(model.mechanics().unwrap().limits().iterations, 1000);
            assert_eq!(model.config().steps, 4);
            let mut session = PhysicsProofSession::new(model).unwrap();
            let mut midpoint = None;
            let mut reports = Vec::new();
            for step in 1..=4 {
                let allocation = allocation_counter::measure(|| {
                    session.step().unwrap();
                });
                assert_eq!(allocation.count_total, 0);
                let snapshot = session.checkpoint();
                let report = snapshot.mechanics.as_ref().unwrap();
                assert!(report.iterations <= 1000 && report.residual_raw <= 10_000);
                reports.push((report.iterations, report.residual_raw));
                if step == 2 {
                    midpoint = Some(snapshot);
                }
            }
            let final_state = session.checkpoint();
            assert_eq!(
                reports,
                [(191, 8229), (240, 8767), (317, 7981), (359, 9233)]
            );
            assert_eq!(
                final_state.digest,
                "ea31345903c06a614208d12ccfdc63e06a281e0d8de61aa1612a7f40eb85740e"
            );
            let allocation = allocation_counter::measure(|| {
                session.restore(midpoint.as_ref().unwrap()).unwrap();
                session.step().unwrap();
                session.step().unwrap();
            });
            assert_eq!(allocation.count_total, 0);
            assert_eq!(session.checkpoint(), final_state);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn opt_in_changes_identity_and_bank_layout_but_not_worker_or_snapshot_sizes() {
    let config = config();
    let zero = CompiledPhysicsProof::compile(config.clone()).unwrap();
    let mut previous = config;
    let PhysicsProofCase::DryingSpecimen { mechanics, .. } = &mut previous.case else {
        unreachable!()
    };
    mechanics.as_mut().unwrap().initial_guess = MechanicsInitialGuess::PreviousStep;
    let previous = CompiledPhysicsProof::compile(previous).unwrap();
    assert_ne!(zero.digest(), previous.digest());
    assert!(zero.mechanics().unwrap().reconstructible());
    assert!(!previous.mechanics().unwrap().reconstructible());
    assert!(
        !serde_json::to_string(previous.config())
            .unwrap()
            .contains("initial_guess")
    );
    let zero_layout = zero.storage_layout();
    let previous_layout = previous.storage_layout();
    assert_eq!(zero_layout.state_bytes, zero_layout.cells * 16);
    assert_eq!(
        previous_layout.state_bytes - zero_layout.state_bytes,
        zero_layout.nodes * 24
    );
    assert_eq!(
        zero_layout.instance_buffer_bytes(),
        previous_layout.instance_buffer_bytes()
    );
    assert_eq!(
        zero_layout.checkpoint_field_bytes,
        previous_layout.checkpoint_field_bytes
    );
    assert_eq!(
        zero_layout.immutable_bytes(),
        previous_layout.immutable_bytes()
    );
}

#[test]
fn reconstructed_fields_match_independent_execution_with_no_sealed_allocations() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let model = Arc::new(CompiledPhysicsProof::compile(config()).unwrap());
            let memory = PopulationMemoryPlan::compile(&model, 3).unwrap();
            let mut bank = None;
            let allocation = allocation_counter::measure(|| {
                bank = Some(PhysicsProofPopulation::new(model.clone(), 3).unwrap());
            });
            assert_eq!(allocation.bytes_total, memory.construction_allocation_bytes);
            let mut bank = bank.unwrap();
            let mut independent: Vec<_> = (0..3)
                .map(|_| PhysicsProofSession::new(model.clone()).unwrap())
                .collect();
            let initial = independent[0].checkpoint();
            for _ in 0..3 {
                for index in [2, 0, 2, 1, 0] {
                    let allocations = allocation_counter::measure(|| {
                        bank.step(index).unwrap();
                        independent[index].step().unwrap();
                        bank.write_snapshot(std::io::sink()).unwrap();
                    });
                    assert_eq!(allocations.count_total, 0);
                    for (slot, session) in independent.iter().enumerate() {
                        assert_eq!(bank.checkpoint(slot).unwrap(), session.checkpoint());
                    }
                }
            }
            let mut archive = Vec::new();
            bank.write_snapshot(&mut archive).unwrap();
            let mut restored =
                PhysicsProofPopulation::read_snapshot(model, Cursor::new(archive)).unwrap();
            for index in [1, 2, 0, 1, 0] {
                bank.step(index).unwrap();
                restored.step(index).unwrap();
                independent[index].step().unwrap();
                assert_eq!(
                    bank.checkpoint(index).unwrap(),
                    independent[index].checkpoint()
                );
                assert_eq!(
                    restored.checkpoint(index).unwrap(),
                    independent[index].checkpoint()
                );
            }
            restored.restore(0, &initial).unwrap();
            assert_eq!(restored.checkpoint(0).unwrap(), initial);
            assert_eq!(restored.checkpoint(1).unwrap(), independent[1].checkpoint());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn reconstruction_failure_keeps_every_product_at_its_committed_boundary() {
    let mut config = config();
    config.timestep_s = "10".into();
    let PhysicsProofCase::DryingSpecimen { mechanics, .. } = &mut config.case else {
        unreachable!()
    };
    mechanics.as_mut().unwrap().maximum_iterations = 1;
    mechanics.as_mut().unwrap().residual_tolerance = "0.000000001".into();
    let model = Arc::new(CompiledPhysicsProof::compile(config).unwrap());
    let mut bank = PhysicsProofPopulation::new(model, 3).unwrap();
    let initial = bank.checkpoint(0).unwrap();
    let allocations = allocation_counter::measure(|| {
        for index in [2, 0, 1, 2] {
            assert!(bank.step(index).is_err());
        }
    });
    assert_eq!(allocations.count_total, 0);
    for index in 0..3 {
        assert_eq!(bank.checkpoint(index).unwrap(), initial);
    }
}

#[test]
fn zero_start_and_previous_start_solve_the_same_law_without_rewriting_old_replay() {
    let mut previous = config();
    let zero = PhysicsReplay::record(previous.clone()).unwrap();
    zero.verify().unwrap();
    let PhysicsProofCase::DryingSpecimen { mechanics, .. } = &mut previous.case else {
        unreachable!()
    };
    mechanics.as_mut().unwrap().initial_guess = MechanicsInitialGuess::PreviousStep;
    let previous = PhysicsReplay::record(previous).unwrap();
    assert_eq!(
        zero.final_state.first_field_raw,
        previous.final_state.first_field_raw
    );
    assert_eq!(
        zero.final_state.temperature_k_raw,
        previous.final_state.temperature_k_raw
    );
    assert_eq!(
        zero.final_state.balance_error_raw,
        previous.final_state.balance_error_raw
    );
    let zero = zero.final_state.mechanics.unwrap();
    let previous = previous.final_state.mechanics.unwrap();
    assert_ne!(zero, previous);
    assert!(zero.residual_raw <= 100 && previous.residual_raw <= 100);
    // An independent physical-space comparison, not bit equivalence to the old
    // initial guess. Reference length is 0.06 m; this bound is 0.6 micrometres.
    let displacement_error = zero
        .displacement_raw
        .iter()
        .zip(&previous.displacement_raw)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    assert!(displacement_error < 10_000, "{displacement_error}");
}

#[test]
fn wet_reconstruction_preserves_golden_fields_across_pacing_archive_and_restart() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let replay: PhysicsReplay = serde_json::from_str(include_str!(
                "../../../../../project/physics/replays/wet-elastic-zero.json"
            ))
            .unwrap();
            let model = Arc::new(
                CompiledPhysicsProof::compile_with_resources(
                    replay.config.clone(),
                    replay.geometry.clone(),
                    replay.material.clone(),
                )
                .unwrap(),
            );
            let mut bank = PhysicsProofPopulation::new(model.clone(), 3).unwrap();
            bank.restore(1, &replay.checkpoints[0]).unwrap();
            let mut archive = Vec::new();
            bank.write_snapshot(&mut archive).unwrap();
            let mut bank =
                PhysicsProofPopulation::read_snapshot(model.clone(), Cursor::new(archive)).unwrap();
            let allocation = allocation_counter::measure(|| {
                for _ in replay.checkpoints[0].step..replay.config.steps {
                    bank.step(1).unwrap();
                }
                bank.write_snapshot(std::io::sink()).unwrap();
            });
            assert_eq!(allocation.count_total, 0);
            assert_eq!(bank.checkpoint(1).unwrap(), replay.final_state);
            assert_eq!(bank.checkpoint(0).unwrap(), replay.initial);
            assert_eq!(bank.checkpoint(2).unwrap(), replay.initial);
            for rate in [
                PlaybackRate::One,
                PlaybackRate::Two,
                PlaybackRate::Five,
                PlaybackRate::Ten,
            ] {
                let mut session = PhysicsProofSession::new(model.clone()).unwrap();
                session.restore(&replay.checkpoints[0]).unwrap();
                let mut pacer = PhysicsPacer::new(model.timestep_us()).unwrap();
                pacer.set_rate(rate);
                pacer.set_paused(true);
                pacer.request_single_step().unwrap();
                session.step().unwrap();
                pacer.commit_step().unwrap();
                pacer.set_paused(false);
                for _ in replay.checkpoints[0].step + 1..replay.config.steps {
                    while !pacer.step_due() {
                        pacer.accrue(333_333).unwrap();
                    }
                    session.step().unwrap();
                    pacer.commit_step().unwrap();
                }
                assert_eq!(session.checkpoint(), replay.final_state);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
