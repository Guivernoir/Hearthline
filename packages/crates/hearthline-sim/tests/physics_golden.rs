use hearthline_sim::physics::PhysicsReplay;

#[test]
fn committed_physics_fields_recompute_exactly_on_a_constrained_stack() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            for source in [
                include_str!("../../../../project/physics/replays/receiver-equilibration.json"),
                include_str!("../../../../project/physics/replays/drying-specimen.json"),
                include_str!("../../../../project/physics/replays/drying-deformation.json"),
                include_str!("../../../../project/physics/replays/compressible-receiver.json"),
                include_str!("../../../../project/physics/replays/basin-drying.json"),
                include_str!("../../../../project/physics/replays/variable-drying.json"),
                include_str!("../../../../project/physics/replays/enthalpy-drying.json"),
                include_str!("../../../../project/physics/replays/wet-drying.json"),
                include_str!("../../../../project/physics/replays/wet-elastic-zero.json"),
            ] {
                let replay: PhysicsReplay = serde_json::from_str(source).unwrap();
                if replay.config.id == "wet-elastic-zero" {
                    let config: hearthline_config::physics::PhysicsProofConfig = serde_yaml_ng::from_str(include_str!("../../../../project/physics/proofs/wet-elastic-zero.yaml")).unwrap();
                    assert_eq!(replay.config, config);
                    assert_eq!(replay.final_state.digest, "8c581d4232237a2016d599ec3c3feb3d8f4f5c34973b00b1ec3c64346ed472b6");
                    assert!(replay.final_state.mechanics.as_ref().unwrap().iterations > 0);
                }
                if replay.config.id == "wet-drying" {
                    let config: hearthline_config::physics::PhysicsProofConfig = serde_yaml_ng::from_str(include_str!("../../../../project/physics/proofs/wet-drying.yaml")).unwrap();
                    let material = hearthline_config::physics::material::DryingMaterialDocument::from_yaml(include_str!("../../../../project/physics/materials/synthetic-wet-drying.yaml")).unwrap();
                    assert_eq!(replay.config, config);
                    assert_eq!(replay.material.as_ref(), Some(&material));
                    assert_eq!(replay.final_state.digest, "e1d4685d0586e30f4a36867041bd4b5d04922c041ada1bd2c63af77e00cf9a07");
                }
                if replay.config.id == "enthalpy-drying" {
                    let config: hearthline_config::physics::PhysicsProofConfig = serde_yaml_ng::from_str(include_str!("../../../../project/physics/proofs/enthalpy-drying.yaml")).unwrap();
                    let material = hearthline_config::physics::material::DryingMaterialDocument::from_yaml(include_str!("../../../../project/physics/materials/synthetic-drying-enthalpy.yaml")).unwrap();
                    assert_eq!(replay.config, config);
                    assert_eq!(replay.material.as_ref(), Some(&material));
                    assert_eq!(replay.final_state.digest, "b07cde3bd5592659d9af47bd7198e31e042137b1908bb81415055fb0e8463e6b");
                }
                if replay.config.id == "variable-drying" {
                    let config: hearthline_config::physics::PhysicsProofConfig = serde_yaml_ng::from_str(include_str!("../../../../project/physics/proofs/variable-drying.yaml")).unwrap();
                    let material = hearthline_config::physics::material::DryingMaterialDocument::from_yaml(include_str!("../../../../project/physics/materials/synthetic-moisture-diffusion.yaml")).unwrap();
                    assert_eq!(replay.config, config);
                    assert_eq!(replay.material.as_ref(), Some(&material));
                    assert_eq!(replay.final_state.digest, "8394f8def96e5faabbb99c1bd683937a3653a69068dfc9237682f3f7f689f25a");
                }
                if replay.config.id == "basin-drying" {
                    let source: hearthline_config::physics::PhysicsProofConfig =
                        serde_yaml_ng::from_str(include_str!(
                            "../../../../project/physics/proofs/basin-drying.yaml"
                        ))
                        .unwrap();
                    assert_eq!(
                        serde_json::to_value(&replay.config).unwrap(),
                        serde_json::to_value(source).unwrap()
                    );
                    let mechanics = replay.final_state.mechanics.as_ref().unwrap();
                    assert!(mechanics.iterations > 0);
                    assert!(mechanics.displacement_raw.iter().any(|value| *value != 0));
                    assert!(mechanics.residual_raw <= 10_000);
                }
                replay.verify().unwrap();
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
