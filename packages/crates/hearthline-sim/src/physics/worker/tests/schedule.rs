use std::io::Cursor;

use hearthline_project::physics::schedule::{PhysicsInstanceIdentity, PhysicsInstanceRegistration};

use super::*;
use crate::physics::clock::PhysicsPacer;

fn plan(fails: bool) -> Arc<CompiledPhysicsSchedule> {
    let fast = receiver(6);
    let mut slow = fast.config().clone();
    slow.id = "compressible-slow".into();
    slow.timestep_s = "0.0002".into();
    slow.steps = 3;
    if fails {
        slow.maximum_iterations = 1;
        slow.residual_tolerance = "0.000000001".into();
        let hearthline_config::physics::PhysicsProofCase::CompressibleReceiver { flow } =
            &mut slow.case
        else {
            unreachable!()
        };
        flow.maximum_substeps = 1;
    }
    let registrations = [
        ("receiver-a", Arc::clone(&fast)),
        ("receiver-b", fast),
        (
            "receiver-slow",
            Arc::new(CompiledPhysicsProof::compile(slow).unwrap()),
        ),
    ]
    .map(|(id, model)| PhysicsInstanceRegistration {
        identity: PhysicsInstanceIdentity::new("factory", "air", id).unwrap(),
        model,
    });
    Arc::new(CompiledPhysicsSchedule::compile(Vec::from(registrations), 16 * 1024 * 1024).unwrap())
}

fn finish(runtime: &mut PhysicsSchedule) {
    while !runtime.complete() {
        runtime.step_next().unwrap();
    }
}

#[test]
fn multirate_worker_preserves_every_event_and_field_at_every_pacing_mode() {
    let plan = plan(false);
    let mut expected = PhysicsSchedule::new(Arc::clone(&plan)).unwrap();
    finish(&mut expected);
    let expected = expected.state_digest().unwrap();
    for mode in [
        ClockCommand::Headless,
        ClockCommand::SetRate(PlaybackRate::One),
        ClockCommand::SetRate(PlaybackRate::Two),
        ClockCommand::SetRate(PlaybackRate::Five),
        ClockCommand::SetRate(PlaybackRate::Ten),
    ] {
        let mut runner = Runner::new(PhysicsSchedule::new(Arc::clone(&plan)).unwrap()).unwrap();
        let mut independent: Vec<_> = plan
            .instances()
            .iter()
            .map(|binding| {
                PhysicsProofSession::new(Arc::clone(&plan.profiles()[binding.profile])).unwrap()
            })
            .collect();
        assert!(runner.projection().fields.is_none());
        runner
            .cycle(0, Some(request(0, mode)))
            .unwrap()
            .result
            .unwrap();
        runner
            .cycle(0, Some(request(1, ClockCommand::Resume)))
            .unwrap()
            .result
            .unwrap();
        if let Some(fields) = runner.projection().fields {
            let instance = fields.instance.unwrap();
            independent[instance].step().unwrap();
            assert_eq!(fields.values, independent[instance].field_ranges());
        }
        let allocation = allocation_counter::measure(|| {
            while !runner.projection().complete {
                let next = runner.session.next_event().unwrap();
                runner.cycle(1000, None);
                let status = runner.projection();
                let fields = status.fields.unwrap();
                independent[next.instance].step().unwrap();
                assert_eq!(fields.values, independent[next.instance].field_ranges());
                assert_eq!(fields.instance, Some(next.instance));
                assert_eq!(fields.time_us, next.time_us);
                assert_eq!(status.time_us, runner.session.time_us());
                let binding = &plan.instances()[next.instance];
                assert_eq!(
                    fields.profile_digest.as_slice(),
                    plan.profiles()[binding.profile].digest().as_bytes()
                );
                assert_eq!(
                    fields.instance_step,
                    runner.session.completed_steps(next.instance).unwrap()
                );
            }
        });
        assert_eq!(allocation.count_total, 0);
        assert_eq!(runner.projection().committed_events, plan.total_events());
        assert_eq!(
            runner.projection().automatic_completed_us,
            plan.horizon_us()
        );
        assert_eq!(runner.session.state_digest().unwrap(), expected);

        let worker =
            PhysicsWorker::spawn_schedule(Arc::clone(&plan), WorkerPolicy::reviewed_default())
                .unwrap();
        let client = worker.client();
        command(&client, 0, mode).result.unwrap();
        command(&client, 1, ClockCommand::Resume).result.unwrap();
        let done = completed(&client);
        assert_eq!(done.committed_events, plan.total_events());
        assert_eq!(done.automatic_completed_us, plan.horizon_us());
        assert!(done.active_wall_us > 0);
        assert_eq!(worker.shutdown().unwrap().state_digest().unwrap(), expected);
    }
}

#[test]
fn single_step_respects_ties_and_restart_preserves_the_partial_timestamp() {
    let plan = plan(false);
    let worker =
        PhysicsWorker::spawn_schedule(Arc::clone(&plan), WorkerPolicy::reviewed_default()).unwrap();
    let client = worker.client();
    command(&client, 0, ClockCommand::SingleStep)
        .result
        .unwrap();
    let first = client.projection().unwrap();
    command(&client, 1, ClockCommand::SingleStep)
        .result
        .unwrap();
    let second = client.projection().unwrap();
    assert_eq!((first.time_us, second.time_us), (100, 100));
    assert_eq!(
        (
            first.fields.unwrap().instance,
            second.fields.unwrap().instance
        ),
        (Some(0), Some(1))
    );
    assert_eq!(second.automatic_completed_us, 0);
    let mut stopped = worker.shutdown().unwrap();
    let mut saved = Vec::new();
    stopped.write_snapshot(&mut saved).unwrap();
    let restored = PhysicsSchedule::read_snapshot(Arc::clone(&plan), Cursor::new(saved)).unwrap();
    let worker =
        PhysicsWorker::spawn_schedule_session(restored, WorkerPolicy::reviewed_default()).unwrap();
    let client = worker.client();
    assert!(client.projection().unwrap().fields.is_none());
    assert_eq!(client.projection().unwrap().time_us, 100);
    command(&client, 0, ClockCommand::Headless).result.unwrap();
    command(&client, 1, ClockCommand::Resume).result.unwrap();
    let done = completed(&client);
    assert_eq!(done.automatic_completed_us, plan.horizon_us() - 100);
    finish(&mut stopped);
    assert_eq!(
        worker.shutdown().unwrap().state_digest().unwrap(),
        stopped.state_digest().unwrap()
    );
}

#[test]
fn schedule_failure_and_latest_fields_survive_worker_shutdown_and_respawn() {
    let plan = plan(true);
    let mut expected = PhysicsSchedule::new(Arc::clone(&plan)).unwrap();
    while expected.step_next().is_ok() {}
    let expected_failure = expected.failure().unwrap();
    let mut runner = Runner::new(PhysicsSchedule::new(Arc::clone(&plan)).unwrap()).unwrap();
    runner
        .cycle(0, Some(request(0, ClockCommand::Headless)))
        .unwrap()
        .result
        .unwrap();
    runner
        .cycle(0, Some(request(1, ClockCommand::Resume)))
        .unwrap()
        .result
        .unwrap();
    while runner.projection().fault.is_none() {
        runner.cycle(0, None);
    }
    let status = runner.projection();
    assert_eq!(status.fault, Some(WorkerFault::Schedule(expected_failure)));
    assert_eq!(status.committed_events, expected.committed_events());
    assert_eq!(status.time_us, expected.time_us());
    assert!(status.paused);
    assert_eq!(status.fields.unwrap().instance, Some(1));
    assert_eq!(
        runner.session.state_digest().unwrap(),
        expected.state_digest().unwrap()
    );
    let worker =
        PhysicsWorker::spawn_schedule_session(runner.session, WorkerPolicy::reviewed_default())
            .unwrap();
    let client = worker.client();
    assert_eq!(
        client.projection().unwrap().fault,
        Some(WorkerFault::Schedule(expected_failure))
    );
    assert_eq!(
        command(&client, 0, ClockCommand::Resume).result,
        Err(CommandRejection::Faulted)
    );
    assert_eq!(
        worker.shutdown().unwrap().state_digest().unwrap(),
        expected.state_digest().unwrap()
    );
}

#[test]
fn aggregate_memory_preflight_counts_stack_mailbox_and_shared_schedule_once() {
    let plan = plan(false);
    let policy = WorkerPolicy::reviewed_default();
    let memory = WorkerMemoryPlan::compile_schedule(&plan, policy).unwrap();
    assert_eq!(
        memory.numerical_bytes,
        ScheduleMemoryPlan::compile(&plan)
            .unwrap()
            .total_planned_bytes
    );
    assert_eq!(
        memory.planned_bytes,
        memory.numerical_bytes + memory.mailbox_bytes + policy.stack_bytes as u64
    );
    let registrations = plan
        .instances()
        .iter()
        .map(|instance| PhysicsInstanceRegistration {
            identity: instance.identity.clone(),
            model: Arc::clone(&plan.profiles()[instance.profile]),
        })
        .collect();
    let limited = Arc::new(CompiledPhysicsSchedule::compile(registrations, 1024 * 1024).unwrap());
    assert!(PhysicsSchedule::new(Arc::clone(&limited)).is_ok());
    assert!(WorkerMemoryPlan::compile_schedule(&limited, policy).is_err());
    assert!(PhysicsWorker::spawn_schedule(limited, policy).is_err());
}

#[test]
fn aggregate_thread_loop_with_an_unread_full_mailbox_allocates_nothing() {
    let plan = plan(false);
    let mut expected = PhysicsSchedule::new(Arc::clone(&plan)).unwrap();
    finish(&mut expected);
    let mut runner = Runner::new(PhysicsSchedule::new(plan).unwrap()).unwrap();
    runner.clock.set_headless();
    runner.clock.set_paused(false);
    let shared = Arc::new(Shared::new(4, runner.projection()).unwrap());
    let client = PhysicsWorkerClient {
        shared: Arc::clone(&shared),
    };
    for _ in 0..4 {
        client.try_submit(99, ClockCommand::Pause).unwrap();
    }
    assert_eq!(
        client.try_submit(0, ClockCommand::Pause),
        Err(WorkerAccessError::MailboxFull)
    );
    let handle = std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(move || {
            let mut state = None;
            let allocation = allocation_counter::measure(|| state = Some(runner.run(shared)));
            (state.unwrap(), allocation)
        })
        .unwrap();
    let done = completed(&client);
    assert_eq!(done.command_high_water, 4);
    assert_eq!(done.outstanding_commands, 4);
    {
        let _guard = client.shared.mailbox.lock().unwrap();
        client.shared.shutdown.store(true, Ordering::Release);
        client.shared.wake.notify_all();
    }
    let (mut state, allocation) = handle.join().unwrap();
    assert_eq!(allocation.count_total, 0);
    assert_eq!(
        state.state_digest().unwrap(),
        expected.state_digest().unwrap()
    );
}

#[test]
fn event_clock_handles_equal_deadlines_and_checked_rejection_atomically() {
    let mut fixed = PhysicsPacer::new(10).unwrap();
    assert_eq!(
        fixed.set_next_event(Some(10)),
        Err(PacingError::WrongCadence)
    );
    let mut clock = PhysicsPacer::at_event_boundary(10, Some(10)).unwrap();
    clock.set_paused(true);
    assert!(!clock.step_due());
    clock.request_single_step().unwrap();
    clock.commit_step().unwrap();
    assert_eq!(clock.completed_us(), 10);
    assert_eq!(clock.wait_us(), None);
    let before = clock;
    assert_eq!(
        clock.set_next_event(Some(9)),
        Err(PacingError::InvalidBoundary)
    );
    assert_eq!(clock, before);
    clock.set_next_event(Some(20)).unwrap();
    clock.set_paused(false);
    clock.accrue(10).unwrap();
    clock.commit_step().unwrap();
    clock.set_next_event(Some(20)).unwrap();
    clock.commit_step().unwrap();
    assert_eq!(clock.completed_us(), 20);
    assert_eq!(clock.automatic_completed_us(), 10);
    assert_eq!(clock.automatic_events(), 2);
    assert_eq!(clock.commit_step(), Err(PacingError::NoStepDue));
    clock.set_next_event(Some(u64::MAX)).unwrap();
    clock.set_headless();
    clock.commit_step().unwrap();
    assert_eq!(clock.completed_us(), u64::MAX);
    clock.set_next_event(Some(u64::MAX)).unwrap();
    clock.commit_step().unwrap();
    assert_eq!(clock.completed_us(), u64::MAX);
}
