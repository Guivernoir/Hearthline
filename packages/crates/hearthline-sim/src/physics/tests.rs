use super::*;

const SOURCE: &str = include_str!("../../../../../project/physics/proofs/drying-deformation.yaml");

#[test]
fn crafted_mechanical_checkpoints_are_validated_before_any_state_is_restored() {
    let input: PhysicsProofConfig = serde_yaml_ng::from_str(SOURCE).unwrap();
    let model = Arc::new(CompiledPhysicsProof::compile(input).unwrap());
    let mut session = PhysicsProofSession::new(model.clone()).unwrap();
    session.step().unwrap();
    let baseline = session.checkpoint();
    for change in 0..7 {
        let mut checkpoint = baseline.clone();
        let mechanics = checkpoint.mechanics.as_mut().unwrap();
        match change {
            0 => mechanics.displacement_raw[0] = 1,
            1 => mechanics.stress_raw[0] += 1,
            2 => {
                mechanics.stress_raw.pop();
            }
            3 => mechanics.residual_raw = -1,
            4 => mechanics.iterations = u32::MAX,
            5 => mechanics.displacement_raw[3] = i64::MAX,
            _ => checkpoint.mechanics = None,
        }
        checkpoint.digest = checkpoint_digest(&checkpoint, model.digest());
        assert!(
            session.restore(&checkpoint).is_err(),
            "accepted alteration {change}"
        );
        assert_eq!(session.checkpoint(), baseline);
    }
}

#[test]
fn specimens_share_geometry_without_sharing_mutable_material_fields() {
    let input: PhysicsProofConfig = serde_yaml_ng::from_str(SOURCE).unwrap();
    let model = Arc::new(CompiledPhysicsProof::compile(input).unwrap());
    let mut first = PhysicsProofSession::new(model.clone()).unwrap();
    let mut second = PhysicsProofSession::new(model).unwrap();
    let untouched = second.checkpoint();
    first.step().unwrap();
    assert_eq!(second.checkpoint(), untouched);
    assert_ne!(first.checkpoint().mechanics, untouched.mechanics);
    second.step().unwrap();
    assert_eq!(first.checkpoint(), second.checkpoint());
}

#[test]
fn crafted_flow_checkpoints_cannot_change_conserved_fields_or_retry_state_partially() {
    let input: PhysicsProofConfig = serde_yaml_ng::from_str(include_str!(
        "../../../../../project/physics/proofs/compressible-receiver.yaml"
    ))
    .unwrap();
    let model = Arc::new(CompiledPhysicsProof::compile(input).unwrap());
    let mut session = PhysicsProofSession::new(model.clone()).unwrap();
    session.step().unwrap();
    let baseline = session.checkpoint();
    for change in 0..8 {
        let mut checkpoint = baseline.clone();
        let flow = checkpoint.flow.as_mut().unwrap();
        match change {
            0 => flow.conserved_raw[0][0] = -1,
            1 => {
                flow.conserved_raw.pop();
            }
            2 => flow.last_step = None,
            3 => flow.last_step.as_mut().unwrap().substeps = 3,
            4 => flow.last_step.as_mut().unwrap().attempts = 9,
            5 => flow.last_step.as_mut().unwrap().solver_time_us += 1,
            6 => flow.balance_error_raw[0] += 1,
            _ => checkpoint.first_field_raw[0] += 1,
        }
        checkpoint.digest = checkpoint_digest(&checkpoint, model.digest());
        assert!(
            session.restore(&checkpoint).is_err(),
            "accepted alteration {change}"
        );
        assert_eq!(session.checkpoint(), baseline);
    }
}

#[test]
fn recording_rejects_oversized_spatial_artifacts_before_allocating_meshes_or_fields() {
    let mut input: PhysicsProofConfig = serde_yaml_ng::from_str(SOURCE).unwrap();
    input.shape = [1000; 3];
    let mut result = None;
    let measured = allocation_counter::measure(|| {
        result = Some(PhysicsReplay::record(input));
    });
    assert!(result.unwrap().unwrap_err().contains("document limit"));
    assert!(measured.bytes_total < 32 * 1024);
    let mut input: PhysicsProofConfig = serde_yaml_ng::from_str(SOURCE).unwrap();
    input.shape = [u32::MAX; 3];
    assert!(
        PhysicsReplay::record(input)
            .unwrap_err()
            .contains("overflow")
    );
}

#[test]
fn crafted_material_checkpoints_cannot_bypass_domain_or_solver_limits() {
    let config = serde_yaml_ng::from_str(include_str!(
        "../../../../../project/physics/proofs/variable-drying.yaml"
    ))
    .unwrap();
    let document = DryingMaterialDocument::from_yaml(include_str!(
        "../../../../../project/physics/materials/synthetic-moisture-diffusion.yaml"
    ))
    .unwrap();
    let model = Arc::new(
        CompiledPhysicsProof::compile_with_resources(config, None, Some(document)).unwrap(),
    );
    let mut session = PhysicsProofSession::new(model.clone()).unwrap();
    session.step().unwrap();
    let baseline = session.checkpoint();
    for change in 0..9 {
        let mut checkpoint = baseline.clone();
        match change {
            0 => checkpoint.first_field_raw[0] = 400_000_001,
            1 => checkpoint.temperature_k_raw[0] = 249_999_999_999,
            2 => checkpoint.diffusion = None,
            3 => checkpoint.diffusion.as_mut().unwrap().iterations = 0,
            4 => checkpoint.diffusion.as_mut().unwrap().iterations = 101,
            5 => checkpoint.diffusion.as_mut().unwrap().linear_sweeps = 0,
            6 => checkpoint.diffusion.as_mut().unwrap().linear_sweeps = u64::MAX,
            7 => checkpoint.diffusion.as_mut().unwrap().residual_raw = -1,
            _ => checkpoint.diffusion.as_mut().unwrap().residual_raw = 101,
        }
        checkpoint.digest = checkpoint_digest(&checkpoint, model.digest());
        assert!(
            session.restore(&checkpoint).is_err(),
            "accepted material mutation {change}"
        );
        assert_eq!(session.checkpoint(), baseline);
    }
}

#[test]
fn crafted_caloric_reports_are_rejected_before_publishing_any_fields() {
    let config = serde_yaml_ng::from_str(include_str!(
        "../../../../../project/physics/proofs/enthalpy-drying.yaml"
    ))
    .unwrap();
    let material = DryingMaterialDocument::from_yaml(include_str!(
        "../../../../../project/physics/materials/synthetic-drying-enthalpy.yaml"
    ))
    .unwrap();
    let model = Arc::new(
        CompiledPhysicsProof::compile_with_resources(config, None, Some(material)).unwrap(),
    );
    let mut session = PhysicsProofSession::new(model.clone()).unwrap();
    session.step().unwrap();
    let baseline = session.checkpoint();
    for change in 0..7 {
        let mut checkpoint = baseline.clone();
        let thermal = checkpoint
            .diffusion
            .as_mut()
            .unwrap()
            .thermal
            .as_mut()
            .unwrap();
        match change {
            0 => thermal.iterations = 0,
            1 => thermal.iterations = 401,
            2 => thermal.local_iterations = 0,
            3 => thermal.local_iterations = u64::MAX,
            4 => thermal.residual_raw = -1,
            5 => thermal.residual_raw = 101,
            _ => checkpoint.diffusion.as_mut().unwrap().thermal = None,
        }
        checkpoint.digest = checkpoint_digest(&checkpoint, model.digest());
        assert!(
            session.restore(&checkpoint).is_err(),
            "accepted caloric mutation {change}"
        );
        assert_eq!(session.checkpoint(), baseline);
    }
}
