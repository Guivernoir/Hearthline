#![no_main]

use hearthline_project::physics::CompiledPhysicsProof;
use hearthline_sim::ReplayArtifact;
use hearthline_sim::physics::population::PhysicsProofPopulation;
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;
use std::sync::{Arc, OnceLock};

#[path = "physics/gas.rs"]
#[allow(dead_code)]
mod gas;
#[path = "physics/schedule.rs"]
mod schedule;

fuzz_target!(|data: &[u8]| {
    let bounded = &data[..data.len().min(256 * 1_024)];
    if matches!(bounded.first(), Some(b'L' | b'M' | b'N')) {
        gas::exercise_population(bounded);
        return;
    }
    if matches!(bounded.first(), Some(b'C' | b'D' | b'E' | b'F')) {
        gas::exercise_snapshot(bounded);
        return;
    }
    if matches!(
        bounded.first(),
        Some(b'H' | b'J' | b'W' | b'X' | b'Y' | b'Z' | b'K')
    ) {
        schedule::exercise(bounded);
        return;
    }
    if let Some(kind @ (b'P' | b'Q' | b'R' | b'S' | b'T' | b'U' | b'V')) = bounded.first().copied()
    {
        let index = usize::from(kind - b'P');
        let _ =
            PhysicsProofPopulation::read_snapshot(physics_model(index), Cursor::new(&bounded[1..]));
        return;
    }
    if bounded.first() == Some(&b'G') {
        let index = match bounded.get(1) {
            Some(b'm') => 1,
            Some(b'f') => 2,
            Some(b'd') => 3,
            Some(b'e') => 4,
            Some(b'w') => 5,
            Some(b'z') => 6,
            _ => 0,
        };
        let model = physics_model(index);
        let mut population = PhysicsProofPopulation::new(model.clone(), 2).unwrap();
        for _ in 0..bounded.get(2).copied().unwrap_or(0) % 3 {
            population.step(1).unwrap();
        }
        let mut bytes = Vec::new();
        population.write_snapshot(&mut bytes).unwrap();
        if bounded.len() > 3 {
            let index = (usize::from(bounded[1]) * 256 + usize::from(bounded[2])) % bytes.len();
            bytes[index] ^= bounded[3];
        }
        let restored = PhysicsProofPopulation::read_snapshot(model, Cursor::new(bytes));
        if bounded.len() <= 3 || bounded[3] == 0 {
            let mut restored = restored.unwrap();
            for instance in 0..2 {
                assert_eq!(
                    population.checkpoint(instance).unwrap(),
                    restored.checkpoint(instance).unwrap()
                );
            }
        }
        return;
    }
    if let Ok(artifact) = serde_json::from_slice::<ReplayArtifact>(bounded) {
        let _ = artifact.normalize();
    }
});

fn physics_model(index: usize) -> Arc<CompiledPhysicsProof> {
    static MODELS: [OnceLock<Arc<CompiledPhysicsProof>>; 7] = [const { OnceLock::new() }; 7];
    let source = [
        include_str!("../../../project/physics/replays/receiver-equilibration.json"),
        include_str!("../../../project/physics/replays/drying-deformation.json"),
        include_str!("../../../project/physics/replays/compressible-receiver.json"),
        include_str!("../../../project/physics/replays/variable-drying.json"),
        include_str!("../../../project/physics/replays/enthalpy-drying.json"),
        include_str!("../../../project/physics/replays/wet-drying.json"),
        include_str!("../../../project/physics/replays/wet-elastic-zero.json"),
    ][index];
    MODELS[index]
        .get_or_init(|| {
            let replay =
                serde_json::from_str::<hearthline_sim::physics::PhysicsReplay>(source).unwrap();
            let mut config = replay.config;
            config.allocation_budget_bytes = 1024 * 1024;
            if index == 1 {
                config.shape = [2; 3];
            }
            Arc::new(
                CompiledPhysicsProof::compile_with_resources(
                    config,
                    replay.geometry,
                    replay.material,
                )
                .unwrap(),
            )
        })
        .clone()
}
