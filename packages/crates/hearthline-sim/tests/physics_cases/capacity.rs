use std::sync::Arc;

use hearthline_config::physics::capacity::PhysicsCapacityConfig;
use hearthline_project::physics::CompiledPhysicsProof;
use hearthline_project::physics::capacity::CapacitySources;
use hearthline_sim::physics::PhysicsProofSession;
use hearthline_sim::physics::population::{PhysicsProofPopulation, PooledCapacityComparison};

fn inputs() -> (PhysicsCapacityConfig, CapacitySources) {
    (
        serde_yaml_ng::from_str(include_str!(
            "../../../../../project/physics/capacity/factory-basin-study.yaml"
        ))
        .unwrap(),
        CapacitySources {
            proofs: vec![
                serde_yaml_ng::from_str(include_str!(
                    "../../../../../project/physics/proofs/basin-wet-drying.yaml"
                ))
                .unwrap(),
            ],
            geometries: vec![
                serde_yaml_ng::from_str(include_str!(
                    "../../../../../project/physics/geometry/original-basin.yaml"
                ))
                .unwrap(),
            ],
            materials: vec![
                serde_yaml_ng::from_str(include_str!(
                    "../../../../../project/physics/materials/synthetic-wet-drying.yaml"
                ))
                .unwrap(),
            ],
        },
    )
}

#[test]
fn basin_study_preserves_workloads_and_accounts_for_actual_shared_resources() {
    let (mut config, sources) = inputs();
    let baseline: PhysicsCapacityConfig = serde_yaml_ng::from_str(include_str!(
        "../../../../../project/physics/capacity/factory-baseline.yaml"
    ))
    .unwrap();
    let report = PooledCapacityComparison::compile_with_sources(config.clone(), sources).unwrap();
    assert_eq!(report.state_bank_bytes, 203_724_192);
    assert_eq!(report.slot_metadata_bytes, 817_080);
    assert_eq!(report.worker_buffer_bytes, 3_690_128);
    assert_eq!(report.immutable_geometry_bytes, 3_388_748);
    assert_eq!(report.immutable_material_bytes, 208);
    // The process atmosphere now has the largest serialized checkpoint.
    assert_eq!(report.archive_decoder_envelope_bytes, 11_399_168);
    assert_eq!(report.compiler_peak_bytes, 210_176_244);
    assert_eq!(report.runtime_planned_bytes, 407_568_900);
    assert_eq!(report.total_planned_bytes, report.runtime_planned_bytes);
    report.require_fit().unwrap();
    assert_eq!(report.baseline.total_planned_bytes, 5_935_411_196);
    assert!(!report.baseline.within_budget);
    assert_eq!(report.baseline.qualification_gaps.len(), 5);
    for (current, previous) in config.profiles.iter().zip(&baseline.profiles) {
        if current.id != "product-basin" {
            assert_eq!(current, previous);
        }
    }
    for population in &mut config.populations {
        if population.profile == "product-basin" {
            population.profile = "product-block".into();
        }
    }
    config.id = baseline.id.clone();
    config.profiles = baseline.profiles.clone();
    assert_eq!(config, baseline);
}

#[test]
fn all_reserved_basin_slots_match_measured_allocations_on_a_one_mib_stack() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let (config, sources) = inputs();
            let report =
                PooledCapacityComparison::compile_with_sources(config, sources.clone()).unwrap();
            let expected = report
                .profiles
                .iter()
                .find(|p| p.profile == "product-basin")
                .unwrap()
                .memory;
            assert_eq!(expected.instances, 2778);
            assert_eq!(expected.construction_allocation_bytes, 198_348_032);
            let model = Arc::new(
                CompiledPhysicsProof::compile_with_resources(
                    sources.proofs[0].clone(),
                    Some(sources.geometries[0].clone()),
                    Some(sources.materials[0].clone()),
                )
                .unwrap(),
            );
            let mut population = None;
            let measured = allocation_counter::measure(|| {
                population = Some(
                    PhysicsProofPopulation::new(model.clone(), expected.instances as usize)
                        .unwrap(),
                );
            });
            assert_eq!(measured.bytes_total, expected.construction_allocation_bytes);
            let mut population = population.unwrap();
            let last = expected.instances as usize - 1;
            let initial = population.checkpoint(last).unwrap();
            let measured = allocation_counter::measure(|| {
                population.step(0).unwrap();
                population.step(last / 2).unwrap();
            });
            assert_eq!(measured.count_total, 0);
            assert_eq!(population.checkpoint(last).unwrap(), initial);
            assert_eq!(population.checkpoint(1).unwrap(), initial);
            assert_eq!(
                population.checkpoint(0).unwrap(),
                population.checkpoint(last / 2).unwrap()
            );
            let mut resumed = PhysicsProofSession::new(model).unwrap();
            resumed.restore(&population.checkpoint(0).unwrap()).unwrap();
            population.step(0).unwrap();
            resumed.step().unwrap();
            assert_eq!(population.checkpoint(0).unwrap(), resumed.checkpoint());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn pooled_runtime_fit_does_not_hide_excess_compiler_memory() {
    let (mut config, sources) = inputs();
    config.profiles.retain(|p| p.model.is_some());
    config.populations.retain(|p| p.id == "mould-products");
    for resource in &mut config.auxiliary {
        resource.peak_entries = 3;
        resource.capacity_entries = 4;
        resource.bytes_per_entry = 1;
    }
    let original =
        PooledCapacityComparison::compile_with_sources(config.clone(), sources.clone()).unwrap();
    assert!(original.compiler_peak_bytes > original.runtime_planned_bytes);
    config.allocation_budget_bytes = original.runtime_planned_bytes * 4 / 3 + 4;
    let report = PooledCapacityComparison::compile_with_sources(config, sources).unwrap();
    assert!(report.runtime_planned_bytes <= report.baseline.reviewed_budget_bytes);
    assert_eq!(report.total_planned_bytes, report.compiler_peak_bytes);
    assert!(report.require_fit().is_err());
}

#[test]
fn refined_basin_keeps_the_workload_and_is_rejected_before_allocating_its_bank() {
    let (coarse, mut sources) = inputs();
    let refined: PhysicsCapacityConfig = serde_yaml_ng::from_str(include_str!(
        "../../../../../project/physics/capacity/factory-basin-refinement.yaml"
    ))
    .unwrap();
    let proof = serde_yaml_ng::from_str(include_str!(
        "../../../../../project/physics/proofs/basin-wet-refined.yaml"
    ))
    .unwrap();
    let original_proof = std::mem::replace(&mut sources.proofs[0], proof);
    let mut normalized = sources.proofs[0].clone();
    normalized.id = original_proof.id.clone();
    normalized.shape = original_proof.shape;
    normalized.spacing_m = original_proof.spacing_m.clone();
    assert_eq!(normalized, original_proof);
    assert_eq!(refined.populations, coarse.populations);
    assert_eq!(refined.auxiliary, coarse.auxiliary);
    assert_eq!(
        refined.allocation_budget_bytes,
        coarse.allocation_budget_bytes
    );
    assert_eq!(refined.reserve_percent, coarse.reserve_percent);
    let report = PooledCapacityComparison::compile_with_sources(refined, sources.clone()).unwrap();
    assert_eq!(report.state_bank_bytes, 1_263_253_392);
    assert_eq!(report.total_planned_bytes, 1_556_311_074);
    assert_eq!(report.compiler_peak_bytes, 386_076_288);
    assert_eq!(report.baseline.reviewed_budget_bytes, 1_207_959_552);
    assert!(report.within_workload_capacities);
    assert!(report.require_fit().is_err());
    let product = report
        .baseline
        .profiles
        .iter()
        .find(|p| p.profile.model.is_some())
        .unwrap();
    assert_eq!(
        (
            product.layout.cells,
            product.layout.nodes,
            product.layout.elements
        ),
        (8576, 13119, 51456)
    );
    let model = Arc::new(
        CompiledPhysicsProof::compile_with_resources(
            sources.proofs.remove(0),
            Some(sources.geometries.remove(0)),
            Some(sources.materials.remove(0)),
        )
        .unwrap(),
    );
    let measured = allocation_counter::measure(|| {
        assert!(PhysicsProofPopulation::new(model.clone(), 2778).is_err());
    });
    assert!(measured.bytes_total < 4096);
}

#[test]
fn reconstructed_refined_bank_keeps_all_slots_mesh_fields_and_reserve() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let (coarse, mut sources) = inputs();
            let config: PhysicsCapacityConfig = serde_yaml_ng::from_str(include_str!(
                "../../../../../project/physics/capacity/factory-basin-reconstructed.yaml"
            ))
            .unwrap();
            sources.proofs[0] = serde_yaml_ng::from_str(include_str!(
                "../../../../../project/physics/proofs/basin-wet-reconstructed.yaml"
            ))
            .unwrap();
            let previous: hearthline_config::physics::PhysicsProofConfig = serde_yaml_ng::from_str(
                include_str!("../../../../../project/physics/proofs/basin-wet-refined.yaml"),
            )
            .unwrap();
            let mut normalized = sources.proofs[0].clone();
            normalized.id = previous.id.clone();
            let hearthline_config::physics::PhysicsProofCase::DryingSpecimen { mechanics, .. } =
                &mut normalized.case
            else {
                unreachable!()
            };
            mechanics.as_mut().unwrap().initial_guess =
                hearthline_config::physics::MechanicsInitialGuess::PreviousStep;
            assert_eq!(normalized, previous);
            assert_eq!(config.populations, coarse.populations);
            assert_eq!(config.auxiliary, coarse.auxiliary);
            assert_eq!(
                config.allocation_budget_bytes,
                coarse.allocation_budget_bytes
            );
            assert_eq!(config.reserve_percent, coarse.reserve_percent);
            let report =
                PooledCapacityComparison::compile_with_sources(config, sources.clone()).unwrap();
            report.require_fit().unwrap();
            assert_eq!(report.state_bank_bytes, 388_583_424);
            assert_eq!(report.total_planned_bytes, 681_641_106);
            assert_eq!(report.compiler_peak_bytes, 386_076_288);
            assert_eq!(report.baseline.total_planned_bytes, 42_292_282_548);
            assert_eq!(report.archive_decoder_envelope_bytes, 72_363_566);
            let expected = report
                .profiles
                .iter()
                .find(|p| p.profile == "product-basin")
                .unwrap()
                .memory;
            assert_eq!(expected.instances, 2778);
            assert_eq!(expected.construction_allocation_bytes, 391_202_296);
            let model = Arc::new(
                CompiledPhysicsProof::compile_with_resources(
                    sources.proofs.remove(0),
                    Some(sources.geometries.remove(0)),
                    Some(sources.materials.remove(0)),
                )
                .unwrap(),
            );
            let layout = model.storage_layout();
            assert_eq!(
                (layout.cells, layout.nodes, layout.elements),
                (8576, 13119, 51456)
            );
            assert_eq!(layout.state_bytes, 137_216);
            assert_eq!(layout.checkpoint_field_bytes, 2_921_960);
            assert_eq!(layout.instance_buffer_bytes().unwrap(), 9_282_856);
            let mut bank = None;
            let measured = allocation_counter::measure(|| {
                bank = Some(PhysicsProofPopulation::new(model.clone(), 2778).unwrap());
            });
            assert_eq!(measured.bytes_total, expected.construction_allocation_bytes);
            let mut bank = bank.unwrap();
            let initial = bank.checkpoint(2777).unwrap();
            let measured = allocation_counter::measure(|| {
                bank.step(1389).unwrap();
            });
            assert_eq!(measured.count_total, 0);
            assert_eq!(bank.checkpoint(2777).unwrap(), initial);
            assert_eq!(bank.checkpoint(0).unwrap(), initial);
            let advanced = bank.checkpoint(1389).unwrap();
            let mechanics = advanced.mechanics.as_ref().unwrap();
            assert_eq!(mechanics.iterations, 191);
            assert_eq!(mechanics.residual_raw, 8229);
            assert_eq!(mechanics.displacement_raw.len(), 13119 * 3);
            assert_eq!(mechanics.stress_raw.len(), 51456 * 6);
            let mut resumed = PhysicsProofSession::new(model).unwrap();
            let measured = allocation_counter::measure(|| resumed.restore(&advanced).unwrap());
            assert_eq!(measured.count_total, 0);
            assert_eq!(resumed.checkpoint(), advanced);
        })
        .unwrap()
        .join()
        .unwrap();
}
