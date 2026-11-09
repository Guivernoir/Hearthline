use hearthline_config::physics::PhysicsProofConfig;
use hearthline_config::physics::material::DryingMaterialDocument;
use hearthline_physics::Scalar;
use hearthline_project::physics::CompiledPhysicsProof;
use hearthline_project::physics::material::material_digest;

fn inputs() -> (PhysicsProofConfig, DryingMaterialDocument) {
    (
        serde_yaml_ng::from_str(include_str!(
            "../../../../project/physics/proofs/enthalpy-drying.yaml"
        ))
        .unwrap(),
        DryingMaterialDocument::from_yaml(include_str!(
            "../../../../project/physics/materials/synthetic-drying-enthalpy.yaml"
        ))
        .unwrap(),
    )
}

#[test]
fn caloric_tables_are_digest_pinned_preflighted_and_not_duplicated_in_field_storage() {
    let (config, document) = inputs();
    let planned =
        CompiledPhysicsProof::planned_storage_with_material(&config, None, Some(&document))
            .unwrap();
    let model =
        CompiledPhysicsProof::compile_with_resources(config.clone(), None, Some(document.clone()))
            .unwrap();
    assert_eq!(model.storage_layout(), planned);
    assert_eq!(model.memory().immutable_material_bytes, 4 * 16 + 3 * 24);
    assert_eq!(model.memory().compiler_scratch_bytes, 3 * 16);
    assert_eq!(model.memory().field_buffer_bytes, 3456);
    let law = model.material().unwrap().caloric().unwrap().law().unwrap();
    // The two trapezoids integrate to 22,822.5 and 50,000 J/kg.
    assert_eq!(
        law.enthalpy(Scalar::integer(350).unwrap()).unwrap(),
        Scalar::ratio(145645, 2).unwrap()
    );
    let mut changed = document.clone();
    changed
        .dry_heat_capacity_j_kg_k_by_temperature
        .as_mut()
        .unwrap()[0][1] = "810".into();
    assert!(
        CompiledPhysicsProof::compile_with_resources(config.clone(), None, Some(changed.clone()))
            .unwrap_err()
            .contains("digest mismatch")
    );
    let mut input = config;
    input.material.as_mut().unwrap().reference.sha256 = material_digest(&changed).unwrap();
    let changed = CompiledPhysicsProof::compile_with_resources(input, None, Some(changed)).unwrap();
    assert_ne!(model.digest(), changed.digest());
}

#[test]
fn caloric_configuration_cannot_be_partial_or_extrapolate_material_properties() {
    let (config, document) = inputs();
    for change in 0..12 {
        let mut input = config.clone();
        let mut material = document.clone();
        let settings = input.material.as_mut().unwrap();
        let table = material
            .dry_heat_capacity_j_kg_k_by_temperature
            .as_mut()
            .unwrap();
        match change {
            0 => settings.caloric_solver = None,
            1 => material.dry_heat_capacity_j_kg_k_by_temperature = None,
            2 => table.clear(),
            3 => table[1][0] = "273.15".into(),
            4 => table[0][0] = "270".into(),
            5 => table[2][0] = "351".into(),
            6 => table[1][1] = "901".into(),
            7 => table[0][1] = "0".into(),
            8 => table[2][1] = "-1".into(),
            9 => settings.caloric_solver.as_mut().unwrap().maximum_sweeps = 0,
            10 => {
                settings
                    .caloric_solver
                    .as_mut()
                    .unwrap()
                    .maximum_local_iterations = 0
            }
            _ => {
                settings
                    .caloric_solver
                    .as_mut()
                    .unwrap()
                    .residual_tolerance_j = "0".into()
            }
        }
        settings.reference.sha256 = material_digest(&material).unwrap();
        assert!(
            CompiledPhysicsProof::compile_with_resources(input, None, Some(material)).is_err(),
            "accepted caloric mutation {change}"
        );
    }
}

#[test]
fn liquid_law_is_validated_digest_pinned_and_charged_without_extra_runtime_fields() {
    let config: PhysicsProofConfig = serde_yaml_ng::from_str(include_str!(
        "../../../../project/physics/proofs/wet-drying.yaml"
    ))
    .unwrap();
    let document = DryingMaterialDocument::from_yaml(include_str!(
        "../../../../project/physics/materials/synthetic-wet-drying.yaml"
    ))
    .unwrap();
    let model =
        CompiledPhysicsProof::compile_with_resources(config.clone(), None, Some(document.clone()))
            .unwrap();
    assert_eq!(model.memory().field_buffer_bytes, 3456);
    assert_eq!(model.memory().immutable_material_bytes, 208);
    assert_eq!(model.memory().compiler_scratch_bytes, 48);
    assert!(
        model
            .material()
            .unwrap()
            .caloric()
            .unwrap()
            .liquid_law()
            .unwrap()
            .is_some()
    );
    for change in 0..6 {
        let mut material = document.clone();
        let mut input = config.clone();
        match change {
            0 => material.dry_heat_capacity_j_kg_k_by_temperature = None,
            1 => input.material.as_mut().unwrap().caloric_solver = None,
            2 => material
                .liquid_heat_capacity_j_kg_k_by_temperature
                .as_mut()
                .unwrap()
                .clear(),
            3 => {
                material
                    .liquid_heat_capacity_j_kg_k_by_temperature
                    .as_mut()
                    .unwrap()[0][0] = "273".into()
            }
            4 => {
                material
                    .liquid_heat_capacity_j_kg_k_by_temperature
                    .as_mut()
                    .unwrap()[1][1] = "0".into()
            }
            _ => {
                material
                    .liquid_heat_capacity_j_kg_k_by_temperature
                    .as_mut()
                    .unwrap()[1][0] = "350".into()
            }
        }
        input.material.as_mut().unwrap().reference.sha256 = material_digest(&material).unwrap();
        assert!(
            CompiledPhysicsProof::compile_with_resources(input, None, Some(material)).is_err(),
            "accepted liquid mutation {change}"
        );
    }
    let mut changed = document;
    changed
        .liquid_heat_capacity_j_kg_k_by_temperature
        .as_mut()
        .unwrap()[1][1] = "4100".into();
    assert!(
        CompiledPhysicsProof::compile_with_resources(config.clone(), None, Some(changed.clone()))
            .unwrap_err()
            .contains("digest mismatch")
    );
    let mut input = config;
    input.material.as_mut().unwrap().reference.sha256 = material_digest(&changed).unwrap();
    let changed = CompiledPhysicsProof::compile_with_resources(input, None, Some(changed)).unwrap();
    assert_ne!(model.digest(), changed.digest());
}
