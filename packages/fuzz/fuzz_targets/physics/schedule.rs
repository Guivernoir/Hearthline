use std::io::Cursor;
use std::sync::{Arc, OnceLock};

use hearthline_project::physics::schedule::{
    CompiledPhysicsSchedule, PhysicsInstanceIdentity, PhysicsInstanceRegistration,
};
use hearthline_sim::physics::schedule::PhysicsSchedule;

const BUDGET: u64 = 16 * 1024 * 1024;

pub fn exercise(data: &[u8]) {
    if data.first() == Some(&b'J') {
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
                model: super::physics_model(if index % 2 == 0 { 0 } else { 2 }),
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
                first.step_next().unwrap();
                second.step_next().unwrap();
            }
            assert_eq!(
                first.state_digest().unwrap(),
                second.state_digest().unwrap()
            );
        }
        return;
    }
    let plan = plan();
    if data.first() == Some(&b'W') {
        let _ = PhysicsSchedule::read_snapshot(plan, Cursor::new(&data[1..]));
        return;
    }
    let mut runtime = PhysicsSchedule::new(Arc::clone(&plan)).unwrap();
    for _ in 0..data.get(1).copied().unwrap_or(0) % 10 {
        runtime.step_next().unwrap();
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
        runtime.step_next().unwrap();
        restored.step_next().unwrap();
        assert_eq!(
            runtime.state_digest().unwrap(),
            restored.state_digest().unwrap()
        );
    }
}

fn plan() -> Arc<CompiledPhysicsSchedule> {
    static PLAN: OnceLock<Arc<CompiledPhysicsSchedule>> = OnceLock::new();
    Arc::clone(PLAN.get_or_init(|| {
        let registrations =
            [("receiver-a", 0), ("receiver-b", 0), ("gas", 2)].map(|(id, model)| {
                PhysicsInstanceRegistration {
                    identity: PhysicsInstanceIdentity::new("factory", "cell", id).unwrap(),
                    model: super::physics_model(model),
                }
            });
        Arc::new(CompiledPhysicsSchedule::compile(Vec::from(registrations), BUDGET).unwrap())
    }))
}
