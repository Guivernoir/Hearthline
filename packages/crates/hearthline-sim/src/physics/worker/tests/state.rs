use super::*;
use crate::physics::clock::PhysicsPacer;

#[test]
fn clock_commands_are_revision_checked_and_pause_is_not_a_physical_step() {
    let model = model();
    let dt = model.timestep_us();
    let mut runner = Runner::new(PhysicsProofSession::new(model).unwrap()).unwrap();
    let initial = runner.session.checkpoint();
    runner.cycle(1_000_000_000, None);
    assert_eq!(runner.session.checkpoint(), initial);
    assert_eq!(runner.clock.elapsed_wall_us(), 0);
    let stale = runner
        .cycle(0, Some(request(1, ClockCommand::SingleStep)))
        .unwrap();
    assert_eq!(
        stale.result,
        Err(CommandRejection::StaleRevision { current: 0 })
    );
    assert_eq!(runner.session.checkpoint(), initial);
    let measured = allocation_counter::measure(|| {
        assert!(
            runner
                .cycle(0, Some(request(0, ClockCommand::SingleStep)))
                .unwrap()
                .result
                .is_ok()
        );
        assert_eq!(runner.projection().committed_events, 1);
        assert_eq!(runner.projection().automatic_completed_us, 0);
        assert!(
            runner
                .cycle(
                    0,
                    Some(request(1, ClockCommand::SetRate(PlaybackRate::Five)))
                )
                .unwrap()
                .result
                .is_ok()
        );
        assert!(
            runner
                .cycle(0, Some(request(2, ClockCommand::Resume)))
                .unwrap()
                .result
                .is_ok()
        );
        assert_eq!(
            runner
                .cycle(0, Some(request(3, ClockCommand::SingleStep)))
                .unwrap()
                .result,
            Err(CommandRejection::NotPaused)
        );
        runner.cycle(dt / 5, None);
        assert_eq!(runner.projection().committed_events, 2);
        assert_eq!(runner.projection().achieved_milli_rate(), Some(5000));
        assert!(
            runner
                .cycle(0, Some(request(3, ClockCommand::Pause)))
                .unwrap()
                .result
                .is_ok()
        );
        runner.cycle(dt * 100, None);
        assert_eq!(runner.projection().committed_events, 2);
        assert_eq!(runner.projection().time_us, 2 * dt);
    });
    assert_eq!(measured.count_total, 0);
}

#[test]
fn every_pacing_mode_reconstructs_the_same_full_fields_and_preserves_backlog() {
    let model = model();
    let mut expected = PhysicsProofSession::new(Arc::clone(&model)).unwrap();
    for _ in 0..model.config().steps {
        expected.step().unwrap();
    }
    for mode in [
        PlaybackMode::Headless,
        PlaybackMode::Paced(PlaybackRate::One),
        PlaybackMode::Paced(PlaybackRate::Two),
        PlaybackMode::Paced(PlaybackRate::Five),
        PlaybackMode::Paced(PlaybackRate::Ten),
    ] {
        let mut runner =
            Runner::new(PhysicsProofSession::new(Arc::clone(&model)).unwrap()).unwrap();
        let mode_command = match mode {
            PlaybackMode::Headless => ClockCommand::Headless,
            PlaybackMode::Paced(rate) => ClockCommand::SetRate(rate),
        };
        assert!(
            runner
                .cycle(0, Some(request(0, mode_command)))
                .unwrap()
                .result
                .is_ok()
        );
        assert!(
            runner
                .cycle(0, Some(request(1, ClockCommand::Resume)))
                .unwrap()
                .result
                .is_ok()
        );
        let allocation = allocation_counter::measure(|| {
            while !runner.projection().complete {
                runner.cycle(model.timestep_us() * 2, None);
                if mode != PlaybackMode::Headless {
                    assert!(runner.projection().pending_pacing_us >= model.timestep_us());
                }
            }
        });
        assert_eq!(allocation.count_total, 0);
        let checkpoint = runner.session.checkpoint();
        assert_eq!(checkpoint, expected.checkpoint());
        let allocation = allocation_counter::measure(|| {
            assert_eq!(
                runner.session.state_digest().unwrap().as_slice(),
                checkpoint.digest.as_bytes()
            );
        });
        assert_eq!(allocation.count_total, 0);
        assert_eq!(
            runner
                .cycle(0, Some(request(2, ClockCommand::Resume)))
                .unwrap()
                .result,
            Err(CommandRejection::Complete)
        );
    }
}

#[test]
fn rejected_solver_step_latches_diagnostic_without_advancing_the_clock_or_fields() {
    let mut config = receiver(20).config().clone();
    config.maximum_iterations = 1;
    config.residual_tolerance = "0.000000001".into();
    let hearthline_config::physics::PhysicsProofCase::CompressibleReceiver { flow } =
        &mut config.case
    else {
        unreachable!()
    };
    flow.maximum_substeps = 1;
    let mut runner = Runner::new(
        PhysicsProofSession::new(Arc::new(CompiledPhysicsProof::compile(config).unwrap())).unwrap(),
    )
    .unwrap();
    let before = runner.session.checkpoint();
    let allocation = allocation_counter::measure(|| {
        let reply = runner
            .cycle(0, Some(request(0, ClockCommand::SingleStep)))
            .unwrap();
        assert!(reply.result.is_ok());
        assert!(matches!(
            runner.projection().fault,
            Some(WorkerFault::Numerical(_))
        ));
        assert!(runner.projection().paused);
        assert_eq!(runner.projection().time_us, 0);
        assert_eq!(
            runner
                .cycle(0, Some(request(1, ClockCommand::Resume)))
                .unwrap()
                .result,
            Err(CommandRejection::Faulted)
        );
    });
    assert_eq!(allocation.count_total, 0);
    assert_eq!(runner.session.checkpoint(), before);
}

#[test]
fn pacing_mode_transitions_and_boundary_overflow_are_checked() {
    assert!(matches!(
        PhysicsPacer::at_boundary(3, 2),
        Err(PacingError::InvalidBoundary)
    ));
    let mut clock = PhysicsPacer::at_boundary(10, 100).unwrap();
    clock.accrue(50).unwrap();
    clock.set_headless();
    assert_eq!(clock.pending_credit_us(), 0);
    clock.commit_step().unwrap();
    assert_eq!(clock.completed_us(), 110);
    clock.set_paused(true);
    assert_eq!(clock.wait_us(), None);
    clock.request_single_step().unwrap();
    assert_eq!(clock.wait_us(), Some(0));
    clock.commit_step().unwrap();
    assert_eq!(clock.completed_us(), 120);
    assert_eq!(clock.automatic_completed_us(), 10);
    clock.set_rate(PlaybackRate::Two);
    clock.set_paused(false);
    assert_eq!(clock.wait_us(), Some(5));
    let mut overflow = PhysicsPacer::at_boundary(1, u64::MAX).unwrap();
    overflow.set_headless();
    assert_eq!(overflow.commit_step(), Err(PacingError::Overflow));
    assert_eq!(overflow.completed_us(), u64::MAX);
    assert_eq!(overflow.automatic_completed_us(), 0);
}

#[test]
fn mailbox_allocation_and_policy_reserves_are_measured_not_inferred_from_a_constant() {
    let model = model();
    let policy = WorkerPolicy::reviewed_default();
    let plan = WorkerMemoryPlan::compile(&model, policy).unwrap();
    let runner = Runner::new(PhysicsProofSession::new(Arc::clone(&model)).unwrap()).unwrap();
    let mut shared = None;
    let allocation = allocation_counter::measure(|| {
        shared = Some(Arc::new(
            Shared::new(policy.command_capacity, runner.projection()).unwrap(),
        ));
    });
    assert_eq!(allocation.bytes_total, plan.mailbox_bytes);
    assert_eq!(
        plan.planned_bytes,
        plan.numerical_bytes + plan.mailbox_bytes + plan.requested_stack_bytes
    );
    assert!(plan.planned_bytes <= plan.reviewed_budget_bytes);
    for bad in [
        WorkerPolicy {
            command_capacity: 0,
            ..policy
        },
        WorkerPolicy {
            reviewed_command_burst: 0,
            ..policy
        },
        WorkerPolicy {
            command_capacity: 15,
            ..policy
        },
        WorkerPolicy {
            command_capacity: usize::MAX,
            ..policy
        },
        WorkerPolicy {
            stack_bytes: 1,
            ..policy
        },
    ] {
        assert!(WorkerMemoryPlan::compile(&model, bad).is_err());
    }
    let mut limited = model.config().clone();
    limited.allocation_budget_bytes = 1024 * 1024;
    let limited = CompiledPhysicsProof::compile_with_resources(
        limited,
        model.geometry().cloned(),
        model.material().map(|material| material.document().clone()),
    );
    // A valid proof-buffer budget does not automatically include a worker stack.
    assert!(limited.is_ok());
    assert!(WorkerMemoryPlan::compile(&limited.unwrap(), policy).is_err());
}
