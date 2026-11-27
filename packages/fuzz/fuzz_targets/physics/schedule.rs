use std::io::Cursor;
use std::sync::{Arc, OnceLock};

use hearthline_project::physics::schedule::{
    CompiledPhysicsSchedule, PhysicsInstanceIdentity, PhysicsInstanceRegistration,
};
use hearthline_sim::physics::schedule::PhysicsSchedule;

const BUDGET: u64 = 16 * 1024 * 1024;

pub fn exercise(data: &[u8]) {
    if matches!(data.first(), Some(b'J' | b'K')) {
        let registrations: Vec<_> = data
            .iter()
            .skip(1)
            .take(6)
            .enumerate()
            .map(|(index, &byte)| PhysicsInstanceRegistration {
                identity: PhysicsInstanceIdentity::new(
                    "factory",
                    "cell",
                    &format!("instance-{}", byte % 5),
                )
                .unwrap(),
                model: if data.first() == Some(&b'K') && index % 2 == 1 {
                    super::gas::cached_model(1).into()
                } else {
                    super::physics_model(if index % 2 == 0 { 0 } else { 2 }).into()
                },
            })
            .collect();
        let mut reverse = registrations.clone();
        reverse.reverse();
        let first = CompiledPhysicsSchedule::compile(registrations, BUDGET);
        let second = CompiledPhysicsSchedule::compile(reverse, BUDGET);
        assert_eq!(first.is_ok(), second.is_ok());
        if let (Ok(first), Ok(second)) = (first, second) {
            assert_eq!(first.digest(), second.digest());
            let mut first = PhysicsSchedule::new(Arc::new(first)).unwrap();
            let mut second = PhysicsSchedule::new(Arc::new(second)).unwrap();
            for _ in 0..32 {
                assert_eq!(first.next_event(), second.next_event());
                if first.complete() {
                    break;
                }
                let a = first.step_next();
                let b = second.step_next();
                assert_eq!(a.is_ok(), b.is_ok());
                assert_eq!(first.failure(), second.failure());
                if a.is_err() {
                    break;
                }
            }
            assert_eq!(
                first.state_digest().unwrap(),
                second.state_digest().unwrap()
            );
        }
        return;
    }
    let kind = match data.first() {
        Some(b'X' | b'Z') => 1,
        Some(b'Y') => 2,
        _ => 0,
    };
    let plan = plan(kind);
    if matches!(data.first(), Some(b'W' | b'Z')) {
        let _ = PhysicsSchedule::read_snapshot(plan, Cursor::new(&data[1..]));
        return;
    }
    let mut runtime = PhysicsSchedule::new(Arc::clone(&plan)).unwrap();
    for _ in 0..data.get(1).copied().unwrap_or(0) % 10 {
        if runtime.step_next().is_err() {
            break;
        }
    }
    let mut saved = Vec::new();
    runtime.write_snapshot(&mut saved).unwrap();
    let unchanged = data.len() < 5 || data[4] == 0;
    if !unchanged {
        let offset = (usize::from(data[2]) * 256 + usize::from(data[3])) % saved.len();
        saved[offset] ^= data[4];
    }
    let restored = PhysicsSchedule::read_snapshot(plan, Cursor::new(saved));
    if unchanged {
        let mut restored = restored.unwrap();
        assert_eq!(
            runtime.state_digest().unwrap(),
            restored.state_digest().unwrap()
        );
        assert_eq!(runtime.next_event(), restored.next_event());
        assert_eq!(runtime.failure(), restored.failure());
        assert_eq!(runtime.step_next().is_ok(), restored.step_next().is_ok());
        assert_eq!(
            runtime.state_digest().unwrap(),
            restored.state_digest().unwrap()
        );
    }
}

fn plan(kind: usize) -> Arc<CompiledPhysicsSchedule> {
    static PLAN: [OnceLock<Arc<CompiledPhysicsSchedule>>; 3] = [const { OnceLock::new() }; 3];
    Arc::clone(PLAN[kind].get_or_init(|| {
        let registrations = if kind == 0 {
            [("receiver-a", 0), ("receiver-b", 0), ("gas", 2)].map(|(id, model)| {
                PhysicsInstanceRegistration {
                    identity: PhysicsInstanceIdentity::new("factory", "cell", id).unwrap(),
                    model: super::physics_model(model).into(),
                }
            })
        } else {
            [
                ("a-air", super::gas::cached_model(1).into()),
                (
                    "b-air",
                    super::gas::cached_model(if kind == 2 { 2 } else { 1 }).into(),
                ),
                ("specimen", super::physics_model(1).into()),
            ]
            .map(|(id, model)| PhysicsInstanceRegistration {
                identity: PhysicsInstanceIdentity::new("factory", "cell", id).unwrap(),
                model,
            })
        };
        Arc::new(CompiledPhysicsSchedule::compile(Vec::from(registrations), BUDGET).unwrap())
    }))
}
