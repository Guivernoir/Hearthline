use std::io::Cursor;
use std::sync::{Arc, OnceLock};

use hearthline_model::ComponentId;
use hearthline_physics::Scalar;
use hearthline_physics::flow::FlowBoundary;
use hearthline_physics::transport::SolverLimits;
use hearthline_project::physics::flow::connected::{
    CompiledConnectedGas, ConnectedGasProfile, GasReference, GasRegion, GasRetryConfig,
    RegionInitialGas,
};
use hearthline_project::physics::flow::mesh::{FlowPatch, FlowPatchConnection};
use hearthline_project::physics::schedule::PhysicsInstanceIdentity;
use hearthline_sim::physics::flow::connected::ConnectedGasSession;

fn source() -> (
    Vec<GasRegion>,
    Vec<FlowPatchConnection>,
    Vec<RegionInitialGas>,
    ConnectedGasProfile,
) {
    let identities = ["supply", "receiver"]
        .map(|name| PhysicsInstanceIdentity::new("site", "air", name).unwrap());
    let regions = identities
        .iter()
        .map(|identity| GasRegion {
            identity: identity.clone(),
            shape: [2, 1, 1],
            spacing_m: [Scalar::ONE; 3],
            boundaries: [FlowBoundary::AdiabaticSlipWall; 6],
        })
        .collect();
    let initial = identities
        .iter()
        .enumerate()
        .map(|(index, identity)| RegionInitialGas {
            identity: identity.clone(),
            density_kg_m3: Scalar::ONE,
            pressure_pa: Scalar::integer(index as i64 + 1).unwrap(),
            velocity_m_s: [Scalar::ZERO; 3],
        })
        .collect();
    let connections = vec![FlowPatchConnection {
        id: ComponentId::new("feed").unwrap(),
        first: FlowPatch {
            region: identities[0].clone(),
            face: 1,
        },
        second: FlowPatch {
            region: identities[1].clone(),
            face: 0,
        },
    }];
    let profile = ConnectedGasProfile {
        reference: GasReference {
            length_m: Scalar::ONE,
            velocity_m_s: Scalar::ONE,
            density_kg_m3: Scalar::ONE,
            gas_constant_j_kg_k: Scalar::ONE,
        },
        heat_capacity_ratio: Scalar::ratio(14, 10).unwrap(),
        dynamic_viscosity_pa_s: Scalar::ZERO,
        thermal_conductivity_w_m_k: Scalar::ZERO,
        timestep_us: 1000,
        steps: 2,
        limits: SolverLimits {
            iterations: 32,
            residual: Scalar::from_raw(100),
        },
    };
    (regions, connections, initial, profile)
}

pub fn exercise_compile(input: &[u8]) {
    let Some(bytes) = input.get(..10) else {
        return;
    };
    let (mut regions, mut connections, mut initial, mut profile) = source();
    let value = Scalar::from_raw(i64::from_le_bytes(bytes[2..10].try_into().unwrap()));
    match bytes[0] % 12 {
        0 => profile.reference.length_m = value,
        1 => profile.reference.velocity_m_s = value,
        2 => initial[0].density_kg_m3 = value,
        3 => initial[0].pressure_pa = value,
        4 => profile.dynamic_viscosity_pa_s = value,
        5 => profile.thermal_conductivity_w_m_k = value,
        6 => regions[0].spacing_m[0] = value,
        7 => connections[0].first.face = bytes[1],
        8 => regions[0].shape[0] = usize::from(bytes[1] % 4),
        9 => profile.heat_capacity_ratio = value,
        10 => profile.timestep_us = u64::from_le_bytes(bytes[2..10].try_into().unwrap()),
        _ => {
            regions[0].boundaries[0] = FlowBoundary::NoSlipWall {
                velocity: [Scalar::ZERO; 3],
                temperature: Some(value),
            }
        }
    }
    let compiled = if bytes[1] & 1 == 0 {
        CompiledConnectedGas::compile(regions, connections, initial, profile, 128 * 1024)
    } else {
        CompiledConnectedGas::compile_with_retries(
            regions,
            connections,
            initial,
            profile,
            128 * 1024,
            GasRetryConfig {
                maximum_substeps: 1 << (bytes[1] % 5),
                minimum_timestep_us: u64::from(bytes[1] % 3),
            },
        )
    };
    let Ok(model) = compiled else {
        return;
    };
    let model = Arc::new(model);
    let Ok(mut session) = ConnectedGasSession::new(Arc::clone(&model)) else {
        return;
    };
    let before = session.fields().to_vec();
    if session.step_next().is_err() {
        assert_eq!(session.fields(), before);
        assert_eq!(session.time_us(), 0);
    }
    let mut snapshot = Vec::new();
    session.write_snapshot(&mut snapshot).unwrap();
    let restored = ConnectedGasSession::read_snapshot(model, Cursor::new(snapshot)).unwrap();
    assert_eq!(
        restored.state_digest().unwrap(),
        session.state_digest().unwrap()
    );
}

pub fn exercise_snapshot(input: &[u8]) {
    let index = match input.first() {
        Some(b'E') => 1,
        Some(b'F') => 2,
        _ => 0,
    };
    let model = cached_model(index);
    if input.first() == Some(&b'C') {
        let _ = ConnectedGasSession::read_snapshot(model, Cursor::new(&input[1..]));
        return;
    }
    let mut session = ConnectedGasSession::new(Arc::clone(&model)).unwrap();
    for _ in 0..input.get(1).copied().unwrap_or(0) % 3 {
        if session.step_next().is_err() {
            break;
        }
    }
    let mut snapshot = Vec::new();
    session.write_snapshot(&mut snapshot).unwrap();
    if input.len() > 3 {
        let index = (usize::from(input[1]) * 256 + usize::from(input[2])) % snapshot.len();
        snapshot[index] ^= input[3];
    }
    let restored = ConnectedGasSession::read_snapshot(model, Cursor::new(snapshot));
    if input.len() <= 3 || input[3] == 0 {
        assert_eq!(
            restored.unwrap().state_digest().unwrap(),
            session.state_digest().unwrap()
        );
    }
}

pub(super) fn cached_model(index: usize) -> Arc<CompiledConnectedGas> {
    static MODELS: [OnceLock<Arc<CompiledConnectedGas>>; 3] = [const { OnceLock::new() }; 3];
    MODELS[index]
        .get_or_init(|| {
            let (regions, connections, initial, mut profile) = source();
            let compiled = if index == 0 {
                CompiledConnectedGas::compile(regions, connections, initial, profile, 128 * 1024)
            } else {
                profile.timestep_us = 1001;
                profile.limits.iterations = if index == 1 { 14 } else { 1 };
                CompiledConnectedGas::compile_with_retries(
                    regions,
                    connections,
                    initial,
                    profile,
                    128 * 1024,
                    GasRetryConfig {
                        maximum_substeps: 64,
                        minimum_timestep_us: 1,
                    },
                )
            };
            Arc::new(compiled.unwrap())
        })
        .clone()
}

pub fn exercise_population(input: &[u8]) {
    use hearthline_sim::physics::flow::connected::population::ConnectedGasPopulation;
    let model = cached_model(if input.first() == Some(&b'N') { 2 } else { 1 });
    if input.first() == Some(&b'L') {
        let _ = ConnectedGasPopulation::read_snapshot(model, Cursor::new(&input[1..]));
        return;
    }
    let count = usize::from(input.get(1).copied().unwrap_or(0) % 3 + 1);
    let mut pool = ConnectedGasPopulation::new(Arc::clone(&model), count).unwrap();
    let mut direct: Vec<_> = (0..count)
        .map(|_| ConnectedGasSession::new(Arc::clone(&model)).unwrap())
        .collect();
    for byte in input.iter().skip(2).take(5) {
        let slot = usize::from(*byte) % count;
        assert_eq!(pool.step(slot).is_ok(), direct[slot].step_next().is_ok());
        for (slot, state) in direct.iter().enumerate() {
            assert_eq!(pool.fields(slot).unwrap(), state.fields());
            assert_eq!(
                pool.instance_digest(slot).unwrap(),
                state.state_digest().unwrap()
            );
        }
    }
    let mut snapshot = Vec::new();
    pool.write_snapshot(&mut snapshot).unwrap();
    if let Some(mutation) = input.get(7..10) {
        let index = (usize::from(mutation[0]) * 256 + usize::from(mutation[1])) % snapshot.len();
        snapshot[index] ^= mutation[2];
    }
    let restored = ConnectedGasPopulation::read_snapshot(model, Cursor::new(snapshot));
    if input.len() < 10 || input[9] == 0 {
        assert_eq!(
            restored.unwrap().state_digest().unwrap(),
            pool.state_digest().unwrap()
        );
    }
}
