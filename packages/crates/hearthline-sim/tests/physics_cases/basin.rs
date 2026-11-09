use std::io::Cursor;
use std::sync::Arc;

use hearthline_config::physics::geometry::{SolidGeometry, SolidGeometryDocument};
use hearthline_config::physics::{PhysicsProofCase, PhysicsProofConfig};
use hearthline_physics::Scalar;
use hearthline_project::physics::CompiledPhysicsProof;
use hearthline_project::physics::geometry::geometry_digest;
use hearthline_sim::physics::clock::{PhysicsPacer, PlaybackRate};
use hearthline_sim::physics::population::PhysicsProofPopulation;
use hearthline_sim::physics::{PhysicsProofSession, PhysicsReplay};

const PROOF: &str = include_str!("../../../../../project/physics/proofs/basin-drying.yaml");
const GEOMETRY: &str = include_str!("../../../../../project/physics/geometry/original-basin.yaml");

fn small_basin() -> (PhysicsProofConfig, SolidGeometryDocument) {
    let mut config: PhysicsProofConfig = serde_yaml_ng::from_str(PROOF).unwrap();
    let mut geometry: SolidGeometryDocument = serde_yaml_ng::from_str(GEOMETRY).unwrap();
    let SolidGeometry::OpenBasin { outer_extent_m, .. } = &mut geometry.solid;
    *outer_extent_m = ["0.12".into(), "0.10".into(), "0.06".into()];
    config.shape = [6, 5, 3];
    if let PhysicsProofCase::DryingSpecimen {
        mechanics: Some(mechanics),
        ..
    } = &mut config.case
    {
        mechanics.length_scale_m = "0.12".into();
    }
    config.geometry.as_mut().unwrap().sha256 = geometry_digest(&geometry).unwrap();
    (config, geometry)
}

#[test]
fn basin_fields_are_independent_and_coupled_steps_are_allocation_free() {
    let (config, geometry) = small_basin();
    let model =
        Arc::new(CompiledPhysicsProof::compile_with_geometry(config, Some(geometry)).unwrap());
    let mut first = None;
    let measured = allocation_counter::measure(|| {
        first = Some(PhysicsProofSession::new(model.clone()).unwrap());
    });
    assert_eq!(measured.bytes_total, model.memory().field_buffer_bytes);
    let mut first = first.unwrap();
    let second = PhysicsProofSession::new(model.clone()).unwrap();
    let initial = first.checkpoint();
    assert_eq!(initial.first_field_raw.len(), 66);
    assert_eq!(
        initial.mechanics.as_ref().unwrap().displacement_raw.len(),
        156 * 3
    );
    let measured = allocation_counter::measure(|| {
        first.step().unwrap();
    });
    assert_eq!(measured.count_total, 0);
    let final_state = first.checkpoint();
    assert_ne!(final_state.mechanics, initial.mechanics);
    assert!(
        final_state
            .mechanics
            .as_ref()
            .unwrap()
            .displacement_raw
            .iter()
            .any(|value| *value != 0)
    );
    assert!(
        final_state
            .first_field_raw
            .iter()
            .all(|value| *value < 200_000_000)
    );
    assert!(
        final_state
            .temperature_k_raw
            .iter()
            .any(|value| *value < 300_000_000_000)
    );
    assert_eq!(second.checkpoint(), initial);
}

#[test]
fn basin_replay_carries_its_geometry_and_continues_on_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let (config, geometry) = small_basin();
            let artifact = PhysicsReplay::record_with_geometry(config, Some(geometry)).unwrap();
            artifact.verify().unwrap();
            let model = Arc::new(
                CompiledPhysicsProof::compile_with_geometry(
                    artifact.config.clone(),
                    artifact.geometry.clone(),
                )
                .unwrap(),
            );
            let mut population = PhysicsProofPopulation::new(model.clone(), 2).unwrap();
            population.restore(1, &artifact.checkpoints[0]).unwrap();
            let mut bytes = Vec::new();
            population.write_snapshot(&mut bytes).unwrap();
            let mut restored =
                PhysicsProofPopulation::read_snapshot(model, Cursor::new(bytes)).unwrap();
            for _ in artifact.checkpoints[0].step..artifact.config.steps {
                restored.step(1).unwrap();
            }
            assert_eq!(restored.checkpoint(1).unwrap(), artifact.final_state);
            assert_eq!(restored.checkpoint(0).unwrap(), artifact.initial);
            let mut changed = artifact.clone();
            let SolidGeometry::OpenBasin {
                floor_thickness_m, ..
            } = &mut changed.geometry.as_mut().unwrap().solid;
            *floor_thickness_m = "0.04".into();
            assert!(changed.verify().unwrap_err().contains("digest mismatch"));
            changed.geometry = None;
            assert!(changed.verify().is_err());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn every_exposed_basin_face_contributes_to_the_initial_moisture_exchange() {
    let (mut config, geometry) = small_basin();
    if let PhysicsProofCase::DryingSpecimen { mechanics, .. } = &mut config.case {
        *mechanics = None;
    }
    config.timestep_s = "1".into();
    let model =
        Arc::new(CompiledPhysicsProof::compile_with_geometry(config, Some(geometry)).unwrap());
    let mut session = PhysicsProofSession::new(model.clone()).unwrap();
    let before = session.checkpoint();
    session.step().unwrap();
    let after = session.checkpoint();
    let domain = model.solid_domain().unwrap();
    let min_faces = (0..model.cell_count())
        .min_by_key(|&cell| {
            domain
                .exposed_faces(cell)
                .unwrap()
                .into_iter()
                .filter(|v| *v)
                .count()
        })
        .unwrap();
    let max_faces = (0..model.cell_count())
        .max_by_key(|&cell| {
            domain
                .exposed_faces(cell)
                .unwrap()
                .into_iter()
                .filter(|v| *v)
                .count()
        })
        .unwrap();
    assert!(after.first_field_raw[max_faces] < after.first_field_raw[min_faces]);
    assert!(
        after
            .first_field_raw
            .iter()
            .zip(before.first_field_raw)
            .all(|(after, before)| *after <= before)
    );
    assert!(
        Scalar::from_raw(after.balance_error_raw[0])
            .abs()
            .unwrap()
            .raw()
            < 1000
    );
}

#[test]
fn unresolved_surface_transfer_is_diagnostic_and_never_advances_the_basin() {
    let (mut config, geometry) = small_basin();
    config.timestep_s = "0.001".into();
    let model =
        Arc::new(CompiledPhysicsProof::compile_with_geometry(config, Some(geometry)).unwrap());
    let mut session = PhysicsProofSession::new(model).unwrap();
    let before = session.checkpoint();
    assert_eq!(
        session.step().unwrap_err(),
        hearthline_sim::physics::PhysicsStepError::Numerical(
            hearthline_physics::PhysicsError::InsufficientResolution
        )
    );
    assert_eq!(session.checkpoint(), before);
}

#[test]
fn original_basin_preflight_rejects_a_tiny_budget_before_mesh_allocation() {
    let mut config: PhysicsProofConfig = serde_yaml_ng::from_str(PROOF).unwrap();
    let geometry: SolidGeometryDocument = serde_yaml_ng::from_str(GEOMETRY).unwrap();
    config.allocation_budget_bytes = 1024;
    let measured = allocation_counter::measure(|| {
        assert!(
            CompiledPhysicsProof::compile_with_geometry(config.clone(), Some(geometry.clone()))
                .unwrap_err()
                .contains("25% reserve")
        );
    });
    assert!(measured.bytes_total < 16 * 1024);
}

#[test]
fn assembly_peak_is_rejected_even_when_resident_fields_and_mesh_fit() {
    let mut config: PhysicsProofConfig = serde_yaml_ng::from_str(PROOF).unwrap();
    let geometry: SolidGeometryDocument = serde_yaml_ng::from_str(GEOMETRY).unwrap();
    let layout = CompiledPhysicsProof::planned_storage(&config, Some(&geometry)).unwrap();
    config.allocation_budget_bytes =
        (layout.immutable_geometry_bytes + layout.instance_buffer_bytes().unwrap()).div_ceil(3) * 4;
    let measured = allocation_counter::measure(|| {
        assert!(
            CompiledPhysicsProof::compile_with_geometry(config.clone(), Some(geometry.clone()))
                .unwrap_err()
                .contains("25% reserve")
        );
    });
    assert!(measured.bytes_total < 16 * 1024);
}

#[test]
fn measured_basin_compile_peak_is_covered_by_the_scratch_plan() {
    let config: PhysicsProofConfig = serde_yaml_ng::from_str(PROOF).unwrap();
    let geometry: SolidGeometryDocument = serde_yaml_ng::from_str(GEOMETRY).unwrap();
    let mut compiled = None;
    let measured = allocation_counter::measure(|| {
        compiled =
            Some(CompiledPhysicsProof::compile_with_geometry(config, Some(geometry)).unwrap());
    });
    let model = compiled.unwrap();
    let numerical_peak =
        model.memory().immutable_mesh_bytes + model.memory().compiler_scratch_bytes;
    // Config/digest strings are host metadata, excluded from array payloads.
    assert!(measured.bytes_max >= numerical_peak);
    assert!(measured.bytes_max - numerical_peak < 16 * 1024);
    assert!(measured.bytes_current >= model.memory().immutable_mesh_bytes as i64);
    assert!(measured.bytes_current - (model.memory().immutable_mesh_bytes as i64) < 16 * 1024);
}

#[test]
fn basin_pacing_does_not_change_deformation_or_skip_physical_steps() {
    let (config, geometry) = small_basin();
    let model = Arc::new(
        CompiledPhysicsProof::compile_with_geometry(config.clone(), Some(geometry.clone()))
            .unwrap(),
    );
    let expected = PhysicsReplay::record_with_geometry(config, Some(geometry)).unwrap();
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
                pacer.accrue(model.timestep_us() / 7 + 1).unwrap();
            }
            session.step().unwrap();
            pacer.commit_step().unwrap();
        }
        assert_eq!(session.checkpoint(), expected.final_state);
    }
}

#[test]
fn sparse_mechanical_failure_rolls_back_already_solved_moisture_and_heat() {
    let (mut config, geometry) = small_basin();
    if let PhysicsProofCase::DryingSpecimen {
        mechanics: Some(mechanics),
        ..
    } = &mut config.case
    {
        mechanics.maximum_iterations = 1;
    }
    let model =
        Arc::new(CompiledPhysicsProof::compile_with_geometry(config, Some(geometry)).unwrap());
    let mut session = PhysicsProofSession::new(model).unwrap();
    let before = session.checkpoint();
    let allocations = allocation_counter::measure(|| {
        assert!(matches!(
            session.step(),
            Err(hearthline_sim::physics::PhysicsStepError::Numerical(
                hearthline_physics::PhysicsError::NonConvergence { .. }
            ))
        ));
    });
    assert_eq!(allocations.count_total, 0);
    assert_eq!(session.checkpoint(), before);
}
