use hearthline_engine::{
    BodyPreparationFault, BodyPreparationPhase, BodyPreparationProcess, BodyPreparationStartError,
    CarrierMedium, CeramicSlipBatch, FormingControlState, FormingMeasurements, FormingPhase,
    FormingPhysicsInputs, FormingProcess, MediumKind, PortHardwareKind, PreparationTrain,
    RUNTIME_CAPACITY_BUDGETS, SimulatedMedium, TelephoneMedium, VirtualMedium,
    appliance_supports_port,
};
use hearthline_model::{ComponentKind, FixedValue, Position, Text, fixed};

#[test]
fn router_rejects_telephone_cabling() {
    assert!(!appliance_supports_port(
        ComponentKind::Router,
        PortHardwareKind::TelephoneRj11
    ));
    assert!(appliance_supports_port(
        ComponentKind::VoiceGateway,
        PortHardwareKind::TelephoneRj11
    ));
}

#[test]
fn port_hardware_owns_media_capability() {
    assert!(PortHardwareKind::EthernetRj45.supports(MediumKind::Copper));
    assert!(!PortHardwareKind::EthernetRj45.supports(MediumKind::Telephone));
    assert!(PortHardwareKind::TelephoneRj11.supports(MediumKind::Telephone));
}

#[test]
fn telephone_media_enforces_physical_bounds_and_reports_facts() {
    let medium = TelephoneMedium {
        connector: Text::from("RJ11"),
        pairs: 2,
        length: Position::from_raw(25_000_000),
    };
    medium.validate().expect("valid telephone segment");
    assert!(medium.detail().contains("RJ11 / 2 pair(s) / 25"));
    assert_eq!(medium.physical_facts().len(), 3);
    assert!(medium.propagation_delay_us() > 0);
    assert_eq!(medium.max_capacity_mbps(), Some(1));

    for invalid in [
        TelephoneMedium {
            connector: Text::from(" "),
            ..medium.clone()
        },
        TelephoneMedium {
            pairs: 0,
            ..medium.clone()
        },
        TelephoneMedium {
            pairs: 5,
            ..medium.clone()
        },
        TelephoneMedium {
            length: Position::ZERO,
            ..medium.clone()
        },
        TelephoneMedium {
            length: Position::from_raw(5_000_000_001),
            ..medium
        },
    ] {
        assert!(invalid.validate().is_err());
    }
}

#[test]
fn abstracted_carrier_and_virtual_media_require_named_technologies() {
    for result in [
        CarrierMedium {
            service: Text::from(" "),
        }
        .validate(),
        VirtualMedium {
            technology: Text::from(""),
        }
        .validate(),
    ] {
        assert!(result.is_err());
    }
}

#[test]
fn every_fixed_runtime_capacity_reserves_operational_headroom() {
    assert!(
        RUNTIME_CAPACITY_BUDGETS.len() >= 50,
        "capacity ledger unexpectedly lost coverage"
    );
    for (index, budget) in RUNTIME_CAPACITY_BUDGETS.iter().enumerate() {
        assert!(
            budget.has_required_headroom(),
            "{} has capacity {}, nominal load {}, and insufficient reserve",
            budget.resource,
            budget.capacity,
            budget.nominal_load
        );
        assert!(
            budget.reserve() > 0,
            "{} has no burst reserve",
            budget.resource
        );
        for other in &RUNTIME_CAPACITY_BUDGETS[index + 1..] {
            assert_ne!(
                budget.resource, other.resource,
                "capacity resources must have unique identities"
            );
        }
    }
}

#[test]
fn body_preparation_runs_the_public_reference_mass_balance() {
    let mut process = BodyPreparationProcess::default();
    let setpoints = process.setpoints();

    assert_eq!(setpoints.dry_mass_kg(), fixed!(1_000.0));
    assert!((setpoints.target_solids_percent() - fixed!(75.0)).abs() < fixed!(0.2));
    process.start(true).expect("start batch");

    for _ in 0..100 {
        process.tick(500);
        if process.batch_count() == 1 {
            break;
        }
    }

    assert_eq!(process.batch_count(), 1);
    assert_eq!(process.phase(), BodyPreparationPhase::Idle);
    assert!(!process.running());
    let measurements = process.measurements().slip;
    assert!((measurements.batch_mass_kg - setpoints.total_batch_mass_kg()).abs() < fixed!(0.1));
    assert!((measurements.solids_percent - setpoints.target_solids_percent()).abs() < fixed!(0.1));
    assert!((fixed!(1.78)..=fixed!(1.84)).contains(&measurements.density_kg_l));
    assert!((fixed!(400.0)..=fixed!(850.0)).contains(&measurements.high_shear_viscosity_mpa_s));
    assert!((fixed!(3.0)..=fixed!(8.0)).contains(&measurements.thixotropic_index));
    assert!(measurements.residue_44um_percent <= fixed!(10.0));
    assert!((measurements.temperature_c - fixed!(40.0)).abs() < fixed!(0.1));
    assert!(process.released_slip().is_some());
}

#[test]
fn body_preparation_release_is_invariant_across_scheduler_cadences() {
    let mut expected: Option<CeramicSlipBatch> = None;
    for cadence_ms in [1, 7, 50, 333, 1_000, 5_000] {
        let mut process = BodyPreparationProcess::default();
        process.start(true).expect("start batch");
        for _ in 0..100_000 {
            process.tick(cadence_ms);
            if process.released_slip().is_some() {
                break;
            }
        }
        let batch = process.released_slip().expect("released slip batch");
        if let Some(reference) = &expected {
            assert_eq!(batch.batch_number, reference.batch_number);
            assert!((batch.density_kg_l - reference.density_kg_l).abs() < fixed!(0.001));
            assert!(
                (batch.high_shear_viscosity_mpa_s - reference.high_shear_viscosity_mpa_s).abs()
                    < fixed!(0.001)
            );
            assert!((batch.temperature_c - reference.temperature_c).abs() < fixed!(0.001));
            assert!((batch.solids_percent - reference.solids_percent).abs() < fixed!(0.001));
            assert_eq!(batch.effects, reference.effects);
        } else {
            expected = Some(batch);
        }
    }
}

#[test]
fn body_preparation_idle_preview_uses_the_public_reference_baseline() {
    let process = BodyPreparationProcess::default();
    let effects = process.slip_effects_preview();

    assert_eq!(effects.filling_flow_factor, fixed!(1.0));
    assert_eq!(effects.casting_rate_g_cm2_min, fixed!(0.152));
    assert_eq!(effects.predicted_green_moisture_percent, fixed!(20.5));
    assert_eq!(effects.predicted_drying_shrinkage_percent, fixed!(2.1));
    assert_eq!(effects.drying_energy_factor, fixed!(1.0));
}

#[test]
fn body_preparation_hold_retains_phase_and_safe_outputs() {
    let mut process = BodyPreparationProcess::default();
    process.start(true).expect("start batch");
    process.tick(200);
    let phase = process.phase();
    let elapsed = process.phase_elapsed_ms();
    let water_before_resume = process.measurements();

    assert!(process.hold());
    assert!(process.held());
    assert_eq!(process.outputs().slip_water_valve, "closed");
    process.tick(2_000);
    assert_eq!(process.phase(), phase);
    assert_eq!(process.phase_elapsed_ms(), elapsed);

    process.start(true).expect("resume batch");
    assert!(process.running());
    assert!(!process.held());
    let water_after_resume = process.measurements();
    assert_eq!(
        water_before_resume.water.treated_tank_l,
        water_after_resume.water.treated_tank_l
    );
    assert_eq!(
        water_before_resume.return_water.body_reuse_tank_l,
        water_after_resume.return_water.body_reuse_tank_l
    );
}

#[test]
fn body_preparation_quality_failure_blocks_transfer_until_reset() {
    let mut process = BodyPreparationProcess::default();
    process.start(true).expect("start batch");
    process.set_fault(Some(BodyPreparationFault::QualityOutOfSpec));

    for _ in 0..100 {
        let tick = process.tick(500);
        if let Some(trip) = tick.trip {
            assert_eq!(trip.code(), "BODY-SLIP-QUALITY-RELEASE-DENIED");
            break;
        }
    }

    assert_eq!(process.phase(), BodyPreparationPhase::Faulted);
    assert_eq!(process.outputs().slip_transfer_pump, "stopped");
    process.set_fault(None);
    assert!(process.reset_after_trip(true));
    assert_eq!(process.phase(), BodyPreparationPhase::Idle);
}

#[test]
fn water_return_and_glaze_trains_complete_independently() {
    let mut process = BodyPreparationProcess::default();
    for (train, expected_cycles) in [
        (PreparationTrain::Water, 1),
        (PreparationTrain::ReturnWater, 1),
        (PreparationTrain::ReturnWater, 2),
        (PreparationTrain::Glaze, 1),
    ] {
        process
            .start_train(train, true)
            .expect("start preparation train");
        for _ in 0..100 {
            process.tick(500);
            if process.train_cycle_count(train) == expected_cycles {
                break;
            }
        }
        assert_eq!(
            process.train_cycle_count(train),
            expected_cycles,
            "{train:?} stopped in {}",
            process.train_phase(train)
        );
        assert_eq!(process.train_phase(train), "idle");
    }
    let measurements = process.measurements();
    assert!(measurements.water.treated_tank_l > fixed!(2_500.0));
    assert!(measurements.return_water.body_reuse_tank_l > fixed!(300.0));
    assert!(measurements.return_water.glaze_reuse_tank_l > fixed!(180.0));
    assert!((fixed!(1.70)..=fixed!(1.72)).contains(&measurements.glaze.density_kg_l));
    assert!((fixed!(20.0)..=fixed!(30.0)).contains(&measurements.glaze.ford_cup_seconds));
    assert!(process.released_glaze().is_some());
}

#[test]
fn return_water_cake_mass_does_not_depend_on_tick_size() {
    let mut coarse = BodyPreparationProcess::default();
    let mut fine = BodyPreparationProcess::default();
    coarse
        .start_train(PreparationTrain::ReturnWater, true)
        .expect("start coarse return-water cycle");
    fine.start_train(PreparationTrain::ReturnWater, true)
        .expect("start fine return-water cycle");

    coarse.tick(20_000);
    for _ in 0..200 {
        fine.tick(100);
    }

    assert_eq!(coarse.train_cycle_count(PreparationTrain::ReturnWater), 1);
    assert_eq!(fine.train_cycle_count(PreparationTrain::ReturnWater), 1);
    assert_eq!(
        (coarse.measurements().return_water.sludge_cake_kg
            - fine.measurements().return_water.sludge_cake_kg)
            .abs(),
        FixedValue::ZERO
    );
}

#[test]
fn water_train_refuses_a_batch_without_sufficient_raw_inventory() {
    let mut process = BodyPreparationProcess::default();
    for expected_cycles in 1..=2 {
        process
            .start_train(PreparationTrain::Water, true)
            .expect("start supported water batch");
        process.tick(20_000);
        assert_eq!(
            process.train_cycle_count(PreparationTrain::Water),
            expected_cycles
        );
    }

    assert_eq!(
        process.start_train(PreparationTrain::Water, true),
        Err(BodyPreparationStartError::WaterUnavailable)
    );
}

#[test]
fn body_preparation_rejects_invalid_lifecycle_transitions() {
    let mut process = BodyPreparationProcess::default();
    let setpoints = process.setpoints();

    assert_eq!(
        process.start_train(PreparationTrain::Slip, false),
        Err(BodyPreparationStartError::SafetyNotReady)
    );
    process.set_fault(Some(BodyPreparationFault::IngredientShortage));
    assert_eq!(
        process.start_train(PreparationTrain::Slip, true),
        Err(BodyPreparationStartError::FaultActive)
    );
    assert!(!process.reset_after_trip(true));
    process.set_fault(None);
    assert!(!process.reset_after_trip(false));
    assert!(!process.reset_after_trip(true));

    process.start(true).expect("start slip train");
    assert_eq!(
        process.start(true),
        Err(BodyPreparationStartError::AlreadyRunning)
    );
    assert!(!process.set_setpoints(setpoints));
    assert!(!process.dispatch_water_pump_maintenance("missing-pump"));
    assert!(!process.set_water_pump_failed("missing-pump", true));
}

#[test]
fn every_body_preparation_train_holds_and_resumes_independently() {
    for train in [
        PreparationTrain::Slip,
        PreparationTrain::Water,
        PreparationTrain::ReturnWater,
        PreparationTrain::Glaze,
    ] {
        let mut process = BodyPreparationProcess::default();
        let original_setpoints = process.setpoints();
        assert!(!process.hold_train(train));
        process
            .start_train(train, true)
            .unwrap_or_else(|error| panic!("start {train:?}: {error:?}"));
        assert!(process.train_running(train));
        assert!(!process.set_setpoints(original_setpoints));
        process.tick(100);
        let phase = process.train_phase(train);
        let elapsed = process.train_elapsed_ms(train);
        assert!(process.hold_train(train));
        assert!(process.train_held(train));
        assert!(!process.hold_train(train));
        process.tick(5_000);
        assert_eq!(process.train_phase(train), phase);
        assert_eq!(process.train_elapsed_ms(train), elapsed);
        process
            .start_train(train, true)
            .unwrap_or_else(|error| panic!("resume {train:?}: {error:?}"));
        assert!(process.train_running(train));
        assert!(!process.train_held(train));
    }
}

#[test]
fn duplicate_glaze_start_is_rejected_before_water_is_reserved_again() {
    let mut process = BodyPreparationProcess::default();
    process
        .start_train(PreparationTrain::Glaze, true)
        .expect("start glaze train");
    let before = process.measurements();
    assert_eq!(
        process.start_train(PreparationTrain::Glaze, true),
        Err(BodyPreparationStartError::AlreadyRunning)
    );
    let after = process.measurements();
    assert_eq!(before.water.treated_tank_l, after.water.treated_tank_l);
    assert_eq!(
        before.return_water.glaze_reuse_tank_l,
        after.return_water.glaze_reuse_tank_l
    );
}

#[test]
fn released_slip_updates_forming_and_downstream_quality_indicators() {
    let mut preparation = BodyPreparationProcess::default();
    preparation.start(true).expect("start slip batch");
    for _ in 0..100 {
        preparation.tick(500);
        if preparation.released_slip().is_some() {
            break;
        }
    }
    let batch = preparation.released_slip().expect("released slip batch");
    let mut forming = FormingProcess::new(forming_measurements());

    forming.apply_slip_batch(batch);
    let actual = forming.measurements();
    assert_eq!(actual.slip_density_g_cm3, batch.density_kg_l);
    assert_eq!(
        actual.slip_viscosity_mpa_s,
        batch.high_shear_viscosity_mpa_s
    );
    assert_eq!(actual.slip_temperature_c, batch.temperature_c);
    assert_eq!(
        actual.predicted_drying_shrinkage_percent,
        batch.effects.predicted_drying_shrinkage_percent
    );
    assert_eq!(
        actual.fired_defect_risk_percent,
        batch.effects.fired_defect_risk_percent
    );
}

#[test]
fn iec_control_state_and_rust_physics_exchange_only_typed_contracts() {
    let mut process = FormingProcess::new(forming_measurements());
    let filling = FormingControlState {
        phase: FormingPhase::Filling,
        running: true,
        scan_count: 1,
        cycle_count: 0,
    };
    process
        .start_controlled(true, filling.phase)
        .expect("start");
    process.apply_control_state(filling);
    let mut feedback = process.advance_controlled(FormingPhysicsInputs {
        elapsed_ms: 1_499,
        control: filling,
        robot_pickup_permitted: false,
        robot_delivery_permitted: false,
    });
    assert!(!feedback.phase_complete);
    feedback = process.advance_controlled(FormingPhysicsInputs {
        elapsed_ms: 1,
        control: filling,
        robot_pickup_permitted: false,
        robot_delivery_permitted: false,
    });
    assert!(feedback.phase_complete);
    assert_eq!(process.phase(), FormingPhase::Filling);

    process.apply_control_state(FormingControlState {
        phase: FormingPhase::Pressurizing,
        scan_count: 2,
        ..filling
    });
    assert_eq!(process.phase(), FormingPhase::Pressurizing);
    assert_eq!(process.scan_count(), 2);
}

fn forming_measurements() -> FormingMeasurements {
    FormingMeasurements {
        slip_tank_level_percent: fixed!(72.0),
        slip_density_g_cm3: fixed!(1.70),
        slip_viscosity_mpa_s: fixed!(1_000.0),
        slip_temperature_c: fixed!(25.0),
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
        piece_moisture_percent: fixed!(0.0),
        predicted_drying_shrinkage_percent: fixed!(0.0),
        drying_energy_factor: fixed!(0.0),
        green_strength_index: fixed!(0.0),
        fired_defect_risk_percent: fixed!(0.0),
    }
}
