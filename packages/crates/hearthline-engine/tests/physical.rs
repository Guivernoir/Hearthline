use hearthline_engine::{
    BodyPreparationFault, BodyPreparationPhase, BodyPreparationProcess, BodyPreparationStartError,
    FormingMeasurements, FormingProcess, MediumKind, PortHardwareKind, PreparationTrain,
    appliance_supports_port,
};
use hearthline_model::ComponentKind;

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
fn body_preparation_runs_the_public_reference_mass_balance() {
    let mut process = BodyPreparationProcess::default();
    let setpoints = process.setpoints();

    assert_eq!(setpoints.dry_mass_kg(), 1_000.0);
    assert!((setpoints.target_solids_percent() - 75.0).abs() < 0.2);
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
    assert!((measurements.batch_mass_kg - setpoints.total_batch_mass_kg()).abs() < 0.1);
    assert!((measurements.solids_percent - setpoints.target_solids_percent()).abs() < 0.1);
    assert!((1.78..=1.84).contains(&measurements.density_kg_l));
    assert!((400.0..=850.0).contains(&measurements.high_shear_viscosity_mpa_s));
    assert!((3.0..=8.0).contains(&measurements.thixotropic_index));
    assert!(measurements.residue_44um_percent <= 10.0);
    assert!((measurements.temperature_c - 40.0).abs() < 0.1);
    assert!(process.released_slip().is_some());
}

#[test]
fn body_preparation_idle_preview_uses_the_public_reference_baseline() {
    let process = BodyPreparationProcess::default();
    let effects = process.slip_effects_preview();

    assert_eq!(effects.filling_flow_factor, 1.0);
    assert_eq!(effects.casting_rate_g_cm2_min, 0.152);
    assert_eq!(effects.predicted_green_moisture_percent, 20.5);
    assert_eq!(effects.predicted_drying_shrinkage_percent, 2.1);
    assert_eq!(effects.drying_energy_factor, 1.0);
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
    assert!(measurements.water.treated_tank_l > 2_500.0);
    assert!(measurements.return_water.body_reuse_tank_l > 300.0);
    assert!(measurements.return_water.glaze_reuse_tank_l > 180.0);
    assert!((1.70..=1.72).contains(&measurements.glaze.density_kg_l));
    assert!((20.0..=30.0).contains(&measurements.glaze.ford_cup_seconds));
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
    assert!(
        (coarse.measurements().return_water.sludge_cake_kg
            - fine.measurements().return_water.sludge_cake_kg)
            .abs()
            < f64::EPSILON
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
    let mut forming = FormingProcess::new(FormingMeasurements {
        slip_tank_level_percent: 72.0,
        slip_density_g_cm3: 1.70,
        slip_viscosity_mpa_s: 1_000.0,
        slip_temperature_c: 25.0,
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
        piece_moisture_percent: 0.0,
        predicted_drying_shrinkage_percent: 0.0,
        drying_energy_factor: 0.0,
        green_strength_index: 0.0,
        fired_defect_risk_percent: 0.0,
    });

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
