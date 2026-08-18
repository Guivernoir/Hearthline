use hearthline_engine::{
    BodyPreparationFault, BodyPreparationProcess, Comparison, DropReason, Effect, FormingFault,
    FormingMeasurements, FormingPhase, FormingProcess, FormingSetpoints, FormingTrip,
    HistorianBuffer, LogicRule, PUMP_HEARTBEAT_TIMEOUT_MS, PumpMaintenanceState, RobotCellArbiter,
    RobotCellRequestStatus, RobotCellStage, RobotJoints, RobotMotionKind, RobotMotionRuntime,
    RobotPose, RobotWorkspace, SafetyInterface, SequenceAssignment, SequenceCondition,
    SequenceInputs, SequenceProgram, SequenceRuntime, SequenceStep, SequenceTransition,
    SimulatedComponent, SimulationEvent, VirtualPlc,
};
use hearthline_model::{ComponentId, ProcessEvent, ProcessSignal, SignalValue};

fn id(value: &str) -> ComponentId {
    ComponentId::new(value).expect("test ID")
}

#[test]
fn robot_motion_interpolates_and_stops_at_the_configured_target() {
    let workspace = RobotWorkspace {
        minimum: RobotPose::new(-1_000.0, -1_000.0, 0.0, -180.0, -180.0, -180.0),
        maximum: RobotPose::new(1_000.0, 1_000.0, 2_000.0, 180.0, 180.0, 180.0),
        joint_minimum: RobotJoints::new([-170.0, -90.0, -150.0, -190.0, -125.0, -360.0]),
        joint_maximum: RobotJoints::new([170.0, 150.0, 150.0, 190.0, 125.0, 360.0]),
    };
    let home = RobotPose::new(0.0, 0.0, 1_200.0, 0.0, 90.0, 0.0);
    let target = RobotPose::new(800.0, 500.0, 900.0, 0.0, 90.0, 0.0);
    let mut robot = RobotMotionRuntime::new(workspace, home, 1_000.0, 90.0).expect("robot");
    robot
        .command_pose(target, RobotMotionKind::Linear, 50.0)
        .expect("motion command");

    assert!(robot.active());
    assert!(!robot.tick(500));
    assert!(robot.progress() > 0.0 && robot.progress() < 1.0);
    assert!(robot.pose().x > 0.0 && robot.pose().x < target.x);
    assert!(robot.tick(10_000));
    assert_eq!(robot.pose(), target);
    assert!(!robot.active());
}

#[test]
fn robot_cell_arbiter_grants_one_mould_and_preserves_fifo_order() {
    let mut arbiter = RobotCellArbiter::default();
    assert_eq!(arbiter.request("mould-03"), RobotCellRequestStatus::Granted);
    assert_eq!(arbiter.request("mould-01"), RobotCellRequestStatus::Queued);
    assert_eq!(arbiter.request("mould-02"), RobotCellRequestStatus::Queued);
    assert_eq!(arbiter.active(), Some("mould-03"));
    assert_eq!(
        arbiter.queue().collect::<Vec<_>>(),
        ["mould-01", "mould-02"]
    );

    arbiter.set_stage(RobotCellStage::Return);
    assert_eq!(
        arbiter.complete_active().expect("active").as_str(),
        "mould-03"
    );
    assert_eq!(arbiter.active(), Some("mould-01"));
    assert_eq!(arbiter.stage(), RobotCellStage::Approach);
}

#[test]
fn robot_cell_cancellation_does_not_count_as_a_completed_handoff() {
    let mut arbiter = RobotCellArbiter::default();
    assert_eq!(arbiter.request("mould-01"), RobotCellRequestStatus::Granted);
    assert_eq!(arbiter.request("mould-02"), RobotCellRequestStatus::Queued);

    arbiter.cancel("mould-01");

    assert_eq!(arbiter.completed(), 0);
    assert_eq!(arbiter.active(), Some("mould-02"));
    assert_eq!(arbiter.stage(), RobotCellStage::Approach);
}

fn signal(tag: &str, value: SignalValue) -> ProcessSignal {
    ProcessSignal {
        tag: tag.into(),
        value,
        quality_good: true,
        timestamp_ms: 0,
    }
}

#[test]
fn slip_pipeline_leak_adds_air_and_degrades_the_forming_material_contract() {
    let mut reference = BodyPreparationProcess::default();
    reference.start(true).expect("reference slip start");
    for _ in 0..120 {
        reference.tick(500);
        if reference.released_slip().is_some() {
            break;
        }
    }
    let reference_batch = reference.released_slip().expect("reference released slip");

    let mut leaking = BodyPreparationProcess::default();
    leaking.start(true).expect("leaking slip start");
    leaking.set_fault(Some(BodyPreparationFault::SlipPipelineLeak));
    for _ in 0..120 {
        leaking.tick(500);
        if leaking.phase() == hearthline_engine::SlipPhase::Transfer {
            let line = leaking.measurements().pipelines.slip_to_forming;
            assert!(line.leak_detected);
            assert!(line.outlet_flow_l_min < line.inlet_flow_l_min);
            assert!(line.entrained_air_percent > 3.0);
        }
        if leaking.released_slip().is_some() {
            break;
        }
    }
    let leaking_batch = leaking.released_slip().expect("degraded released slip");
    assert!(leaking_batch.entrained_air_percent > reference_batch.entrained_air_percent);
    assert!(
        leaking_batch.effects.filling_flow_factor < reference_batch.effects.filling_flow_factor
    );
    assert!(
        leaking_batch.effects.green_strength_index < reference_batch.effects.green_strength_index
    );
    assert!(
        leaking_batch.effects.fired_defect_risk_percent
            > reference_batch.effects.fired_defect_risk_percent
    );
    let retained_line = leaking.measurements().pipelines.slip_to_forming;
    assert!(retained_line.leak_detected);
    assert!(retained_line.line_loss_percent > 20.0);
    assert_eq!(leaking.slip_effects_preview(), leaking_batch.effects);
}
#[test]
fn water_distribution_exposes_measured_hydraulics_and_quality() {
    let mut process = BodyPreparationProcess::default();
    process.tick(500);

    let networks = process.measurements().water_networks;
    let header = networks
        .routes
        .iter()
        .find(|route| route.id == "industrial-header")
        .expect("industrial header");
    assert!(header.available);
    assert!(header.inlet_pressure_bar > header.outlet_pressure_bar);
    assert!(header.outlet_flow_l_min > 0.0);
    assert_eq!(header.quality.ph, process.measurements().water.product.ph);
    assert!(networks.pumps.iter().all(|pump| pump.heartbeat_ok));
}
#[test]
fn lost_water_pump_heartbeat_transfers_duty_and_requires_maintenance() {
    let mut process = BodyPreparationProcess::default();
    process.tick(500);
    assert!(process.set_water_pump_failed("area-01-wd-pmp-01a", true));
    process.tick(PUMP_HEARTBEAT_TIMEOUT_MS);

    let networks = process.measurements().water_networks;
    let duty = networks
        .pumps
        .iter()
        .find(|pump| pump.id == "area-01-wd-pmp-01a")
        .expect("duty pump");
    let standby = networks
        .pumps
        .iter()
        .find(|pump| pump.id == "area-01-wd-pmp-01b")
        .expect("standby pump");
    assert!(!duty.heartbeat_ok);
    assert_eq!(duty.maintenance, PumpMaintenanceState::Required);
    assert!(standby.running_feedback);
    assert!(process.dispatch_water_pump_maintenance(duty.id));
    assert_eq!(
        process
            .measurements()
            .water_networks
            .pumps
            .iter()
            .find(|pump| pump.id == duty.id)
            .expect("dispatched pump")
            .maintenance,
        PumpMaintenanceState::Dispatched
    );
}
#[test]
fn virtual_plc_scans_on_period_and_updates_output() {
    let mut plc = VirtualPlc::new(
        id("area-01-vplc-01"),
        [],
        100,
        [LogicRule {
            input: "level-high".into(),
            comparison: Comparison::BoolEquals(true),
            output: "pump-run".into(),
            value_when_true: SignalValue::Bool(false),
            value_when_false: SignalValue::Bool(true),
        }],
    );
    plc.handle(SimulationEvent::Process(ProcessEvent::Signal(signal(
        "level-high",
        SignalValue::Bool(false),
    ))));
    assert!(
        plc.handle(SimulationEvent::Process(ProcessEvent::Tick {
            elapsed_ms: 99
        }))
        .is_empty()
    );
    let effects = plc.handle(SimulationEvent::Process(ProcessEvent::Tick {
        elapsed_ms: 1,
    }));
    assert_eq!(effects.len(), 1);
    assert_eq!(
        plc.outputs()
            .iter()
            .find(|(tag, _)| tag.as_str() == "pump-run")
            .map(|(_, value)| value),
        Some(&SignalValue::Bool(true))
    );
}
#[test]
fn safety_reset_requires_authorization_and_all_permissives() {
    let mut safety = SafetyInterface::new(
        id("area-06-bms-01"),
        ["airflow-ok".into(), "gas-pressure-ok".into()],
    );
    safety.handle(SimulationEvent::Process(ProcessEvent::Signal(signal(
        "airflow-ok",
        SignalValue::Bool(true),
    ))));
    safety.handle(SimulationEvent::Process(ProcessEvent::Signal(signal(
        "gas-pressure-ok",
        SignalValue::Bool(true),
    ))));
    let denied = safety.handle(SimulationEvent::Process(ProcessEvent::Reset {
        authorized: false,
    }));
    assert!(matches!(denied[0], Effect::Drop(DropReason::SafetyTrip(_))));
    safety.handle(SimulationEvent::Process(ProcessEvent::Reset {
        authorized: true,
    }));
    assert!(!safety.trip_latched());
}

fn forming_process() -> FormingProcess {
    FormingProcess::new(FormingMeasurements {
        slip_tank_level_percent: 72.0,
        slip_density_g_cm3: 1.82,
        slip_viscosity_mpa_s: 1_800.0,
        slip_temperature_c: 40.0,
        slip_feed_flow_l_min: 0.0,
        slip_feed_pressure_bar: 2.5,
        mould_pressure_bar: 0.0,
        mould_temperature_c: 25.0,
        fill_head_position_mm: 0.0,
        mould_position_mm: 0.0,
        water_flow_l_min: 0.0,
        excess_slip_drain_flow_l_min: 0.0,
        mould_moisture_percent: 8.0,
        compressed_air_pressure_bar: 6.0,
        vacuum_pressure_kpa: 0.0,
        robot_position_mm: 0.0,
        piece_gripped: false,
        piece_moisture_percent: 20.5,
        predicted_drying_shrinkage_percent: 2.1,
        drying_energy_factor: 1.0,
        green_strength_index: 100.0,
        fired_defect_risk_percent: 3.0,
    })
}
#[test]
fn forming_cycle_changes_measurements_and_returns_to_idle() {
    let mut process = forming_process();
    process.start(true).expect("cycle start");

    process.tick(750);
    assert_eq!(process.phase(), FormingPhase::Filling);
    assert_eq!(process.outputs().slip, "filling");
    assert_eq!(process.measurements().slip_feed_flow_l_min, 85.0);
    assert_eq!(process.measurements().fill_head_position_mm, 400.0);

    process.tick(13_250);
    assert_eq!(process.phase(), FormingPhase::Idle);
    assert!(!process.running());
    assert_eq!(process.cycle_count(), 1);
    assert_eq!(process.outputs().mould, "closed");
    assert_eq!(process.measurements().mould_position_mm, 0.0);
    assert!(!process.measurements().piece_gripped);
    assert_eq!(process.scan_count(), 700);
}
#[test]
fn forming_cycle_keeps_release_assist_separate_from_mould_cleaning() {
    let mut process = forming_process();
    process.start(true).expect("cycle start");

    process.tick(1_500);
    assert_eq!(process.phase(), FormingPhase::Pressurizing);
    assert_eq!(process.outputs().air, "pressurizing");

    process.tick(750);
    assert_eq!(process.phase(), FormingPhase::PressureDwell);

    process.tick(2_500);
    assert_eq!(process.phase(), FormingPhase::Depressurizing);
    assert_eq!(process.outputs().air, "isolated");

    process.tick(500);
    assert_eq!(process.phase(), FormingPhase::Draining);
    assert_eq!(process.outputs().slip, "draining");

    process.tick(1000);
    assert_eq!(process.phase(), FormingPhase::ReleaseWater);
    assert_eq!(process.outputs().water, "release-wet");
    process.tick(400);
    assert_eq!(process.phase(), FormingPhase::ReleaseAir);
    assert_eq!(process.outputs().water, "isolated");
    assert_eq!(process.outputs().air, "release-assist");

    process.tick(400);
    assert_eq!(process.phase(), FormingPhase::OpeningMould);
    process.tick(750);
    assert_eq!(process.phase(), FormingPhase::RobotPickup);
    process.tick(1_000);
    assert_eq!(process.phase(), FormingPhase::RobotDelivery);
    assert_eq!(process.outputs().robot, "delivering");

    process.tick(1_200);
    assert_eq!(process.phase(), FormingPhase::MouldWash);
    assert_eq!(process.outputs().water, "mould-wash");
    process.tick(1_000);
    assert_eq!(process.phase(), FormingPhase::AirPurge);
    assert_eq!(process.outputs().water, "isolated");
    assert_eq!(process.outputs().air, "cleaning-purge");
    process.tick(750);
    assert_eq!(process.phase(), FormingPhase::VacuumDry);
    assert_eq!(process.outputs().air, "isolated");
    assert_eq!(process.outputs().vacuum, "vacuum-drying");
}

#[test]
fn forming_vacuum_fault_trips_sequence_to_safe_outputs() {
    let mut process = forming_process();
    process.start(true).expect("cycle start");
    process.tick(11_750);
    process.set_fault(Some(FormingFault::VacuumLoss));

    let result = process.tick(800);
    assert_eq!(result.trip, Some(FormingTrip::VacuumNotEstablished));
    assert_eq!(process.phase(), FormingPhase::Faulted);
    assert_eq!(process.measurements().vacuum_pressure_kpa, -10.0);
    assert_eq!(process.outputs().vacuum, "stopped");
    assert_eq!(process.outputs().mould, "stopped");

    process.set_fault(None);
    assert!(process.reset_after_trip(true));
    assert_eq!(process.phase(), FormingPhase::Idle);
}

#[test]
fn forming_setpoints_drive_phase_duration_and_pressure_dynamics() {
    let setpoints = FormingSetpoints {
        fill_ms: 2_200,
        pressure_bar: 7.4,
        ..FormingSetpoints::default()
    };
    let mut process = forming_process().with_setpoints(setpoints);
    process.start(true).expect("cycle start");
    process.tick(2_000);
    assert_eq!(process.phase(), FormingPhase::Filling);

    process.tick(200);
    assert_eq!(process.phase(), FormingPhase::Pressurizing);
    process.tick(750);
    assert_eq!(process.phase(), FormingPhase::PressureDwell);
    assert_eq!(process.measurements().mould_pressure_bar, 7.4);
}

#[test]
fn historian_buffer_counts_pending_eviction() {
    let mut buffer = HistorianBuffer::<u64, 3>::new();
    buffer.push(1, false);
    buffer.push(2, true);
    buffer.push(3, false);
    buffer.push(4, false);

    assert_eq!(buffer.len(), 3);
    assert_eq!(buffer.pending_count(), 2);
    assert_eq!(buffer.dropped_unreplicated(), 1);
    assert_eq!(
        buffer.iter().map(|(value, _)| *value).collect::<Vec<_>>(),
        [2, 3, 4]
    );
}

#[test]
fn historian_buffer_acknowledges_oldest_pending_record() {
    let mut buffer = HistorianBuffer::<&str, 3>::new();
    buffer.push("sample-1", false);
    buffer.push("sample-2", false);

    let (index, sample) = buffer.oldest_pending().expect("pending sample");
    assert_eq!(*sample, "sample-1");
    assert!(buffer.mark_replicated(index));

    assert_eq!(buffer.pending_count(), 1);
    assert_eq!(buffer.latest(), Some(&"sample-2"));
}

#[test]
fn bounded_sequence_runtime_prioritizes_trip_and_requires_reset() {
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
