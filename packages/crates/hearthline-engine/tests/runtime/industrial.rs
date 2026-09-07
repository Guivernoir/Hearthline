use hearthline_engine::{
    Actuator, BodyPreparationControlState, BodyPreparationFault, BodyPreparationPhysicsInputs,
    BodyPreparationProcess, Comparison, DropReason, Effect, FieldSensor, FirewallHaControl,
    FormingFault, FormingMeasurements, FormingPhase, FormingProcess, FormingSetpoints, FormingTrip,
    HistorianBuffer, IoDirection, LogicRule, NetworkIngress, OperatorInterface,
    PUMP_HEARTBEAT_TIMEOUT_MS, ProcessEffect, PumpMaintenanceState, RemoteIo, RobotCellArbiter,
    RobotCellRequestStatus, RobotCellStage, RobotJoints, RobotMotionKind, RobotMotionRuntime,
    RobotPose, RobotWorkspace, SafetyInterface, SimulatedComponent, SimulationEvent, SlipPhase,
    VirtualPlc,
};
use hearthline_model::{
    Angle, AngularSpeed, ApplicationData, ArpOperation, ArpPacket, ComponentId, ComponentKind,
    EthernetFrame, Ipv4Packet, LinearSpeed, MacAddress, NetworkPayload, Percentage, PortId,
    Position, ProcessCommand, ProcessEvent, ProcessSignal, ServiceKind, SignalValue, TcpFlags,
    TcpSegment, Transport, VlanId, fixed,
};
use std::net::Ipv4Addr;

mod sequence;

fn id(value: &str) -> ComponentId {
    ComponentId::new(value).expect("test ID")
}

fn pose(position_mm: [i64; 3], angle_deg: [i64; 3]) -> RobotPose {
    RobotPose::new(
        Position::from_raw(position_mm[0] * 1_000),
        Position::from_raw(position_mm[1] * 1_000),
        Position::from_raw(position_mm[2] * 1_000),
        Angle::from_raw(angle_deg[0] * 1_000),
        Angle::from_raw(angle_deg[1] * 1_000),
        Angle::from_raw(angle_deg[2] * 1_000),
    )
}

fn joints(angle_deg: [i64; 6]) -> RobotJoints {
    RobotJoints::new(angle_deg.map(|value| Angle::from_raw(value * 1_000)))
}

#[test]
fn robot_motion_interpolates_and_stops_at_the_configured_target() {
    let workspace = RobotWorkspace {
        minimum: pose([-1_000, -1_000, 0], [-180, -180, -180]),
        maximum: pose([1_000, 1_000, 2_000], [180, 180, 180]),
        joint_minimum: joints([-170, -90, -150, -190, -125, -360]),
        joint_maximum: joints([170, 150, 150, 190, 125, 360]),
    };
    let home = pose([0, 0, 1_200], [0, 90, 0]);
    let target = pose([800, 500, 900], [0, 90, 0]);
    let mut robot = RobotMotionRuntime::new(
        workspace,
        home,
        LinearSpeed::from_raw(1_000_000),
        AngularSpeed::from_raw(90_000),
    )
    .expect("robot");
    robot
        .command_pose(target, RobotMotionKind::Linear, Percentage::from_raw(5_000))
        .expect("motion command");

    assert!(robot.active());
    assert!(!robot.tick(500));
    assert!(robot.progress() > Percentage::ZERO && robot.progress().raw() < 10_000);
    assert!(robot.pose().x > Position::ZERO && robot.pose().x < target.x);
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

fn command(tag: &str, value: SignalValue) -> ProcessCommand {
    ProcessCommand {
        tag: tag.into(),
        value,
        source: "contract-test".into(),
    }
}

fn industrial_frame(application: ApplicationData, destination_port: u16) -> EthernetFrame {
    EthernetFrame {
        source: MacAddress::new([0x02, 0, 0, 0, 0, 1]),
        destination: MacAddress::new([0x02, 0, 0, 0, 0, 2]),
        vlan: VlanId::new(10).unwrap(),
        payload: NetworkPayload::Ipv4(Ipv4Packet {
            source: Ipv4Addr::new(192, 0, 2, 1),
            destination: Ipv4Addr::new(192, 0, 2, 2),
            ttl: 64,
            transport: Transport::Tcp(TcpSegment {
                source_port: 40_000,
                destination_port,
                flags: TcpFlags::default(),
            }),
            application,
        }),
        wire_len_bytes: 64,
    }
}

fn ingress(port: &str, frame: EthernetFrame) -> SimulationEvent {
    SimulationEvent::Network(NetworkIngress {
        port: PortId::new(port).unwrap(),
        frame,
        received_at_us: 0,
    })
}

#[test]
fn virtual_plc_comparisons_quality_and_lifecycle_have_explicit_outputs() {
    assert!(std::panic::catch_unwind(|| VirtualPlc::new(id("bad-plc"), [], 0, [])).is_err());
    let rules = [
        LogicRule {
            input: "bool".into(),
            comparison: Comparison::BoolEquals(true),
            output: "bool-out".into(),
            value_when_true: SignalValue::Bool(true),
            value_when_false: SignalValue::Bool(false),
        },
        LogicRule {
            input: "analog-gt".into(),
            comparison: Comparison::AnalogGreaterThan(fixed!(5.0)),
            output: "analog-gt-out".into(),
            value_when_true: SignalValue::Bool(true),
            value_when_false: SignalValue::Bool(false),
        },
        LogicRule {
            input: "analog-lt".into(),
            comparison: Comparison::AnalogLessThan(fixed!(5.0)),
            output: "analog-lt-out".into(),
            value_when_true: SignalValue::Bool(true),
            value_when_false: SignalValue::Bool(false),
        },
        LogicRule {
            input: "integer-gt".into(),
            comparison: Comparison::IntegerGreaterThan(5),
            output: "integer-gt-out".into(),
            value_when_true: SignalValue::Bool(true),
            value_when_false: SignalValue::Bool(false),
        },
        LogicRule {
            input: "integer-lt".into(),
            comparison: Comparison::IntegerLessThan(5),
            output: "integer-lt-out".into(),
            value_when_true: SignalValue::Bool(true),
            value_when_false: SignalValue::Bool(false),
        },
        LogicRule {
            input: "type-mismatch".into(),
            comparison: Comparison::AnalogGreaterThan(fixed!(0.0)),
            output: "type-mismatch-out".into(),
            value_when_true: SignalValue::Bool(true),
            value_when_false: SignalValue::Bool(false),
        },
        LogicRule {
            input: "missing".into(),
            comparison: Comparison::BoolEquals(true),
            output: "missing-out".into(),
            value_when_true: SignalValue::Bool(true),
            value_when_false: SignalValue::Bool(false),
        },
    ];
    let mut plc = VirtualPlc::new(
        id("comparison-plc"),
        [PortId::new("network").unwrap()],
        10,
        rules,
    );
    for (tag, value) in [
        ("bool", SignalValue::Bool(true)),
        ("analog-gt", SignalValue::Analog(fixed!(6.0))),
        ("analog-lt", SignalValue::Analog(fixed!(4.0))),
        ("integer-gt", SignalValue::Integer(6)),
        ("integer-lt", SignalValue::Integer(4)),
        ("type-mismatch", SignalValue::Bool(true)),
    ] {
        assert!(
            plc.handle(SimulationEvent::Process(ProcessEvent::Signal(signal(
                tag, value
            ))))
            .is_empty()
        );
    }
    let first = plc.handle(SimulationEvent::Process(ProcessEvent::Tick {
        elapsed_ms: 10,
    }));
    assert_eq!(first.len(), 7);
    assert!(first.iter().any(|effect| matches!(effect, Effect::Process(hearthline_engine::ProcessEffect::Alarm { code, .. }) if code.as_str() == "PLC-MISSING-INPUT")));
    let unchanged = plc.handle(SimulationEvent::Process(ProcessEvent::Tick {
        elapsed_ms: 10,
    }));
    assert_eq!(unchanged.len(), 1);

    for (tag, value) in [
        ("bool", SignalValue::Bool(false)),
        ("analog-gt", SignalValue::Analog(fixed!(4.0))),
        ("analog-lt", SignalValue::Analog(fixed!(6.0))),
        ("integer-gt", SignalValue::Integer(4)),
        ("integer-lt", SignalValue::Integer(6)),
    ] {
        plc.handle(SimulationEvent::Process(ProcessEvent::Signal(signal(
            tag, value,
        ))));
    }
    let changed = plc.handle(SimulationEvent::Process(ProcessEvent::Tick {
        elapsed_ms: 20,
    }));
    assert!(changed.len() >= 6);
    assert!(matches!(
        plc.handle(SimulationEvent::Process(ProcessEvent::Command(command(
            "manual-output",
            SignalValue::Bool(true),
        ))))[0],
        Effect::Process(hearthline_engine::ProcessEffect::Command(_))
    ));
    for authorized in [false, true] {
        assert!(matches!(
            plc.handle(SimulationEvent::Process(ProcessEvent::Reset { authorized }))[0],
            Effect::Process(hearthline_engine::ProcessEffect::State { .. })
        ));
    }
    assert!(matches!(
        plc.handle(ingress(
            "missing",
            industrial_frame(ApplicationData::None, 80),
        ))[0],
        Effect::Drop(DropReason::InvalidIngress(_))
    ));
    assert!(matches!(
        plc.handle(ingress(
            "network",
            industrial_frame(ApplicationData::Service(ServiceKind::IndustrialIo), 80),
        ))[0],
        Effect::Deliver { .. }
    ));
    let arp = EthernetFrame {
        payload: NetworkPayload::Arp(ArpPacket {
            operation: ArpOperation::Request,
            sender_mac: MacAddress::new([0x02, 0, 0, 0, 0, 1]),
            sender_ip: Ipv4Addr::new(192, 0, 2, 1),
            target_mac: None,
            target_ip: Ipv4Addr::new(192, 0, 2, 2),
        }),
        ..industrial_frame(ApplicationData::None, 80)
    };
    assert!(matches!(
        plc.handle(ingress("network", arp))[0],
        Effect::Drop(DropReason::UnsupportedProtocol)
    ));
    assert!(matches!(
        plc.handle(SimulationEvent::Process(ProcessEvent::Trip {
            cause: "external trip".into(),
        }))[0],
        Effect::Process(hearthline_engine::ProcessEffect::Alarm { .. })
    ));
    assert!(plc.outputs().is_empty());
    plc.handle(SimulationEvent::SetOperational(false));
    assert!(matches!(
        plc.handle(SimulationEvent::Process(ProcessEvent::Tick {
            elapsed_ms: 10
        }))[0],
        Effect::Drop(DropReason::ComponentDown)
    ));
    plc.handle(SimulationEvent::SetOperational(true));
    assert!(matches!(
        plc.handle(SimulationEvent::FirewallHa(
            FirewallHaControl::HeartbeatTick { at_us: 0 }
        ))[0],
        Effect::Drop(DropReason::UnsupportedProtocol)
    ));
}

#[test]
fn remote_io_and_operator_interfaces_enforce_direction_and_authority() {
    let network = PortId::new("network").unwrap();
    let channels = [
        ("level".into(), IoDirection::Input),
        ("pump".into(), IoDirection::Output),
    ];
    let mut io = RemoteIo::new(id("remote-io"), [network.clone()], channels);
    assert!(io.has_port(&network));
    assert!(matches!(
        io.handle(ingress(
            "missing",
            industrial_frame(ApplicationData::None, 502)
        ))[0],
        Effect::Drop(DropReason::InvalidIngress(_))
    ));
    assert!(matches!(
        io.handle(ingress(
            "network",
            industrial_frame(ApplicationData::None, 502)
        ))[0],
        Effect::Deliver { .. }
    ));
    assert!(matches!(
        io.handle(ingress(
            "network",
            industrial_frame(ApplicationData::None, 443)
        ))[0],
        Effect::Drop(DropReason::PolicyDenied { .. })
    ));
    assert!(matches!(
        io.handle(SimulationEvent::Process(ProcessEvent::Signal(signal(
            "level",
            SignalValue::Analog(fixed!(50.0)),
        ))))[0],
        Effect::Process(hearthline_engine::ProcessEffect::Signal(_))
    ));
    assert!(matches!(
        io.handle(SimulationEvent::Process(ProcessEvent::Signal(signal(
            "pump",
            SignalValue::Bool(true),
        ))))[0],
        Effect::Process(hearthline_engine::ProcessEffect::Alarm { .. })
    ));
    assert!(matches!(
        io.handle(SimulationEvent::Process(ProcessEvent::Command(command(
            "pump",
            SignalValue::Bool(true),
        ))))[0],
        Effect::Process(hearthline_engine::ProcessEffect::Output { .. })
    ));
    assert!(matches!(
        io.handle(SimulationEvent::Process(ProcessEvent::Command(command(
            "level",
            SignalValue::Bool(true),
        ))))[0],
        Effect::Process(hearthline_engine::ProcessEffect::Alarm { .. })
    ));
    for authorized in [false, true] {
        assert!(matches!(
            io.handle(SimulationEvent::Process(ProcessEvent::Reset { authorized }))[0],
            Effect::Process(hearthline_engine::ProcessEffect::State { .. })
        ));
    }
    io.handle(SimulationEvent::Process(ProcessEvent::Trip {
        cause: "trip".into(),
    }));
    assert!(io.values().is_empty());
    io.handle(SimulationEvent::SetOperational(false));
    assert!(matches!(
        io.handle(SimulationEvent::Process(ProcessEvent::Tick {
            elapsed_ms: 1
        }))[0],
        Effect::Drop(DropReason::ComponentDown)
    ));
    io.handle(SimulationEvent::SetOperational(true));

    let mut hmi = OperatorInterface::with_kind(
        id("operator"),
        ComponentKind::ScadaWorkstation,
        [network],
        ["pump".into()],
    );
    assert_eq!(hmi.kind(), ComponentKind::ScadaWorkstation);
    assert!(matches!(
        hmi.handle(SimulationEvent::Process(ProcessEvent::Command(command(
            "pump",
            SignalValue::Bool(true),
        ))))[0],
        Effect::Process(hearthline_engine::ProcessEffect::Command(_))
    ));
    assert!(matches!(
        hmi.handle(SimulationEvent::Process(ProcessEvent::Command(command(
            "valve",
            SignalValue::Bool(true),
        ))))[0],
        Effect::Process(hearthline_engine::ProcessEffect::Alarm { .. })
    ));
    assert!(matches!(
        hmi.handle(ingress(
            "network",
            industrial_frame(ApplicationData::None, 4840)
        ))[0],
        Effect::Deliver { .. }
    ));
    assert!(matches!(
        hmi.handle(ingress(
            "network",
            industrial_frame(ApplicationData::None, 80)
        ))[0],
        Effect::Drop(DropReason::PolicyDenied { .. })
    ));
    assert!(matches!(
        hmi.handle(ingress(
            "missing",
            industrial_frame(ApplicationData::None, 4840)
        ))[0],
        Effect::Drop(DropReason::InvalidIngress(_))
    ));
    for authorized in [false, true] {
        assert!(matches!(
            hmi.handle(SimulationEvent::Process(ProcessEvent::Reset { authorized }))[0],
            Effect::Process(hearthline_engine::ProcessEffect::State { .. })
        ));
    }
    assert!(matches!(
        hmi.handle(SimulationEvent::Process(ProcessEvent::Trip {
            cause: "trip".into()
        }))[0],
        Effect::Process(hearthline_engine::ProcessEffect::Alarm { .. })
    ));
    hmi.handle(SimulationEvent::SetOperational(false));
    assert!(matches!(
        hmi.handle(SimulationEvent::Process(ProcessEvent::Tick {
            elapsed_ms: 1
        }))[0],
        Effect::Drop(DropReason::ComponentDown)
    ));
}

#[test]
fn sensors_and_actuators_preserve_quality_calibration_and_safe_state() {
    assert!(
        std::panic::catch_unwind(|| {
            FieldSensor::new(
                id("bad-sensor"),
                "pressure".into(),
                0,
                fixed!(1.0),
                fixed!(0.0),
            )
        })
        .is_err()
    );
    let mut sensor = FieldSensor::with_ports(
        id("pressure-sensor"),
        [PortId::new("io").unwrap()],
        "pressure".into(),
        100,
        fixed!(2.0),
        fixed!(1.0),
    );
    assert!(
        sensor
            .handle(SimulationEvent::Process(ProcessEvent::Tick {
                elapsed_ms: 99
            }))
            .is_empty()
    );
    sensor.handle(SimulationEvent::Process(ProcessEvent::Command(command(
        "pressure",
        SignalValue::Analog(fixed!(3.0)),
    ))));
    let sample = sensor.handle(SimulationEvent::Process(ProcessEvent::Tick {
        elapsed_ms: 1,
    }));
    assert!(
        matches!(&sample[0], Effect::Process(hearthline_engine::ProcessEffect::Signal(value)) if value.value == SignalValue::Analog(fixed!(7.0)) && value.quality_good)
    );
    assert!(matches!(
        sensor.handle(SimulationEvent::Process(ProcessEvent::Command(command(
            "pressure",
            SignalValue::Bool(true),
        ))))[0],
        Effect::Process(hearthline_engine::ProcessEffect::Alarm { .. })
    ));
    assert!(matches!(
        sensor.handle(SimulationEvent::Process(ProcessEvent::Command(command(
            "other",
            SignalValue::Analog(fixed!(1.0)),
        ))))[0],
        Effect::Drop(DropReason::UnsupportedProtocol)
    ));
    sensor.handle(SimulationEvent::Process(ProcessEvent::Trip {
        cause: "bad signal".into(),
    }));
    sensor.handle(SimulationEvent::Process(ProcessEvent::Reset {
        authorized: false,
    }));
    let bad = sensor.handle(SimulationEvent::Process(ProcessEvent::Tick {
        elapsed_ms: 100,
    }));
    assert!(
        matches!(&bad[0], Effect::Process(hearthline_engine::ProcessEffect::Signal(value)) if !value.quality_good)
    );
    sensor.handle(SimulationEvent::Process(ProcessEvent::Reset {
        authorized: true,
    }));
    sensor.handle(SimulationEvent::SetOperational(false));
    let down = sensor.handle(SimulationEvent::Process(ProcessEvent::Tick {
        elapsed_ms: 100,
    }));
    assert!(
        matches!(&down[0], Effect::Process(hearthline_engine::ProcessEffect::Signal(value)) if !value.quality_good)
    );

    let mut actuator = Actuator::with_ports(
        id("pump-actuator"),
        [PortId::new("io").unwrap()],
        "pump".into(),
        SignalValue::Bool(false),
        SignalValue::Bool(false),
    );
    actuator.set_failed(true);
    assert!(matches!(
        actuator.handle(SimulationEvent::Process(ProcessEvent::Command(command(
            "pump",
            SignalValue::Bool(true),
        ))))[0],
        Effect::Drop(DropReason::ComponentDown)
    ));
    actuator.set_failed(false);
    assert!(matches!(
        actuator.handle(SimulationEvent::Process(ProcessEvent::Command(command(
            "pump",
            SignalValue::Bool(true),
        ))))[0],
        Effect::Process(hearthline_engine::ProcessEffect::Output { .. })
    ));
    assert_eq!(actuator.value(), &SignalValue::Bool(true));
    let trip = actuator.handle(SimulationEvent::Process(ProcessEvent::Trip {
        cause: "guard open".into(),
    }));
    assert_eq!(trip.len(), 2);
    assert_eq!(actuator.value(), &SignalValue::Bool(false));
    actuator.handle(SimulationEvent::SetOperational(false));
    assert!(matches!(
        actuator.handle(SimulationEvent::Process(ProcessEvent::Command(command(
            "pump",
            SignalValue::Bool(true),
        ))))[0],
        Effect::Drop(DropReason::ComponentDown)
    ));
    actuator.handle(SimulationEvent::SetOperational(true));
    assert!(matches!(
        actuator.handle(SimulationEvent::Process(ProcessEvent::Command(command(
            "other",
            SignalValue::Bool(true),
        ))))[0],
        Effect::Drop(DropReason::UnsupportedProtocol)
    ));
    assert!(
        actuator
            .handle(SimulationEvent::Process(ProcessEvent::Tick {
                elapsed_ms: 1
            }))
            .is_empty()
    );
}

#[test]
fn robot_cell_arbiter_covers_duplicates_capacity_faults_and_cancellation() {
    let mut arbiter = RobotCellArbiter::default();
    arbiter.set_stage(RobotCellStage::Faulted);
    assert_eq!(arbiter.stage(), RobotCellStage::Idle);
    assert_eq!(arbiter.complete_active(), None);
    assert_eq!(
        arbiter.request(&"x".repeat(65)),
        RobotCellRequestStatus::Invalid
    );
    assert_eq!(
        arbiter.request("mould-active"),
        RobotCellRequestStatus::Granted
    );
    assert_eq!(
        arbiter.request("mould-active"),
        RobotCellRequestStatus::AlreadyPending
    );
    assert_eq!(
        arbiter.request("mould-queued"),
        RobotCellRequestStatus::Queued
    );
    assert_eq!(
        arbiter.request("mould-queued"),
        RobotCellRequestStatus::AlreadyPending
    );
    arbiter.set_stage(RobotCellStage::Faulted);
    arbiter.clear_fault();
    assert_eq!(arbiter.stage(), RobotCellStage::Approach);
    arbiter.cancel("missing");
    arbiter.cancel("mould-queued");
    assert_eq!(arbiter.queue_len(), 0);

    let mut suffix = 0_u32;
    loop {
        let status = arbiter.request(&format!("queued-{suffix}"));
        suffix += 1;
        if status == RobotCellRequestStatus::Full {
            break;
        }
    }
    assert!(arbiter.queue_len() > 0);
    while arbiter.complete_active().is_some() {}
    assert_eq!(arbiter.stage(), RobotCellStage::Idle);
    arbiter.set_stage(RobotCellStage::Faulted);
    arbiter.clear_fault();
    assert_eq!(arbiter.stage(), RobotCellStage::Idle);
    assert!(arbiter.completed() > 0);
}

#[test]
fn body_preparation_physics_cannot_advance_the_iec_owned_phase() {
    let mut process = BodyPreparationProcess::default();
    process.start(true).expect("physical batch initialization");
    let water_charge = BodyPreparationControlState {
        phase: SlipPhase::WaterCharge,
        running: true,
        scan_count: 1,
        batch_count: 0,
    };
    process.apply_control_state(water_charge);

    let tick = process.advance_controlled(BodyPreparationPhysicsInputs {
        elapsed_ms: 10_000,
        control: water_charge,
        automatic_enabled: true,
    });
    assert!(tick.physics.phase_complete);
    assert_eq!(process.phase(), SlipPhase::WaterCharge);
    assert_eq!(process.batch_count(), 0);

    process.apply_control_state(BodyPreparationControlState {
        phase: SlipPhase::DeflocculantCharge,
        scan_count: 2,
        ..water_charge
    });
    assert_eq!(process.phase(), SlipPhase::DeflocculantCharge);
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
            assert!(line.entrained_air_percent > fixed!(3.0));
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
    assert!(retained_line.line_loss_percent > fixed!(20.0));
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
    assert!(header.outlet_flow_l_min > fixed!(0.0));
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

#[test]
fn safety_interface_latches_trips_inhibits_outages_and_ignores_non_motion_events() {
    let mut safety = SafetyInterface::new(id("safety-contract"), ["guard-closed".into()]);
    assert_eq!(
        safety.kind(),
        hearthline_model::ComponentKind::SafetyInterface
    );
    assert_eq!(safety.id().as_str(), "safety-contract");
    assert!(!safety.has_port(&hearthline_model::PortId::new("input").unwrap()));

    assert!(
        safety
            .handle(SimulationEvent::Process(ProcessEvent::Tick {
                elapsed_ms: 1
            }))
            .is_empty()
    );
    assert!(
        safety
            .handle(SimulationEvent::Process(ProcessEvent::Command(
                ProcessCommand {
                    tag: "reset".into(),
                    value: SignalValue::Bool(true),
                    source: "operator".into(),
                }
            )))
            .is_empty()
    );

    let trip = safety.handle(SimulationEvent::Process(ProcessEvent::Trip {
        cause: "gate opened".into(),
    }));
    assert!(matches!(trip[0], Effect::Drop(DropReason::SafetyTrip(_))));
    safety.handle(SimulationEvent::SetOperational(false));
    assert!(safety.trip_latched());
    let denied = safety.handle(SimulationEvent::Process(ProcessEvent::Reset {
        authorized: true,
    }));
    assert!(matches!(denied[0], Effect::Drop(DropReason::SafetyTrip(_))));
    safety.handle(SimulationEvent::SetOperational(true));

    let unsupported = safety.handle(SimulationEvent::FirewallHa(
        FirewallHaControl::HeartbeatTick { at_us: 0 },
    ));
    assert!(matches!(
        unsupported[0],
        Effect::Drop(DropReason::UnsupportedProtocol)
    ));

    safety.handle(SimulationEvent::Process(ProcessEvent::Signal(signal(
        "unrelated",
        SignalValue::Bool(true),
    ))));
    safety.handle(SimulationEvent::Process(ProcessEvent::Signal(
        ProcessSignal {
            tag: "guard-closed".into(),
            value: SignalValue::Bool(true),
            quality_good: false,
            timestamp_ms: 0,
        },
    )));
    assert!(safety.trip_latched());

    safety.handle(SimulationEvent::SetOperational(true));
    safety.handle(SimulationEvent::Process(ProcessEvent::Signal(signal(
        "guard-closed",
        SignalValue::Bool(true),
    ))));
    safety.handle(SimulationEvent::Process(ProcessEvent::Reset {
        authorized: true,
    }));
    let permissive = safety.handle(SimulationEvent::Process(ProcessEvent::Signal(signal(
        "guard-closed",
        SignalValue::Bool(true),
    ))));
    assert!(matches!(
        permissive[0],
        Effect::Process(ProcessEffect::Output {
            value: SignalValue::Bool(true),
            ..
        })
    ));

    let fallback = safety.handle(SimulationEvent::Process(ProcessEvent::Reset {
        authorized: false,
    }));
    assert!(matches!(
        fallback[0],
        Effect::Drop(DropReason::SafetyTrip(_))
    ));

    let non_boolean = safety.handle(SimulationEvent::Process(ProcessEvent::Signal(signal(
        "guard-closed",
        SignalValue::Integer(1),
    ))));
    assert!(matches!(
        non_boolean[0],
        Effect::Process(ProcessEffect::Output {
            value: SignalValue::Bool(false),
            ..
        })
    ));
}

fn forming_process() -> FormingProcess {
    FormingProcess::new(FormingMeasurements {
        slip_tank_level_percent: fixed!(72.0),
        slip_density_g_cm3: fixed!(1.82),
        slip_viscosity_mpa_s: fixed!(1_800.0),
        slip_temperature_c: fixed!(40.0),
        slip_feed_flow_l_min: fixed!(0.0),
        slip_feed_pressure_bar: fixed!(2.5),
        mould_pressure_bar: fixed!(0.0),
        mould_temperature_c: fixed!(25.0),
        fill_head_position_mm: fixed!(0.0),
        mould_position_mm: fixed!(0.0),
        water_flow_l_min: fixed!(0.0),
        excess_slip_drain_flow_l_min: fixed!(0.0),
        mould_moisture_percent: fixed!(8.0),
        compressed_air_pressure_bar: fixed!(6.0),
        vacuum_pressure_kpa: fixed!(0.0),
        robot_position_mm: fixed!(0.0),
        piece_gripped: false,
        piece_moisture_percent: fixed!(20.5),
        predicted_drying_shrinkage_percent: fixed!(2.1),
        drying_energy_factor: fixed!(1.0),
        green_strength_index: fixed!(100.0),
        fired_defect_risk_percent: fixed!(3.0),
    })
}
#[test]
fn forming_cycle_changes_measurements_and_returns_to_idle() {
    let mut process = forming_process();
    process.start(true).expect("cycle start");

    process.tick(750);
    assert_eq!(process.phase(), FormingPhase::Filling);
    assert_eq!(process.outputs().slip, "filling");
    assert_eq!(process.measurements().slip_feed_flow_l_min, fixed!(85.0));
    assert_eq!(process.measurements().fill_head_position_mm, fixed!(400.0));

    process.tick(13_250);
    assert_eq!(process.phase(), FormingPhase::Idle);
    assert!(!process.running());
    assert_eq!(process.cycle_count(), 1);
    assert_eq!(process.outputs().mould, "closed");
    assert_eq!(process.measurements().mould_position_mm, fixed!(0.0));
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
    assert_eq!(process.measurements().vacuum_pressure_kpa, fixed!(-10.0));
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
        pressure_bar: fixed!(7.4),
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
    assert_eq!(process.measurements().mould_pressure_bar, fixed!(7.4));
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
