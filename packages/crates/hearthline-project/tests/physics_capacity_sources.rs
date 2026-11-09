use hearthline_config::physics::capacity::{
    CapacityKernel, CapacityModelReference, PhysicsCapacityConfig, PopulationWorkload,
};
use hearthline_project::physics::CompiledPhysicsProof;
use hearthline_project::physics::capacity::{
    CapacitySources, PhysicsCapacityReport, proof_source_digest,
};

fn inputs() -> (PhysicsCapacityConfig, CapacitySources) {
    let mut config: PhysicsCapacityConfig = serde_yaml_ng::from_str(include_str!(
        "../../../../project/physics/capacity/factory-baseline.yaml"
    ))
    .unwrap();
    let sources = CapacitySources {
        proofs: vec![
            serde_yaml_ng::from_str(include_str!(
                "../../../../project/physics/proofs/basin-wet-drying.yaml"
            ))
            .unwrap(),
        ],
        geometries: vec![
            serde_yaml_ng::from_str(include_str!(
                "../../../../project/physics/geometry/original-basin.yaml"
            ))
            .unwrap(),
        ],
        materials: vec![
            serde_yaml_ng::from_str(include_str!(
                "../../../../project/physics/materials/synthetic-wet-drying.yaml"
            ))
            .unwrap(),
        ],
    };
    let profile = config
        .profiles
        .iter_mut()
        .find(|p| p.id == "product-block")
        .unwrap();
    profile.shape = sources.proofs[0].shape;
    profile.model = Some(CapacityModelReference {
        id: sources.proofs[0].id.clone(),
        sha256: proof_source_digest(&sources.proofs[0]).unwrap(),
    });
    (config, sources)
}

fn small_inputs() -> (PhysicsCapacityConfig, CapacitySources) {
    let (mut config, sources) = inputs();
    config.profiles.retain(|p| p.model.is_some());
    config.populations.retain(|p| p.id == "mould-products");
    config.populations[0].capacity = 4;
    config.populations[0].workload = PopulationWorkload::Fixed { count: 3 };
    for resource in &mut config.auxiliary {
        resource.peak_entries = 3;
        resource.capacity_entries = 4;
        resource.bytes_per_entry = 1;
    }
    (config, sources)
}

#[test]
fn source_profiles_use_runtime_geometry_material_and_compilation_layouts() {
    let (config, sources) = inputs();
    let model = CompiledPhysicsProof::compile_with_resources(
        sources.proofs[0].clone(),
        Some(sources.geometries[0].clone()),
        Some(sources.materials[0].clone()),
    )
    .unwrap();
    let report = PhysicsCapacityReport::compile_with_sources(config.clone(), sources).unwrap();
    let profile = report
        .profiles
        .iter()
        .find(|p| p.profile.model.is_some())
        .unwrap();
    assert_eq!(profile.layout, model.storage_layout());
    assert_eq!(
        profile.compiled_model_digest.as_deref(),
        Some(model.digest())
    );
    assert_eq!(
        profile.compiler_scratch_bytes,
        model.memory().compiler_scratch_bytes
    );
    assert_eq!(
        (
            profile.layout.cells,
            profile.layout.nodes,
            profile.layout.elements
        ),
        (1072, 2230, 6432)
    );
    assert_eq!(profile.layout.immutable_material_bytes, 208);
    assert_eq!(profile.layout.state_bytes, 70_672);
    assert_eq!(profile.layout.instance_buffer_bytes().unwrap(), 1_287_824);
    assert_eq!(report.immutable_material_bytes, 208);
    let slots: u64 = report
        .populations
        .iter()
        .filter(|p| p.population.profile == "product-block")
        .map(|p| p.population.capacity)
        .sum();
    assert_eq!(slots, 2778);
    assert!(!report.within_budget);
    assert!(report.within_population_capacities && report.within_auxiliary_capacities);
    assert_eq!(report.profiles.len(), 5);
    assert_eq!(report.populations.len(), 21);
    assert!(
        report
            .capacity_contract
            .starts_with("hearthline-physics-capacity/3:")
    );
    let mut original = config.populations;
    original.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(
        original,
        report
            .populations
            .into_iter()
            .map(|p| p.population)
            .collect::<Vec<_>>()
    );
}

#[test]
fn compile_peak_can_reject_a_study_whose_runtime_arrays_fit() {
    let (mut config, sources) = small_inputs();
    let full =
        PhysicsCapacityReport::compile_with_sources(config.clone(), sources.clone()).unwrap();
    assert!(full.compiler_peak_bytes > full.runtime_planned_bytes);
    assert_eq!(full.total_planned_bytes, full.compiler_peak_bytes);
    assert_eq!(
        full.compiler_peak_bytes,
        full.immutable_geometry_bytes + full.immutable_material_bytes + 22_237_912 + 20
    );
    config.allocation_budget_bytes = full.runtime_planned_bytes * 4 / 3 + 4;
    let report = PhysicsCapacityReport::compile_with_sources(config, sources).unwrap();
    assert!(report.runtime_planned_bytes <= report.reviewed_budget_bytes);
    assert!(report.compiler_peak_bytes > report.reviewed_budget_bytes);
    assert!(!report.within_budget);
    assert!(report.require_fit().is_err());
}

#[test]
fn source_order_and_yaml_presentation_do_not_change_identity_or_report() {
    let (mut config, mut sources) = inputs();
    let proof = serde_yaml_ng::from_str(include_str!(
        "../../../../project/physics/proofs/wet-drying.yaml"
    ))
    .unwrap();
    sources.proofs.push(proof);
    let profile = config
        .profiles
        .iter_mut()
        .find(|p| p.id == "preparation-vessel")
        .unwrap();
    profile.shape = sources.proofs[1].shape;
    profile.model = Some(CapacityModelReference {
        id: sources.proofs[1].id.clone(),
        sha256: proof_source_digest(&sources.proofs[1]).unwrap(),
    });
    let first =
        PhysicsCapacityReport::compile_with_sources(config.clone(), sources.clone()).unwrap();
    let json = serde_json::to_string_pretty(&sources.proofs[0]).unwrap();
    let parsed = serde_yaml_ng::from_str(&json).unwrap();
    assert_eq!(
        proof_source_digest(&sources.proofs[0]).unwrap(),
        proof_source_digest(&parsed).unwrap()
    );
    config.profiles.reverse();
    config.populations.reverse();
    config.auxiliary.reverse();
    sources.proofs.reverse();
    assert_eq!(
        first,
        PhysicsCapacityReport::compile_with_sources(config, sources).unwrap()
    );
}

#[test]
fn sources_are_required_validated_and_never_silently_ignored() {
    for mutation in 0..14 {
        let (mut config, mut sources) = small_inputs();
        match mutation {
            0 => sources.proofs.clear(),
            1 => sources.geometries.clear(),
            2 => sources.materials.clear(),
            3 => sources.proofs.push(sources.proofs[0].clone()),
            4 => sources.geometries.push(sources.geometries[0].clone()),
            5 => sources.materials.push(sources.materials[0].clone()),
            6 => {
                let mut p = sources.proofs[0].clone();
                p.id = "unused".into();
                sources.proofs.push(p);
            }
            7 => {
                let mut p = sources.geometries[0].clone();
                p.id = "unused".into();
                sources.geometries.push(p);
            }
            8 => {
                let mut p = sources.materials[0].clone();
                p.id = "unused".into();
                sources.materials.push(p);
            }
            9 => config.profiles[0].shape[0] += 1,
            10 => config.profiles[0].kernel = CapacityKernel::CompressibleFlow,
            11 => config.profiles[0].model.as_mut().unwrap().sha256 = "0".repeat(64),
            12 => sources.geometries[0].schema_version = "99.0.0".into(),
            _ => {
                sources.materials[0]
                    .liquid_heat_capacity_j_kg_k_by_temperature
                    .as_mut()
                    .unwrap()[1][1] = "0".into()
            }
        }
        assert!(
            PhysicsCapacityReport::compile_with_sources(config, sources).is_err(),
            "mutation {mutation}"
        );
    }
    let (config, _) = small_inputs();
    assert!(
        PhysicsCapacityReport::compile(config)
            .unwrap_err()
            .contains("missing")
    );
}

#[test]
fn a_valid_digest_cannot_bypass_semantic_validation_or_host_policy() {
    let (mut config, mut sources) = small_inputs();
    sources.proofs[0].timestep_s = "-1".into();
    config.profiles[0].model.as_mut().unwrap().sha256 =
        proof_source_digest(&sources.proofs[0]).unwrap();
    assert!(PhysicsCapacityReport::compile_with_sources(config, sources).is_err());
    let (mut config, mut sources) = small_inputs();
    sources.proofs[0].allocation_budget_bytes += 1;
    config.profiles[0].model.as_mut().unwrap().sha256 =
        proof_source_digest(&sources.proofs[0]).unwrap();
    assert!(
        PhysicsCapacityReport::compile_with_sources(config, sources)
            .unwrap_err()
            .contains("host allocation policy")
    );
}

#[test]
fn material_layout_and_solver_changes_are_part_of_capacity_evidence() {
    let (mut config, mut sources) = small_inputs();
    let original =
        PhysicsCapacityReport::compile_with_sources(config.clone(), sources.clone()).unwrap();
    sources.proofs[0].maximum_iterations += 1;
    config.profiles[0].model.as_mut().unwrap().sha256 =
        proof_source_digest(&sources.proofs[0]).unwrap();
    let changed = PhysicsCapacityReport::compile_with_sources(config, sources).unwrap();
    assert_ne!(original.digest, changed.digest);
    assert_ne!(
        original.profiles[0].compiled_model_digest,
        changed.profiles[0].compiled_model_digest
    );
    assert_eq!(original.profiles[0].layout, changed.profiles[0].layout);
}
