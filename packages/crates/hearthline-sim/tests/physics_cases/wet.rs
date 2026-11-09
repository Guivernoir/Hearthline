use std::{io::Cursor, sync::Arc};

use hearthline_config::physics::material::DryingMaterialDocument;
use hearthline_config::physics::{PhysicsProofCase, PhysicsProofConfig};
use hearthline_project::physics::CompiledPhysicsProof;
use hearthline_project::physics::material::material_digest;
use hearthline_sim::physics::clock::{PhysicsPacer, PlaybackRate};
use hearthline_sim::physics::population::PhysicsProofPopulation;
use hearthline_sim::physics::{PhysicsProofSession, PhysicsReplay};

fn inputs() -> (PhysicsProofConfig, DryingMaterialDocument) {
    (
        serde_yaml_ng::from_str(include_str!(
            "../../../../../project/physics/proofs/wet-drying.yaml"
        ))
        .unwrap(),
        DryingMaterialDocument::from_yaml(include_str!(
            "../../../../../project/physics/materials/synthetic-wet-drying.yaml"
        ))
        .unwrap(),
    )
}

#[test]
fn wet_sensible_storage_changes_cooling_with_conservative_allocation_free_steps() {
    let (config, material) = inputs();
    let model = Arc::new(
        CompiledPhysicsProof::compile_with_resources(config.clone(), None, Some(material.clone()))
            .unwrap(),
    );
    let mut session = None;
    let constructed = allocation_counter::measure(|| {
        session = Some(PhysicsProofSession::new(model.clone()).unwrap())
    });
    assert_eq!(constructed.bytes_total, model.memory().field_buffer_bytes);
    let mut session = session.unwrap();
    let measured = allocation_counter::measure(|| {
        for _ in 0..config.steps {
            session.step().unwrap();
        }
    });
    assert_eq!(measured.count_total, 0);
    let result = session.checkpoint();
    let thermal = result.diffusion.unwrap().thermal.unwrap();
    assert!(thermal.residual_raw <= 100);
    // Each row's energy residual, plus separately rounded dry inventory products.
    let bound = model.cell_count() as u64 * u64::from(config.steps) * 102;
    assert!(result.balance_error_raw[1].unsigned_abs() <= bound);
    let mut dry_material = material;
    dry_material.liquid_heat_capacity_j_kg_k_by_temperature = None;
    let mut dry_config = config;
    dry_config.material.as_mut().unwrap().reference.sha256 =
        material_digest(&dry_material).unwrap();
    let dry = PhysicsReplay::record_with_resources(dry_config, None, Some(dry_material)).unwrap();
    assert_eq!(result.first_field_raw, dry.final_state.first_field_raw);
    assert!(
        result
            .temperature_k_raw
            .iter()
            .zip(&dry.final_state.temperature_k_raw)
            .all(|(wet, dry)| wet > dry && *wet < 300_000_000_000)
    );
}

#[test]
fn wet_replay_and_population_resume_are_independent_on_a_one_mib_stack() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let (config, material) = inputs();
            let replay =
                PhysicsReplay::record_with_resources(config.clone(), None, Some(material.clone()))
                    .unwrap();
            replay.verify().unwrap();
            let model = Arc::new(
                CompiledPhysicsProof::compile_with_resources(config, None, Some(material)).unwrap(),
            );
            let mut population = PhysicsProofPopulation::new(model.clone(), 3).unwrap();
            population.restore(1, &replay.checkpoints[0]).unwrap();
            let mut bytes = Vec::new();
            population.write_snapshot(&mut bytes).unwrap();
            let mut population =
                PhysicsProofPopulation::read_snapshot(model.clone(), Cursor::new(bytes)).unwrap();
            let measured = allocation_counter::measure(|| {
                for _ in replay.checkpoints[0].step..replay.config.steps {
                    population.step(1).unwrap();
                }
                population.write_snapshot(std::io::sink()).unwrap();
            });
            assert_eq!(measured.count_total, 0);
            assert_eq!(population.checkpoint(1).unwrap(), replay.final_state);
            assert_eq!(population.checkpoint(0).unwrap(), replay.initial);
            assert_eq!(population.checkpoint(2).unwrap(), replay.initial);
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
                for _ in replay.checkpoints[0].step..replay.config.steps {
                    while !pacer.step_due() {
                        pacer.accrue(333_333).unwrap();
                    }
                    session.step().unwrap();
                    pacer.commit_step().unwrap();
                }
                assert_eq!(session.checkpoint(), replay.final_state);
            }
            let mut modified = replay;
            modified
                .material
                .as_mut()
                .unwrap()
                .liquid_heat_capacity_j_kg_k_by_temperature
                .as_mut()
                .unwrap()[1][1] = "1".into();
            assert!(modified.verify().unwrap_err().contains("digest mismatch"));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn wet_condensation_and_evaporation_have_opposite_mass_and_thermal_response() {
    for condensation in [false, true] {
        let (mut config, material) = inputs();
        if condensation
            && let PhysicsProofCase::DryingSpecimen {
                initial_moisture_kg_kg,
                equilibrium_moisture_kg_kg,
                ..
            } = &mut config.case
        {
            *initial_moisture_kg_kg = "0.02".into();
            *equilibrium_moisture_kg_kg = "0.2".into();
        }
        let model = Arc::new(
            CompiledPhysicsProof::compile_with_resources(config, None, Some(material)).unwrap(),
        );
        let mut session = PhysicsProofSession::new(model).unwrap();
        let before = session.checkpoint();
        session.step().unwrap();
        let after = session.checkpoint();
        assert_eq!(
            after.first_field_raw.iter().sum::<i64>() > before.first_field_raw.iter().sum(),
            condensation
        );
        assert_eq!(
            after.temperature_k_raw.iter().sum::<i64>() > before.temperature_k_raw.iter().sum(),
            condensation
        );
    }
}

#[test]
fn wet_domain_failure_preserves_accepted_energy_fields_and_diagnostics() {
    let (mut config, mut material) = inputs();
    material.temperature_range_k[0] = "295".into();
    material
        .dry_heat_capacity_j_kg_k_by_temperature
        .as_mut()
        .unwrap()[0][0] = "295".into();
    material
        .liquid_heat_capacity_j_kg_k_by_temperature
        .as_mut()
        .unwrap()[0][0] = "295".into();
    config.material.as_mut().unwrap().reference.sha256 = material_digest(&material).unwrap();
    let model = Arc::new(
        CompiledPhysicsProof::compile_with_resources(config, None, Some(material)).unwrap(),
    );
    let mut session = PhysicsProofSession::new(model).unwrap();
    let mut rejected = false;
    for _ in 0..16 {
        let before = session.checkpoint();
        let mut result = Ok(());
        let measured = allocation_counter::measure(|| result = session.step().map(|_| ()));
        assert_eq!(measured.count_total, 0);
        if result.is_err() {
            assert!(before.step > 0);
            assert_eq!(session.checkpoint(), before);
            rejected = true;
            break;
        }
    }
    assert!(rejected);
}
