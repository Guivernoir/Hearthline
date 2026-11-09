use hearthline_config::physics::material::{
    DryingMaterialConfig, DryingMaterialDocument, MaterialProvenance, MaterialReference,
};
use hearthline_config::physics::{PhysicsProofCase, PhysicsProofConfig};
use hearthline_project::physics::CompiledPhysicsProof;
use hearthline_project::physics::material::material_digest;

const DOCUMENT: &str =
    include_str!("../../../../project/physics/materials/synthetic-moisture-diffusion.yaml");
const PROOF: &str = include_str!("../../../../project/physics/proofs/drying-specimen.yaml");

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
        residual_tolerance: "0.000001".into(),
        relaxation: "1".into(),
    });
    (config, document)
}

#[test]
fn material_storage_is_preflighted_and_typed_yaml_is_format_independent() {
    let (config, document) = inputs();
    let reordered = serde_yaml_ng::to_string(&serde_json::to_value(&document).unwrap()).unwrap();
    assert_eq!(
        material_digest(&document),
        material_digest(&DryingMaterialDocument::from_yaml(&reordered).unwrap())
    );
    let mut constant = config.clone();
    constant.material = None;
    let baseline = CompiledPhysicsProof::compile(constant).unwrap();
    let model =
        CompiledPhysicsProof::compile_with_resources(config.clone(), None, Some(document.clone()))
            .unwrap();
    assert_eq!(
        model.memory().field_buffer_bytes - baseline.memory().field_buffer_bytes,
        27 * 32
    );
    assert_eq!(model.memory().immutable_material_bytes, 4 * 16);
    assert_eq!(model.storage_layout().immutable_bytes().unwrap(), 64);
    assert_eq!(model.material().unwrap().document(), &document);
    assert_ne!(model.digest(), baseline.digest());
    assert!(CompiledPhysicsProof::planned_storage(&config, None).is_err());
    assert_eq!(
        CompiledPhysicsProof::planned_storage_with_material(&config, None, Some(&document))
            .unwrap(),
        model.storage_layout()
    );
    let mut limited = config;
    limited.allocation_budget_bytes = baseline.memory().field_buffer_bytes.div_ceil(3) * 4;
    assert!(
        CompiledPhysicsProof::compile_with_resources(limited, None, Some(document))
            .unwrap_err()
            .contains("25% reserve")
    );
}

#[test]
fn material_documents_require_valid_references_properties_and_provenance() {
    let (config, document) = inputs();
    assert!(CompiledPhysicsProof::compile(config.clone()).is_err());
    let mut missing = config.clone();
    missing.material = None;
    assert!(
        CompiledPhysicsProof::compile_with_resources(missing, None, Some(document.clone()))
            .is_err()
    );
    let mut modified = document.clone();
    modified.temperature_range_k[0] = "200".into();
    assert!(
        CompiledPhysicsProof::compile_with_resources(config.clone(), None, Some(modified))
            .unwrap_err()
            .contains("digest mismatch")
    );
    for change in 0..14 {
        let mut modified = document.clone();
        match change {
            0 => modified.schema_version = "99.0.0".into(),
            1 => modified.id = "../material".into(),
            2 => modified.temperature_range_k = ["300".into(), "300".into()],
            3 => modified.temperature_range_k = ["301".into(), "350".into()],
            4 => modified.diffusivity_multiplier_by_moisture.clear(),
            5 => modified.diffusivity_multiplier_by_moisture.truncate(1),
            6 => modified.diffusivity_multiplier_by_moisture[1][0] = "0".into(),
            7 => modified.diffusivity_multiplier_by_moisture[0][1] = "-1".into(),
            8 => modified.diffusivity_multiplier_by_moisture[0][0] = "0.03".into(),
            9 => modified.reference_diffusivity_m2_s = "0.000002".into(),
            10 => {
                modified.provenance = MaterialProvenance::SyntheticVerification {
                    description: " ".into(),
                }
            }
            11 => {
                modified.provenance = MaterialProvenance::PublicMeasurement {
                    source: "private://not-public".into(),
                    locator: "table 1".into(),
                }
            }
            12 => {
                modified.provenance = MaterialProvenance::PublicMeasurement {
                    source: "https://".into(),
                    locator: "table".into(),
                }
            }
            _ => {
                modified.provenance = MaterialProvenance::PublicMeasurement {
                    source: "https://user:password@example.org/data".into(),
                    locator: "table".into(),
                }
            }
        }
        let mut input = config.clone();
        input.material.as_mut().unwrap().reference.sha256 = material_digest(&modified).unwrap();
        assert!(
            CompiledPhysicsProof::compile_with_resources(input, None, Some(modified)).is_err(),
            "accepted mutation {change}"
        );
    }
    assert!(DryingMaterialDocument::from_yaml(&format!("{DOCUMENT}\nunknown: true\n")).is_err());
    assert!(DryingMaterialDocument::from_yaml(&format!("{DOCUMENT}\nid: duplicate\n")).is_err());
    let mut sourced = document;
    sourced.provenance = MaterialProvenance::PublicMeasurement {
        source: "https://example.org/measurement-data".into(),
        locator: "synthetic test of provenance syntax; no data are fetched".into(),
    };
    let mut input = config;
    input.material.as_mut().unwrap().reference.sha256 = material_digest(&sourced).unwrap();
    assert!(CompiledPhysicsProof::compile_with_resources(input, None, Some(sourced)).is_ok());
}

#[test]
fn material_solver_limits_and_wrong_equipment_cannot_be_silently_ignored() {
    let (config, document) = inputs();
    for change in 0..4 {
        let mut input = config.clone();
        let controls = input.material.as_mut().unwrap();
        match change {
            0 => controls.maximum_iterations = 0,
            1 => controls.relaxation = "0".into(),
            2 => controls.relaxation = "1.01".into(),
            _ => controls.residual_tolerance = "0.00000001".into(),
        }
        assert!(
            CompiledPhysicsProof::compile_with_resources(input, None, Some(document.clone()))
                .is_err()
        );
    }
    let mut input = config;
    input.case = PhysicsProofCase::IsothermalReceiver {
        temperature_k: "300".into(),
        gas_constant_j_kg_k: "287".into(),
        initial_pressure_pa: "200000".into(),
        receiver_pressure_pa: "100000".into(),
        conductance_kg_pa_s: "0.000001".into(),
    };
    assert!(
        CompiledPhysicsProof::compile_with_resources(input, None, Some(document))
            .unwrap_err()
            .contains("only supported")
    );
}
