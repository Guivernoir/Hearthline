use std::io::Cursor;
use std::sync::Arc;

use hearthline_config::physics::material::{
    DryingMaterialConfig, DryingMaterialDocument, MaterialReference,
};
use hearthline_config::physics::{PhysicsProofCase, PhysicsProofConfig};
use hearthline_project::physics::CompiledPhysicsProof;
use hearthline_project::physics::material::material_digest;
use hearthline_sim::physics::clock::{PhysicsPacer, PlaybackRate};
use hearthline_sim::physics::population::PhysicsProofPopulation;
use hearthline_sim::physics::{PhysicsProofSession, PhysicsReplay};

const DOCUMENT: &str =
    include_str!("../../../../../project/physics/materials/synthetic-moisture-diffusion.yaml");
const PROOF: &str = include_str!("../../../../../project/physics/proofs/drying-specimen.yaml");

fn inputs() -> (PhysicsProofConfig, DryingMaterialDocument) {
    let document = DryingMaterialDocument::from_yaml(DOCUMENT).unwrap();
    let mut config: PhysicsProofConfig = serde_yaml_ng::from_str(PROOF).unwrap();
    config.material = Some(DryingMaterialConfig {
        caloric_solver: None,
        reference: MaterialReference {
            id: document.id.clone(),
            sha256: material_digest(&document).unwrap(),
        },
        maximum_iterations: 100,
        residual_tolerance: "0.0000001".into(),
        relaxation: "1".into(),
    });
    config.timestep_s = "10".into();
    config.steps = 8;
    if let PhysicsProofCase::DryingSpecimen {
        surface_transfer_m_s,
        ..
    } = &mut config.case
    {
        *surface_transfer_m_s = "0.000003".into();
    }
    (config, document)
}

#[test]
fn material_drying_steps_are_preallocated_and_respond_to_the_declared_law() {
    let (config, document) = inputs();
    let model = Arc::new(
        CompiledPhysicsProof::compile_with_resources(config.clone(), None, Some(document.clone()))
            .unwrap(),
    );
    let mut session = None;
    let measured = allocation_counter::measure(|| {
        session = Some(PhysicsProofSession::new(model.clone()).unwrap())
    });
    assert_eq!(measured.bytes_total, model.memory().field_buffer_bytes);
    let mut session = session.unwrap();
    let initial = session.checkpoint();
    let measured = allocation_counter::measure(|| {
        for _ in 0..config.steps {
            session.step().unwrap();
        }
    });
    assert_eq!(measured.count_total, 0);
    let actual = session.checkpoint();
    assert!(
        actual
            .first_field_raw
            .iter()
            .zip(&initial.first_field_raw)
            .all(|(a, b)| a < b)
    );
    assert!(
        actual
            .temperature_k_raw
            .iter()
            .any(|&v| v < 300_000_000_000)
    );
    assert!(actual.diffusion.unwrap().iterations > 0);
    assert!(actual.diffusion.unwrap().residual_raw <= 100);
    // Summed per-cell equation tolerance, with two raw units for inventory
    // product rounding. This bounds the ledger without fitting observed output.
    let balance_bound = model.cell_count() as i64 * i64::from(config.steps) * 102;
    assert!(
        actual
            .balance_error_raw
            .iter()
            .all(|v| v.unsigned_abs() <= balance_bound as u64)
    );
    assert!(
        actual
            .temperature_k_raw
            .iter()
            .all(|&v| v >= 273_150_000_000)
    );
    let mut slow_document = document;
    for knot in &mut slow_document.diffusivity_multiplier_by_moisture {
        knot[1] = "0.01".into();
    }
    let mut slow = config;
    slow.material.as_mut().unwrap().reference.sha256 = material_digest(&slow_document).unwrap();
    let slow = PhysicsReplay::record_with_resources(slow, None, Some(slow_document)).unwrap();
    assert!(
        actual.first_field_raw.iter().sum::<i64>()
            < slow.final_state.first_field_raw.iter().sum::<i64>()
    );
}

#[test]
fn material_replay_and_streamed_population_resume_on_a_constrained_stack() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let (config, document) = inputs();
            let replay =
                PhysicsReplay::record_with_resources(config, None, Some(document)).unwrap();
            let encoded = serde_json::to_string(&replay).unwrap();
            let replay: PhysicsReplay = serde_json::from_str(&encoded).unwrap();
            replay.verify().unwrap();
            let model = Arc::new(
                CompiledPhysicsProof::compile_with_resources(
                    replay.config.clone(),
                    None,
                    replay.material.clone(),
                )
                .unwrap(),
            );
            let mut population = PhysicsProofPopulation::new(model.clone(), 3).unwrap();
            population.restore(1, &replay.checkpoints[0]).unwrap();
            let mut bytes = Vec::new();
            population.write_snapshot(&mut bytes).unwrap();
            let mut population =
                PhysicsProofPopulation::read_snapshot(model, Cursor::new(bytes)).unwrap();
            let untouched = population.checkpoint(2).unwrap();
            let measured = allocation_counter::measure(|| {
                for _ in replay.checkpoints[0].step..replay.config.steps {
                    population.step(1).unwrap();
                }
                population.write_snapshot(std::io::sink()).unwrap();
            });
            assert_eq!(measured.count_total, 0);
            assert_eq!(population.checkpoint(1).unwrap(), replay.final_state);
            assert_eq!(population.checkpoint(2).unwrap(), untouched);
            assert_eq!(population.checkpoint(0).unwrap(), replay.initial);
            let mut changed = replay;
            changed
                .material
                .as_mut()
                .unwrap()
                .diffusivity_multiplier_by_moisture[0][1] = "1".into();
            assert!(changed.verify().unwrap_err().contains("digest mismatch"));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn material_pacing_is_independent_of_requested_speed() {
    let (config, document) = inputs();
    let expected =
        PhysicsReplay::record_with_resources(config.clone(), None, Some(document.clone())).unwrap();
    let model = Arc::new(
        CompiledPhysicsProof::compile_with_resources(config, None, Some(document)).unwrap(),
    );
    for rate in [
        PlaybackRate::One,
        PlaybackRate::Two,
        PlaybackRate::Five,
        PlaybackRate::Ten,
    ] {
        let mut session = PhysicsProofSession::new(model.clone()).unwrap();
        let mut pacer = PhysicsPacer::new(model.timestep_us()).unwrap();
        pacer.set_rate(rate);
        for _ in 0..model.config().steps {
            while !pacer.step_due() {
                pacer.accrue(333_333).unwrap();
            }
            session.step().unwrap();
            pacer.commit_step().unwrap();
        }
        assert_eq!(session.checkpoint(), expected.final_state);
    }
}

#[test]
fn nonlinear_failure_and_invalid_thermal_domain_roll_back_all_fields_and_ledgers() {
    for thermal_failure in [false, true] {
        let (mut config, mut document) = inputs();
        if thermal_failure {
            document.temperature_range_k[0] = "299.999999999".into();
            config.material.as_mut().unwrap().reference.sha256 =
                material_digest(&document).unwrap();
        } else {
            config.material.as_mut().unwrap().maximum_iterations = 1;
            config.material.as_mut().unwrap().relaxation = "0.01".into();
        }
        let model = Arc::new(
            CompiledPhysicsProof::compile_with_resources(config, None, Some(document)).unwrap(),
        );
        let mut session = PhysicsProofSession::new(model).unwrap();
        let before = session.checkpoint();
        let measured = allocation_counter::measure(|| assert!(session.step().is_err()));
        assert_eq!(measured.count_total, 0);
        assert_eq!(session.checkpoint(), before);
    }
}

#[test]
fn sparse_basin_material_and_mechanics_share_one_atomic_step_and_replay() {
    use hearthline_config::physics::geometry::{SolidGeometry, SolidGeometryDocument};
    use hearthline_project::physics::geometry::geometry_digest;
    for kind in 0..3 {
        let (input, document) = if kind == 2 {
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
        } else if kind == 1 {
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
        } else {
            inputs()
        };
        let mut config: PhysicsProofConfig = serde_yaml_ng::from_str(include_str!(
            "../../../../../project/physics/proofs/basin-drying.yaml"
        ))
        .unwrap();
        let mut geometry = SolidGeometryDocument::from_yaml(include_str!(
            "../../../../../project/physics/geometry/original-basin.yaml"
        ))
        .unwrap();
        let SolidGeometry::OpenBasin { outer_extent_m, .. } = &mut geometry.solid;
        *outer_extent_m = ["0.12".into(), "0.10".into(), "0.06".into()];
        config.shape = [6, 5, 3];
        config.material = input.material;
        config.geometry.as_mut().unwrap().sha256 = geometry_digest(&geometry).unwrap();
        config.steps = 2;
        let model = Arc::new(
            CompiledPhysicsProof::compile_with_resources(
                config.clone(),
                Some(geometry.clone()),
                Some(document.clone()),
            )
            .unwrap(),
        );
        assert_eq!(
            model.memory().immutable_material_bytes,
            [64, 136, 208][kind]
        );
        let mut session = None;
        let measured = allocation_counter::measure(|| {
            session = Some(PhysicsProofSession::new(model.clone()).unwrap())
        });
        assert_eq!(measured.bytes_total, model.memory().field_buffer_bytes);
        let mut session = session.unwrap();
        let measured = allocation_counter::measure(|| {
            session.step().unwrap();
        });
        assert_eq!(measured.count_total, 0);
        let checkpoint = session.checkpoint();
        assert!(
            checkpoint
                .mechanics
                .as_ref()
                .unwrap()
                .displacement_raw
                .iter()
                .any(|v| *v != 0)
        );
        assert!(checkpoint.diffusion.is_some());
        let replay = PhysicsReplay::record_with_resources(
            config.clone(),
            Some(geometry.clone()),
            Some(document.clone()),
        )
        .unwrap();
        replay.verify().unwrap();
        let mut resumed = PhysicsProofSession::new(model).unwrap();
        resumed.restore(&checkpoint).unwrap();
        resumed.step().unwrap();
        assert_eq!(resumed.checkpoint(), replay.final_state);
        if let PhysicsProofCase::DryingSpecimen {
            mechanics: Some(mechanics),
            ..
        } = &mut config.case
        {
            mechanics.maximum_iterations = 1;
        }
        let mut failing = PhysicsProofSession::new(Arc::new(
            CompiledPhysicsProof::compile_with_resources(config, Some(geometry), Some(document))
                .unwrap(),
        ))
        .unwrap();
        let before = failing.checkpoint();
        assert!(failing.step().is_err());
        assert_eq!(failing.checkpoint(), before);
    }
}
