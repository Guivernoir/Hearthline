use std::sync::Arc;

use hearthline_config::physics::{MechanicsInitialGuess, PhysicsProofCase, PhysicsProofConfig};
use hearthline_project::physics::CompiledPhysicsProof;

use crate::physics::population::PhysicsProofPopulation;
use crate::physics::{PhysicsCheckpoint, PhysicsProofSession, checkpoint_digest};

fn model() -> Arc<CompiledPhysicsProof> {
    let mut config: PhysicsProofConfig = serde_yaml_ng::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../project/physics/proofs/drying-deformation.yaml"
    )))
    .unwrap();
    let PhysicsProofCase::DryingSpecimen { mechanics, .. } = &mut config.case else {
        unreachable!()
    };
    mechanics.as_mut().unwrap().initial_guess = MechanicsInitialGuess::Zero;
    Arc::new(CompiledPhysicsProof::compile(config).unwrap())
}

#[test]
fn correctly_hashed_but_noncanonical_projections_are_rejected_atomically() {
    let model = model();
    let mut session = PhysicsProofSession::new(model.clone()).unwrap();
    let initial = session.checkpoint();
    session.step().unwrap();
    session.step().unwrap();
    let current = session.checkpoint();
    let mut bank = PhysicsProofPopulation::new(model.clone(), 2).unwrap();
    bank.restore(0, &current).unwrap();
    for mutation in 0..7 {
        let mut candidate = if mutation >= 4 {
            initial.clone()
        } else {
            current.clone()
        };
        let mechanics = candidate.mechanics.as_mut().unwrap();
        match mutation {
            0 => mechanics.displacement_raw[3] += 1,
            1 => mechanics.stress_raw[0] += 1,
            2 | 4 => mechanics.iterations += 1,
            3 => mechanics.residual_raw = if mechanics.residual_raw == 0 { 1 } else { 0 },
            5 => candidate.temperature_k_raw[0] += 1,
            6 => candidate.first_field_raw[0] += 1,
            _ => unreachable!(),
        }
        candidate.digest = checkpoint_digest(&candidate, model.digest());
        let error = session.restore(&candidate).unwrap_err();
        assert!(
            error.contains("reconstruction") || error.contains("initial fields"),
            "{error}"
        );
        assert_eq!(session.checkpoint(), current);
        assert!(bank.restore(0, &candidate).is_err());
        assert_eq!(bank.checkpoint(0).unwrap(), current);
        assert_eq!(bank.checkpoint(1).unwrap(), initial);
    }
    // A valid restore remains usable after failed reconstruction trials.
    session.restore(&initial).unwrap();
    session.step().unwrap();
    bank.restore(1, &initial).unwrap();
    bank.step(1).unwrap();
    assert_eq!(bank.checkpoint(1).unwrap(), session.checkpoint());
}

#[test]
fn previous_start_equilibrium_cannot_be_imported_as_zero_start_history() {
    let zero = model();
    let mut config = zero.config().clone();
    let PhysicsProofCase::DryingSpecimen { mechanics, .. } = &mut config.case else {
        unreachable!()
    };
    mechanics.as_mut().unwrap().initial_guess = MechanicsInitialGuess::PreviousStep;
    let previous = Arc::new(CompiledPhysicsProof::compile(config).unwrap());
    let mut previous = PhysicsProofSession::new(previous).unwrap();
    let mut session = PhysicsProofSession::new(zero.clone()).unwrap();
    for _ in 0..4 {
        previous.step().unwrap();
        session.step().unwrap();
    }
    let before = session.checkpoint();
    let mut candidate: PhysicsCheckpoint = previous.checkpoint();
    assert_ne!(before.mechanics, candidate.mechanics);
    candidate.digest = checkpoint_digest(&candidate, zero.digest());
    assert!(
        session
            .restore(&candidate)
            .unwrap_err()
            .contains("reconstruction")
    );
    assert_eq!(session.checkpoint(), before);
}
