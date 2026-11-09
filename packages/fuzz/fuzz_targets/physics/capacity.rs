use std::sync::OnceLock;

use hearthline_config::physics::capacity::{
    CapacityKernel, CapacityModelReference, PhysicsCapacityConfig,
};
use hearthline_project::physics::capacity::{
    CapacitySources, PhysicsCapacityReport, proof_source_digest,
};
use hearthline_sim::physics::PhysicsReplay;

pub fn exercise(input: &[u8]) {
    if let Ok(text) = std::str::from_utf8(input) {
        let _ = PhysicsCapacityConfig::from_yaml(text);
    }
    let (template, replay) = template();
    let mut config = template.clone();
    let mut sources = CapacitySources {
        proofs: vec![replay.config.clone()],
        geometries: Vec::new(),
        materials: vec![replay.material.clone().unwrap()],
    };
    let profile = config
        .profiles
        .iter_mut()
        .find(|p| p.id == "product-block")
        .unwrap();
    profile.shape = replay.config.shape;
    profile.kernel = CapacityKernel::ScalarTransport;
    profile.model = Some(CapacityModelReference {
        id: replay.config.id.clone(),
        sha256: proof_source_digest(&replay.config).unwrap(),
    });
    let mode = input.first().copied().unwrap_or(0) % 8;
    match mode {
        1 => profile.model.as_mut().unwrap().sha256 = "0".repeat(64),
        2 => sources.proofs.push(sources.proofs[0].clone()),
        3 => sources.materials.clear(),
        4 => profile.shape[0] += 1,
        5 => {
            sources.proofs[0].timestep_s = "-1".into();
            profile.model.as_mut().unwrap().sha256 =
                proof_source_digest(&sources.proofs[0]).unwrap();
        }
        6 => {
            let mut unused = sources.materials[0].clone();
            unused.id = "unused".into();
            sources.materials.push(unused);
        }
        7 => {
            if let Some(bytes) = input.get(1..9) {
                config.populations[0].capacity = u64::from_le_bytes(bytes.try_into().unwrap());
            }
        }
        _ => {}
    }
    let result = PhysicsCapacityReport::compile_with_sources(config, sources);
    if (1..=6).contains(&mode) {
        assert!(result.is_err());
    } else if let Ok(report) = result {
        let profile = report
            .profiles
            .iter()
            .find(|p| p.profile.model.is_some())
            .unwrap();
        assert_eq!(profile.layout.cells, 27);
        assert_eq!(profile.layout.immutable_material_bytes, 208);
        assert_eq!(profile.layout.instance_buffer_bytes().unwrap(), 3456);
        assert!(profile.compiled_model_digest.is_some());
        assert_eq!(
            report.total_planned_bytes,
            report.runtime_planned_bytes.max(report.compiler_peak_bytes)
        );
        assert_eq!(
            report.within_budget,
            report.total_planned_bytes <= report.reviewed_budget_bytes
        );
        assert_eq!(report.immutable_material_bytes, 208);
    } else {
        assert_eq!(mode, 7);
    }
}

fn template() -> &'static (PhysicsCapacityConfig, PhysicsReplay) {
    static TEMPLATE: OnceLock<(PhysicsCapacityConfig, PhysicsReplay)> = OnceLock::new();
    TEMPLATE.get_or_init(|| {
        (
            PhysicsCapacityConfig::from_yaml(include_str!(
                "../../../../project/physics/capacity/factory-baseline.yaml"
            ))
            .unwrap(),
            serde_json::from_str(include_str!(
                "../../../../project/physics/replays/wet-drying.json"
            ))
            .unwrap(),
        )
    })
}
