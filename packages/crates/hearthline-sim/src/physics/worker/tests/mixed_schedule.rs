use super::*;
use crate::physics::schedule::ScheduleInstanceError;
use crate::physics::schedule::tests::mixed::plan;
use std::io::Cursor;

fn finish(runtime: &mut PhysicsSchedule) {
    while !runtime.complete() {
        runtime.step_next().unwrap();
    }
}

#[test]
fn mixed_worker_keeps_per_profile_work_reports_and_all_pacing_modes_identical() {
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
        let allocation = allocation_counter::measure(|| {
            while !runner.projection().complete {
                runner.cycle(4_000_000, None);
                let status = runner.projection();
                assert!(status.fault.is_none());
                let fields = status.fields.unwrap();
                let binding = &plan.instances()[fields.instance.unwrap()];
                let model = &plan.profiles()[binding.profile];
                assert_eq!(fields.profile_digest.as_slice(), model.digest().as_bytes());
                if let Some(gas) = model.connected_gas() {
                    let work = fields.connected_work.unwrap();
                    assert_eq!(work.evaluation_bound, gas.evaluation_bound());
                    assert!(work.accepted_evaluations <= work.evaluation_bound);
                } else {
                    assert!(fields.connected_work.is_none());
                }
            }
        });
        assert_eq!(allocation.count_total, 0);
        assert_eq!(runner.session.state_digest().unwrap(), expected);

        let worker =
            PhysicsWorker::spawn_schedule(Arc::clone(&plan), WorkerPolicy::reviewed_default())
                .unwrap();
        let client = worker.client();
        for revision in 0..3 {
            command(&client, revision, ClockCommand::SingleStep)
                .result
                .unwrap();
        }
        let prefix = client.projection().unwrap();
        assert_eq!(prefix.time_us, 2002);
        assert_eq!(prefix.committed_events, 3);
        assert!(prefix.fields.unwrap().connected_work.is_some());
        let mut stopped = worker.shutdown().unwrap();
        let mut bytes = Vec::new();
        stopped.write_snapshot(&mut bytes).unwrap();
        let restored =
            PhysicsSchedule::read_snapshot(Arc::clone(&plan), Cursor::new(bytes)).unwrap();
        let worker =
            PhysicsWorker::spawn_schedule_session(restored, WorkerPolicy::reviewed_default())
                .unwrap();
        let client = worker.client();
        command(&client, 0, mode).result.unwrap();
        command(&client, 1, ClockCommand::Resume).result.unwrap();
        let done = completed(&client);
        assert_eq!(done.committed_events, plan.total_events());
        assert_eq!(done.time_us, plan.horizon_us());
        assert_eq!(
            done.automatic_completed_us,
            plan.horizon_us() - prefix.time_us
        );
        assert_eq!(worker.shutdown().unwrap().state_digest().unwrap(), expected);
    }
}

#[test]
fn restored_mixed_worker_cannot_resume_past_a_latched_gas_fault() {
    let plan = plan(true);
    let mut runtime = PhysicsSchedule::new(Arc::clone(&plan)).unwrap();
    runtime.step_next().unwrap();
    assert!(runtime.step_next().is_err());
    let failure = runtime.failure().unwrap();
    assert!(matches!(
        failure.error,
        ScheduleInstanceError::Connected { retry: Some(_), .. }
    ));
    let expected = runtime.state_digest().unwrap();
    let mut bytes = Vec::new();
    runtime.write_snapshot(&mut bytes).unwrap();
    let restored = PhysicsSchedule::read_snapshot(plan, Cursor::new(bytes)).unwrap();
    let worker =
        PhysicsWorker::spawn_schedule_session(restored, WorkerPolicy::reviewed_default()).unwrap();
    let client = worker.client();
    assert_eq!(
        client.projection().unwrap().fault,
        Some(WorkerFault::Schedule(failure))
    );
    assert_eq!(
        command(&client, 0, ClockCommand::Resume).result,
        Err(CommandRejection::Faulted)
    );
    assert_eq!(worker.shutdown().unwrap().state_digest().unwrap(), expected);
}
