use std::sync::OnceLock;

use hearthline_config::physics::material::DryingMaterialDocument;
use hearthline_project::physics::CompiledPhysicsProof;
use hearthline_project::physics::material::material_digest;

pub fn exercise(input: &[u8]) {
    if let Ok(text) = std::str::from_utf8(input) {
        let _ = DryingMaterialDocument::from_yaml(text);
    }
    let kind = usize::from(input.first().copied().unwrap_or(0) % 3);
    let template = template(kind);
    let mut document = template.material.clone().unwrap();
    let table = if kind == 2 {
        document
            .liquid_heat_capacity_j_kg_k_by_temperature
            .as_mut()
            .unwrap()
    } else if kind == 1 {
        document
            .dry_heat_capacity_j_kg_k_by_temperature
            .as_mut()
            .unwrap()
    } else {
        &mut document.diffusivity_multiplier_by_moisture
    };
    for (index, bytes) in input.chunks_exact(8).take(table.len() * 2).enumerate() {
        let raw = i64::from_le_bytes(bytes.try_into().unwrap());
        let magnitude = raw.unsigned_abs();
        table[index / 2][index % 2] = format!(
            "{}{}.{:09}",
            if raw < 0 { "-" } else { "" },
            magnitude / 1_000_000_000,
            magnitude % 1_000_000_000,
        );
    }
    let mut config = template.config.clone();
    config.material.as_mut().unwrap().reference.sha256 = material_digest(&document).unwrap();
    if let Ok(model) = CompiledPhysicsProof::compile_with_resources(config, None, Some(document)) {
        assert_eq!(model.memory().field_buffer_bytes, 3456);
        assert_eq!(
            model.memory().immutable_material_bytes,
            [64, 136, 208][kind]
        );
        assert!(model.material().unwrap().multiplier().is_ok());
        assert_eq!(model.material().unwrap().caloric().is_some(), kind > 0);
        assert_eq!(
            model
                .material()
                .unwrap()
                .caloric()
                .is_some_and(|law| law.has_liquid()),
            kind == 2
        );
    }
}

fn template(index: usize) -> &'static hearthline_sim::physics::PhysicsReplay {
    static TEMPLATES: [OnceLock<hearthline_sim::physics::PhysicsReplay>; 3] =
        [const { OnceLock::new() }; 3];
    TEMPLATES[index].get_or_init(|| {
        serde_json::from_str(
            [
                include_str!("../../../../project/physics/replays/variable-drying.json"),
                include_str!("../../../../project/physics/replays/enthalpy-drying.json"),
                include_str!("../../../../project/physics/replays/wet-drying.json"),
            ][index],
        )
        .unwrap()
    })
}
