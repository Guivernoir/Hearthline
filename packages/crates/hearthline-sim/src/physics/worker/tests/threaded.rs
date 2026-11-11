use super::*;

#[test]
fn dedicated_worker_resumes_full_state_and_does_not_own_the_calling_thread() {
    let model = model();
    let mut expected = PhysicsProofSession::new(Arc::clone(&model)).unwrap();
    let worker =
        PhysicsWorker::spawn(Arc::clone(&model), WorkerPolicy::reviewed_default()).unwrap();
    let client = worker.client();
    assert!(client.projection().unwrap().paused);
    assert_eq!(
        client.try_take_reply(CommandTicket(u64::MAX)),
        Err(WorkerAccessError::UnknownTicket)
    );
    for revision in 0..2 {
        assert!(
            command(&client, revision, ClockCommand::SingleStep)
                .result
                .is_ok()
        );
        expected.step().unwrap();
    }
    let saved = worker.shutdown().unwrap();
    assert_eq!(saved.checkpoint(), expected.checkpoint());
    assert!(client.projection().unwrap().terminated);
    assert_eq!(
        client.try_submit(2, ClockCommand::Resume),
        Err(WorkerAccessError::Stopped)
    );
    let worker = PhysicsWorker::spawn_session(saved, WorkerPolicy::reviewed_default()).unwrap();
    let client = worker.client();
    let restored = client.projection().unwrap();
    assert_eq!(restored.committed_events, 2);
    assert_eq!(restored.time_us, 2 * model.timestep_us());
    assert_eq!(restored.active_wall_us, 0);
    assert!(command(&client, 0, ClockCommand::Headless).result.is_ok());
    assert!(command(&client, 1, ClockCommand::Resume).result.is_ok());
    let done = completed(&client);
    assert!(done.active_wall_us > 0);
    assert_eq!(
        done.automatic_completed_us,
        u64::from(model.config().steps - 2) * model.timestep_us()
    );
    for _ in 2..model.config().steps {
        expected.step().unwrap();
    }
    assert_eq!(
        worker.shutdown().unwrap().checkpoint(),
        expected.checkpoint()
    );
}

#[test]
fn full_unread_mailbox_does_not_stop_headless_physics_or_allocate_on_the_worker() {
    let model = receiver(32);
    let mut expected = PhysicsProofSession::new(Arc::clone(&model)).unwrap();
    for _ in 0..model.config().steps {
        expected.step().unwrap();
    }
    let mut runner = Runner::new(PhysicsProofSession::new(model).unwrap()).unwrap();
    runner.clock.set_headless();
    runner.clock.set_paused(false);
    let shared = Arc::new(Shared::new(4, runner.projection()).unwrap());
    let client = PhysicsWorkerClient {
        shared: Arc::clone(&shared),
    };
    let tickets =
        core::array::from_fn::<_, 4, _>(|_| client.try_submit(99, ClockCommand::Pause).unwrap());
    assert_eq!(client.try_take_reply(tickets[0]), Ok(None));
    assert_eq!(
        client.try_submit(0, ClockCommand::Pause),
        Err(WorkerAccessError::MailboxFull)
    );
    let handle = std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(move || {
            let mut session = None;
            let allocation = allocation_counter::measure(|| {
                session = Some(runner.run(shared));
            });
            (session.unwrap(), allocation)
        })
        .unwrap();
    let done = completed(&client);
    assert_eq!(done.outstanding_commands, 4);
    assert_eq!(done.command_high_water, 4);
    for ticket in tickets {
        assert_eq!(
            reply(&client, ticket).result,
            Err(CommandRejection::StaleRevision { current: 0 })
        );
    }
    assert_eq!(client.projection().unwrap().outstanding_commands, 0);
    assert_eq!(
        client.try_take_reply(tickets[0]),
        Err(WorkerAccessError::UnknownTicket)
    );
    assert_eq!(
        command(&client, 0, ClockCommand::Resume).result,
        Err(CommandRejection::Complete)
    );
    {
        let _guard = client.shared.mailbox.lock().unwrap();
        client.shared.shutdown.store(true, Ordering::Release);
        client.shared.wake.notify_all();
    }
    let (session, allocation) = handle.join().unwrap();
    assert_eq!(allocation.count_total, 0);
    assert_eq!(session.checkpoint(), expected.checkpoint());
}

#[test]
fn concurrent_commands_cannot_both_apply_against_one_control_revision() {
    let worker = PhysicsWorker::spawn(receiver(2), WorkerPolicy::reviewed_default()).unwrap();
    let client = worker.client();
    let other = client.clone();
    let thread = std::thread::spawn(move || {
        other
            .try_submit(0, ClockCommand::SetRate(PlaybackRate::Five))
            .unwrap()
    });
    let first = client
        .try_submit(0, ClockCommand::SetRate(PlaybackRate::Two))
        .unwrap();
    let second = thread.join().unwrap();
    let replies = [reply(&client, first), reply(&client, second)];
    assert_eq!(replies.iter().filter(|r| r.result.is_ok()).count(), 1);
    assert_eq!(
        replies
            .iter()
            .filter(|r| r.result == Err(CommandRejection::StaleRevision { current: 1 }))
            .count(),
        1
    );
    assert_eq!(client.projection().unwrap().command_revision, 1);
    worker.shutdown().unwrap();
}

#[test]
fn worker_panic_closes_clients_and_join_reports_failure_even_with_a_poisoned_mailbox() {
    let model = receiver(1);
    let policy = WorkerPolicy::reviewed_default();
    let memory = WorkerMemoryPlan::compile(&model, policy).unwrap();
    let runner = Runner::new(PhysicsProofSession::new(model).unwrap()).unwrap();
    let shared = Arc::new(Shared::new(policy.command_capacity, runner.projection()).unwrap());
    let client = PhysicsWorkerClient {
        shared: Arc::clone(&shared),
    };
    let handle = std::thread::spawn(move || -> PhysicsProofSession {
        let _close = mailbox::CloseOnExit(&shared);
        let _mailbox = shared.mailbox.lock().unwrap();
        panic!("injected worker panic");
    });
    let worker = PhysicsWorker {
        client: client.clone(),
        thread: Some(handle),
        memory,
    };
    assert!(matches!(
        worker.shutdown(),
        Err(WorkerAccessError::Panicked)
    ));
    assert!(client.shared.ended.load(Ordering::Acquire));
    assert_eq!(client.projection(), Err(WorkerAccessError::Poisoned));
    assert_eq!(
        client.try_submit(0, ClockCommand::Resume),
        Err(WorkerAccessError::Poisoned)
    );
}

#[test]
fn shutdown_cannot_lose_a_wakeup_or_leave_an_orphaned_worker() {
    let model = receiver(1);
    for _ in 0..32 {
        let worker =
            PhysicsWorker::spawn(Arc::clone(&model), WorkerPolicy::reviewed_default()).unwrap();
        let client = worker.client();
        drop(worker);
        assert!(client.projection().unwrap().terminated);
    }
}

#[test]
fn shutdown_reports_pending_commands_as_stopped_or_returns_their_committed_acknowledgment() {
    let model = receiver(2);
    for _ in 0..32 {
        let worker =
            PhysicsWorker::spawn(Arc::clone(&model), WorkerPolicy::reviewed_default()).unwrap();
        let client = worker.client();
        let ticket = client.try_submit(0, ClockCommand::SingleStep).unwrap();
        let state = worker.shutdown().unwrap().checkpoint();
        assert!(client.projection().unwrap().terminated);
        match client.try_take_reply(ticket) {
            Ok(Some(reply)) => {
                assert!(reply.result.is_ok());
                assert_eq!(state.step, 1);
            }
            Err(WorkerAccessError::Stopped) => assert_eq!(state.step, 0),
            result => panic!("unfinished command after joined shutdown: {result:?}"),
        }
    }
}
