use hearthline_engine::{
    SequenceAssignment, SequenceCondition, SequenceInputs, SequenceProgram, SequenceRuntime,
    SequenceStep, SequenceTransition,
};

#[test]
fn bounded_sequence_runtime_prioritizes_trip_and_requires_reset() {
    assert!(core::mem::size_of::<SequenceRuntime>() <= 8 * 1024);
    let idle = SequenceStep::new(
        0,
        [SequenceAssignment {
            variable: "phase".into(),
            value: 0,
        }],
        Some(SequenceTransition {
            condition: SequenceCondition::StartPermitted,
            target: 10,
        }),
    )
    .expect("idle step");
    let running = SequenceStep::new(
        10,
        [SequenceAssignment {
            variable: "phase".into(),
            value: 10,
        }],
        Some(SequenceTransition {
            condition: SequenceCondition::TimerElapsed { duration_ms: 50 },
            target: 0,
        }),
    )
    .expect("running step");
    let fault = SequenceStep::new(
        900,
        [SequenceAssignment {
            variable: "phase".into(),
            value: 900,
        }],
        Some(SequenceTransition {
            condition: SequenceCondition::ResetPermitted,
            target: 0,
        }),
    )
    .expect("fault step");
    let program = SequenceProgram::new("test-sequence".into(), 20, 0, 900, [idle, running, fault])
        .expect("sequence program");
    let mut runtime = SequenceRuntime::new(program);

    runtime.execute_scan(SequenceInputs {
        start_request: true,
        safety_ready: true,
        ..SequenceInputs::default()
    });
    assert_eq!(runtime.current_step(), 10);
    assert!(runtime.running());

    runtime.execute_scan(SequenceInputs {
        trip_active: true,
        ..SequenceInputs::default()
    });
    assert_eq!(runtime.current_step(), 900);
    assert!(!runtime.running());

    runtime.execute_scan(SequenceInputs {
        reset_request: true,
        safety_ready: true,
        ..SequenceInputs::default()
    });
    assert_eq!(runtime.current_step(), 0);
}

#[test]
fn bounded_sequence_runtime_accepts_a_reviewed_timer_override() {
    let idle = SequenceStep::new(
        0,
        [],
        Some(SequenceTransition {
            condition: SequenceCondition::StartPermitted,
            target: 10,
        }),
    )
    .expect("idle step");
    let running = SequenceStep::new(
        10,
        [],
        Some(SequenceTransition {
            condition: SequenceCondition::TimerElapsed { duration_ms: 1_000 },
            target: 0,
        }),
    )
    .expect("running step");
    let fault = SequenceStep::new(900, [], None).expect("fault step");
    let program = SequenceProgram::new("override-test".into(), 20, 0, 900, [idle, running, fault])
        .expect("program");
    let mut runtime = SequenceRuntime::new(program);
    runtime.execute_scan(SequenceInputs {
        start_request: true,
        safety_ready: true,
        ..SequenceInputs::default()
    });
    runtime.elapse_with_timer_override(20, SequenceInputs::default(), Some(20));
    assert_eq!(runtime.current_step(), 0);
    assert_eq!(runtime.cycle_count(), 1);
}
