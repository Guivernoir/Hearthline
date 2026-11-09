use std::{io::Cursor, sync::Arc};

use hearthline_config::physics::PhysicsProofConfig;
use hearthline_config::physics::material::DryingMaterialDocument;
use hearthline_project::physics::CompiledPhysicsProof;
use hearthline_project::physics::material::material_digest;
use hearthline_sim::physics::clock::{PhysicsPacer, PlaybackRate};
use hearthline_sim::physics::population::PhysicsProofPopulation;
use hearthline_sim::physics::{PhysicsProofSession, PhysicsReplay};

fn inputs() -> (PhysicsProofConfig, DryingMaterialDocument) {
    (
        serde_yaml_ng::from_str(include_str!(
            "../../../../../project/physics/proofs/enthalpy-drying.yaml"
        ))
        .unwrap(),
        DryingMaterialDocument::from_yaml(include_str!(
            "../../../../../project/physics/materials/synthetic-drying-enthalpy.yaml"
        ))
        .unwrap(),
    )
}

#[test]
fn caloric_coupling_reuses_storage_and_reports_original_energy_residuals() {
    let (config, material) = inputs();
    let model = Arc::new(
        CompiledPhysicsProof::compile_with_resources(config.clone(), None, Some(material.clone()))
            .unwrap(),
    );
    let mut session = None;
    let construction = allocation_counter::measure(|| {
        session = Some(PhysicsProofSession::new(model.clone()).unwrap())
    });
    assert_eq!(construction.bytes_total, model.memory().field_buffer_bytes);
    let mut session = session.unwrap();
    let measured = allocation_counter::measure(|| {
        for _ in 0..config.steps {
            session.step().unwrap();
        }
    });
    assert_eq!(measured.count_total, 0);
    let result = session.checkpoint();
    let thermal = result.diffusion.unwrap().thermal.unwrap();
    assert!(thermal.iterations > 0 && thermal.iterations <= 400);
    assert!(thermal.residual_raw <= 100);
    assert!(thermal.local_iterations >= u64::from(thermal.iterations) * model.cell_count() as u64);
    // Per-cell residual plus inventory rounding, accumulated over eight steps.
    let balance_bound = model.cell_count() as u64 * u64::from(config.steps) * 102;
    assert!(result.balance_error_raw[1].unsigned_abs() <= balance_bound);
    // This fixture's integer dt and latent heat make both products exact
    // after the shared boundary-rate rounding, irrespective of that rate.
    assert_eq!(thermal.latent_exchange_error_raw, 0);
    let mut constant_config = config;
    let mut constant_material = material;
    constant_material.dry_heat_capacity_j_kg_k_by_temperature = None;
    let settings = constant_config.material.as_mut().unwrap();
    settings.caloric_solver = None;
    settings.reference.sha256 = material_digest(&constant_material).unwrap();
    let constant =
        PhysicsReplay::record_with_resources(constant_config, None, Some(constant_material))
            .unwrap();
    assert_eq!(result.first_field_raw, constant.final_state.first_field_raw);
    assert!(
        result
            .temperature_k_raw
            .iter()
            .zip(&constant.final_state.temperature_k_raw)
            .all(|(a, b)| a < b)
    );
}

#[test]
fn caloric_population_continuation_is_independent_allocation_free_and_small_stack() {
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
                PhysicsProofPopulation::read_snapshot(model, Cursor::new(bytes)).unwrap();
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
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn caloric_pacing_and_restart_do_not_change_the_solution() {
    let (config, material) = inputs();
    let replay =
        PhysicsReplay::record_with_resources(config.clone(), None, Some(material.clone())).unwrap();
    let model = Arc::new(
        CompiledPhysicsProof::compile_with_resources(config, None, Some(material)).unwrap(),
    );
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
}

#[test]
fn caloric_iteration_and_domain_failures_roll_back_moisture_temperature_and_clock() {
    for change in 0..3 {
        let (mut config, mut material) = inputs();
        let solver = config
            .material
            .as_mut()
            .unwrap()
            .caloric_solver
            .as_mut()
            .unwrap();
        match change {
            0 => solver.maximum_sweeps = 1,
            1 => solver.maximum_local_iterations = 1,
            _ => {
                material.temperature_range_k[0] = "299.999999999".into();
                material
                    .dry_heat_capacity_j_kg_k_by_temperature
                    .as_mut()
                    .unwrap()[0] = ["299.999999999".into(), "900".into()];
            }
        }
        config.material.as_mut().unwrap().reference.sha256 = material_digest(&material).unwrap();
        let model = Arc::new(
            CompiledPhysicsProof::compile_with_resources(config, None, Some(material)).unwrap(),
        );
        let mut session = PhysicsProofSession::new(model).unwrap();
        let before = session.checkpoint();
        let measured = allocation_counter::measure(|| {
            assert!(session.step().is_err(), "accepted failure case {change}")
        });
        assert_eq!(measured.count_total, 0);
        assert_eq!(session.checkpoint(), before);
    }
}

#[test]
fn caloric_surface_uses_film_and_half_cell_resistances_in_series() {
    use hearthline_config::physics::PhysicsProofCase;
    use hearthline_physics::Scalar;
    let (mut config, material) = inputs();
    config.shape = [1; 3];
    config.steps = 1;
    if let PhysicsProofCase::DryingSpecimen {
        equilibrium_moisture_kg_kg,
        ambient_temperature_k,
        ..
    } = &mut config.case
    {
        *equilibrium_moisture_kg_kg = "0.2".into();
        *ambient_temperature_k = "290".into();
    }
    let model = Arc::new(
        CompiledPhysicsProof::compile_with_resources(config, None, Some(material)).unwrap(),
    );
    let law = model.material().unwrap().caloric().unwrap().law().unwrap();
    let mut session = PhysicsProofSession::new(model.clone()).unwrap();
    session.step().unwrap();
    let result = session.checkpoint();
    let temperature = Scalar::from_raw(result.temperature_k_raw[0]);
    let stored_change = Scalar::ratio(144, 10_000)
        .unwrap()
        .checked_mul(
            law.enthalpy(temperature)
                .unwrap()
                .checked_sub(law.enthalpy(Scalar::integer(300).unwrap()).unwrap())
                .unwrap(),
        )
        .unwrap();
    // Each face: A/(1/h + dx/(2*k)) = 0.0004/(0.125+0.0125).
    let per_face = Scalar::ratio(8, 2750).unwrap();
    let exchange = per_face
        .checked_mul(Scalar::integer(60).unwrap())
        .unwrap()
        .checked_mul(
            Scalar::integer(290)
                .unwrap()
                .checked_sub(temperature)
                .unwrap(),
        )
        .unwrap();
    assert!(stored_change.checked_sub(exchange).unwrap().abs().unwrap() <= Scalar::from_raw(100));
    assert_eq!(result.first_field_raw, [200_000_000]);
}

#[test]
fn leaving_the_caloric_domain_preserves_previously_accepted_history() {
    let (mut config, mut material) = inputs();
    material.temperature_range_k[0] = "290".into();
    material
        .dry_heat_capacity_j_kg_k_by_temperature
        .as_mut()
        .unwrap()[0] = ["290".into(), "800".into()];
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
            assert!(before.diffusion.unwrap().thermal.is_some());
            assert_eq!(session.checkpoint(), before);
            rejected = true;
            break;
        }
    }
    assert!(
        rejected,
        "cooling must report leaving the material validity interval"
    );
}

#[test]
fn replay_names_the_first_divergent_thermal_diagnostic() {
    let (config, material) = inputs();
    let replay = PhysicsReplay::record_with_resources(config, None, Some(material)).unwrap();
    for (change, field) in [
        (0, "diffusion.thermal.iterations"),
        (1, "diffusion.thermal.local_iterations"),
        (2, "diffusion.thermal.residual_raw"),
        (3, "diffusion.thermal.latent_exchange_error_raw"),
    ] {
        let mut modified = replay.clone();
        let thermal = modified.checkpoints[0]
            .diffusion
            .as_mut()
            .unwrap()
            .thermal
            .as_mut()
            .unwrap();
        match change {
            0 => thermal.iterations += 1,
            1 => thermal.local_iterations += 1,
            2 => thermal.residual_raw += 1,
            _ => thermal.latent_exchange_error_raw += 1,
        }
        assert!(modified.verify().unwrap_err().contains(field));
    }
}
