use hearthline_engine::{
    BodyPreparationProcess, DownstreamMaterialEffects, HandoffPipelineMeasurements,
    PUMP_HEARTBEAT_INTERVAL_MS, PUMP_HEARTBEAT_TIMEOUT_MS, PreparationTrain,
    SIMULATED_MS_PER_PROCESS_MINUTE, WaterNetworkMeasurements, WaterQuality,
    WaterRouteMeasurements,
};

use crate::hmi::schema::{
    GLAZE_PREPARATION_PHASES, RETURN_WATER_PHASES, SLIP_PREPARATION_PHASES,
    WATER_PREPARATION_PHASES,
};
use crate::hmi::{
    HmiBodyIngredientState, HmiBodyPreparationPipelineState, HmiBodyPreparationState,
    HmiBodyQualityCheck, HmiDownstreamMaterialEffects, HmiGlazePreparationState,
    HmiHandoffPipelineState, HmiPreparationTrainState, HmiProcessPhase, HmiReturnWaterState,
    HmiSlipPreparationState, HmiWaterNetworkState, HmiWaterPreparationState, HmiWaterPumpState,
    HmiWaterQuality, HmiWaterRouteState,
};

pub(super) fn build(process: &BodyPreparationProcess) -> Option<HmiBodyPreparationState> {
    let sp = process.setpoints();
    let measured = process.measurements();
    let glaze_fraction = |target: f64| {
        if sp.glaze.dry_mass_kg() <= 0.0 {
            0.0
        } else {
            measured.glaze.powder_mass_kg * target / sp.glaze.dry_mass_kg()
        }
    };
    Some(HmiBodyPreparationState {
        recipe_basis: "public-sanitaryware-engineering-reference",
        simulated_ms_per_process_minute: SIMULATED_MS_PER_PROCESS_MINUTE,
        slip: HmiSlipPreparationState {
            train: train(
                process,
                PreparationTrain::Slip,
                "Slip preparation",
                &SLIP_PREPARATION_PHASES,
            ),
            batch_mass_kg: measured.slip.batch_mass_kg,
            target_batch_mass_kg: sp.slip.total_batch_mass_kg(),
            solids_percent: measured.slip.solids_percent,
            density_kg_l: measured.slip.density_kg_l,
            high_shear_viscosity_mpa_s: measured.slip.high_shear_viscosity_mpa_s,
            low_shear_viscosity_mpa_s: measured.slip.low_shear_viscosity_mpa_s,
            thixotropic_index: measured.slip.thixotropic_index,
            structure_parameter: measured.slip.structure_parameter,
            temperature_c: measured.slip.temperature_c,
            mixer_level_percent: measured.slip.mixer_level_percent,
            conditioning_tank_level_percent: measured.slip.conditioning_tank_level_percent,
            transfer_flow_l_min: measured.slip.transfer_flow_l_min,
            specific_energy_kwh_t: measured.slip.specific_energy_kwh_t,
            residue_44um_percent: measured.slip.residue_44um_percent,
            median_particle_um: measured.slip.median_particle_um,
            casting_rate_g_cm2_min: measured.slip.casting_rate_g_cm2_min,
            quality_index: measured.slip.quality_index,
            quality_released: process.slip_quality_released(),
            ingredients: vec![
                ingredient(
                    "ball-clay",
                    "Ball clay",
                    sp.slip.ball_clay_kg,
                    measured.slip.ball_clay_kg,
                ),
                ingredient(
                    "kaolin",
                    "Kaolin",
                    sp.slip.kaolin_kg,
                    measured.slip.kaolin_kg,
                ),
                ingredient(
                    "feldspar",
                    "Feldspar",
                    sp.slip.feldspar_kg,
                    measured.slip.feldspar_kg,
                ),
                ingredient(
                    "quartz",
                    "Quartz",
                    sp.slip.quartz_kg,
                    measured.slip.quartz_kg,
                ),
                ingredient(
                    "water",
                    "Process water",
                    sp.slip.water_kg,
                    measured.slip.water_kg,
                ),
                ingredient(
                    "sodium-silicate",
                    "Sodium silicate",
                    sp.slip.sodium_silicate_kg,
                    measured.slip.sodium_silicate_kg,
                ),
            ],
            quality_checks: slip_quality(process),
            water: water_quality(measured.slip.water),
            downstream: downstream(process.slip_effects_preview()),
        },
        water: HmiWaterPreparationState {
            train: train(
                process,
                PreparationTrain::Water,
                "Process water",
                &WATER_PREPARATION_PHASES,
            ),
            raw_tank_l: measured.water.raw_tank_l,
            treated_tank_l: measured.water.treated_tank_l,
            feed_flow_l_min: measured.water.feed_flow_l_min,
            permeate_flow_l_min: measured.water.permeate_flow_l_min,
            reject_flow_l_min: measured.water.reject_flow_l_min,
            media_filter_dp_bar: measured.water.media_filter_dp_bar,
            ro_recovery_percent: measured.water.ro_recovery_percent,
            raw: water_quality(measured.water.raw),
            product: water_quality(measured.water.product),
        },
        return_water: HmiReturnWaterState {
            train: train(
                process,
                PreparationTrain::ReturnWater,
                "Return-water recovery",
                &RETURN_WATER_PHASES,
            ),
            active_stream: measured.return_water.active_stream,
            body_equalization_l: measured.return_water.body_equalization_l,
            glaze_equalization_l: measured.return_water.glaze_equalization_l,
            body_reuse_tank_l: measured.return_water.body_reuse_tank_l,
            glaze_reuse_tank_l: measured.return_water.glaze_reuse_tank_l,
            feed_flow_l_min: measured.return_water.feed_flow_l_min,
            clarified_flow_l_min: measured.return_water.clarified_flow_l_min,
            sludge_cake_kg: measured.return_water.sludge_cake_kg,
            influent_turbidity_ntu: measured.return_water.influent_turbidity_ntu,
            effluent_turbidity_ntu: measured.return_water.effluent_turbidity_ntu,
            body_reuse_quality: water_quality(measured.return_water.body_reuse_quality),
            glaze_reuse_quality: water_quality(measured.return_water.glaze_reuse_quality),
        },
        glaze: HmiGlazePreparationState {
            train: train(
                process,
                PreparationTrain::Glaze,
                "Glaze preparation",
                &GLAZE_PREPARATION_PHASES,
            ),
            powder_mass_kg: measured.glaze.powder_mass_kg,
            target_powder_mass_kg: sp.glaze.dry_mass_kg(),
            batch_mass_kg: measured.glaze.batch_mass_kg,
            solids_percent: measured.glaze.solids_percent,
            density_kg_l: measured.glaze.density_kg_l,
            ford_cup_seconds: measured.glaze.ford_cup_seconds,
            median_particle_um: measured.glaze.median_particle_um,
            residue_63um_percent: measured.glaze.residue_63um_percent,
            mill_energy_kwh_t: measured.glaze.mill_energy_kwh_t,
            storage_level_percent: measured.glaze.storage_level_percent,
            transfer_flow_l_min: measured.glaze.transfer_flow_l_min,
            settling_risk_percent: measured.glaze.settling_risk_percent,
            quality_index: measured.glaze.quality_index,
            quality_released: process.glaze_quality_released(),
            ingredients: vec![
                ingredient(
                    "glaze-kaolin",
                    "Kaolin",
                    sp.glaze.kaolin_kg,
                    glaze_fraction(sp.glaze.kaolin_kg),
                ),
                ingredient(
                    "glaze-feldspar",
                    "Sodium feldspar",
                    sp.glaze.sodium_feldspar_kg,
                    glaze_fraction(sp.glaze.sodium_feldspar_kg),
                ),
                ingredient(
                    "glaze-quartz",
                    "Quartz",
                    sp.glaze.quartz_kg,
                    glaze_fraction(sp.glaze.quartz_kg),
                ),
                ingredient(
                    "glaze-calcite",
                    "Calcite",
                    sp.glaze.calcite_kg,
                    glaze_fraction(sp.glaze.calcite_kg),
                ),
                ingredient(
                    "glaze-dolomite",
                    "Dolomite",
                    sp.glaze.dolomite_kg,
                    glaze_fraction(sp.glaze.dolomite_kg),
                ),
                ingredient(
                    "glaze-zinc-oxide",
                    "Zinc oxide",
                    sp.glaze.zinc_oxide_kg,
                    glaze_fraction(sp.glaze.zinc_oxide_kg),
                ),
                ingredient(
                    "glaze-zircon",
                    "Zircon",
                    sp.glaze.zircon_kg,
                    glaze_fraction(sp.glaze.zircon_kg),
                ),
                ingredient(
                    "glaze-water",
                    "Process water",
                    sp.glaze.water_kg,
                    measured.glaze.water_kg,
                ),
                ingredient(
                    "glaze-sodium-silicate",
                    "Sodium silicate",
                    sp.glaze.sodium_silicate_kg,
                    measured.glaze.sodium_silicate_kg,
                ),
            ],
            quality_checks: glaze_quality(process),
            water: water_quality(measured.glaze.water),
        },
        pipelines: HmiBodyPreparationPipelineState {
            water_to_slip: pipeline(measured.pipelines.water_to_slip),
            water_to_glaze: pipeline(measured.pipelines.water_to_glaze),
            slip_to_forming: pipeline(measured.pipelines.slip_to_forming),
            glaze_to_glazing: pipeline(measured.pipelines.glaze_to_glazing),
        },
        water_networks: water_networks(measured.water_networks),
    })
}

fn water_networks(value: WaterNetworkMeasurements) -> HmiWaterNetworkState {
    HmiWaterNetworkState {
        pumps: value
            .pumps
            .iter()
            .map(|pump| HmiWaterPumpState {
                id: pump.id,
                label: pump.label,
                group_id: pump.group_id,
                service: pump.service,
                preferred_duty: pump.preferred_duty,
                commanded: pump.commanded,
                running_feedback: pump.running_feedback,
                heartbeat_sequence: pump.heartbeat_sequence,
                heartbeat_age_ms: pump.heartbeat_age_ms,
                heartbeat_ok: pump.heartbeat_ok,
                maintenance: pump.maintenance.as_str(),
            })
            .collect(),
        routes: value.routes.iter().copied().map(water_route).collect(),
        heartbeat_interval_ms: PUMP_HEARTBEAT_INTERVAL_MS,
        heartbeat_timeout_ms: PUMP_HEARTBEAT_TIMEOUT_MS,
    }
}

fn water_route(value: WaterRouteMeasurements) -> HmiWaterRouteState {
    HmiWaterRouteState {
        id: value.id,
        label: value.label,
        network: value.network,
        source: value.source,
        destination: value.destination,
        pump_group: value.pump_group,
        demanded: value.demanded,
        available: value.available,
        inlet_flow_l_min: value.inlet_flow_l_min,
        outlet_flow_l_min: value.outlet_flow_l_min,
        inlet_pressure_bar: value.inlet_pressure_bar,
        outlet_pressure_bar: value.outlet_pressure_bar,
        leak_detected: value.leak_detected,
        quality: water_quality(value.quality),
    }
}

fn pipeline(value: HandoffPipelineMeasurements) -> HmiHandoffPipelineState {
    HmiHandoffPipelineState {
        inlet_flow_l_min: value.inlet_flow_l_min,
        outlet_flow_l_min: value.outlet_flow_l_min,
        inlet_pressure_bar: value.inlet_pressure_bar,
        outlet_pressure_bar: value.outlet_pressure_bar,
        line_loss_percent: value.line_loss_percent,
        entrained_air_percent: value.entrained_air_percent,
        delivered_quality_percent: value.delivered_quality_percent,
        leak_detected: value.leak_detected,
    }
}

fn train(
    process: &BodyPreparationProcess,
    id: PreparationTrain,
    label: &'static str,
    phases: &'static [HmiProcessPhase],
) -> HmiPreparationTrainState {
    HmiPreparationTrainState {
        id: id.as_str(),
        label,
        running: process.train_running(id),
        held: process.train_held(id),
        phase: process.train_phase(id),
        phase_progress_percent: process.train_progress_percent(id),
        phase_elapsed_process_minutes: process.train_elapsed_ms(id) as f64
            / SIMULATED_MS_PER_PROCESS_MINUTE as f64,
        phase_target_process_minutes: process.train_target_process_minutes(id),
        cycle_count: process.train_cycle_count(id),
        phases,
    }
}

fn ingredient(
    id: &'static str,
    label: &'static str,
    target_kg: f64,
    actual_kg: f64,
) -> HmiBodyIngredientState {
    HmiBodyIngredientState {
        id,
        label,
        target_kg,
        actual_kg,
    }
}

fn quality(
    id: &'static str,
    label: &'static str,
    value: f64,
    unit: &'static str,
    minimum: f64,
    maximum: f64,
) -> HmiBodyQualityCheck {
    HmiBodyQualityCheck {
        id,
        label,
        value,
        unit,
        minimum,
        maximum,
        within_limit: value >= minimum && value <= maximum,
    }
}

fn slip_quality(process: &BodyPreparationProcess) -> Vec<HmiBodyQualityCheck> {
    let m = process.measurements().slip;
    vec![
        quality(
            "density",
            "Slip density",
            m.density_kg_l,
            "kg/L",
            1.78,
            1.84,
        ),
        quality(
            "high-shear-viscosity",
            "High-shear viscosity",
            m.high_shear_viscosity_mpa_s,
            "mPa s",
            400.0,
            850.0,
        ),
        quality(
            "thixotropy",
            "Thixotropic index",
            m.thixotropic_index,
            "ratio",
            4.0,
            7.5,
        ),
        quality(
            "residue-44",
            "Residue on 44 um",
            m.residue_44um_percent,
            "%",
            7.0,
            11.0,
        ),
        quality(
            "conductivity",
            "Charge-water conductivity",
            m.water.conductivity_us_cm,
            "uS/cm",
            0.0,
            350.0,
        ),
    ]
}

fn glaze_quality(process: &BodyPreparationProcess) -> Vec<HmiBodyQualityCheck> {
    let m = process.measurements().glaze;
    vec![
        quality(
            "glaze-density",
            "Glaze density",
            m.density_kg_l,
            "kg/L",
            1.70,
            1.72,
        ),
        quality(
            "ford-cup",
            "Ford-cup flow time",
            m.ford_cup_seconds,
            "s",
            20.0,
            30.0,
        ),
        quality(
            "residue-63",
            "Residue on 63 um",
            m.residue_63um_percent,
            "%",
            0.0,
            2.0,
        ),
    ]
}

fn water_quality(value: WaterQuality) -> HmiWaterQuality {
    HmiWaterQuality {
        temperature_c: value.temperature_c,
        ph: value.ph,
        turbidity_ntu: value.turbidity_ntu,
        conductivity_us_cm: value.conductivity_us_cm,
        hardness_mg_l_caco3: value.hardness_mg_l_caco3,
        suspended_solids_mg_l: value.suspended_solids_mg_l,
        glaze_contamination_percent: value.glaze_contamination_percent,
        recovered_fraction_percent: value.recovered_fraction_percent,
    }
}

fn downstream(value: DownstreamMaterialEffects) -> HmiDownstreamMaterialEffects {
    HmiDownstreamMaterialEffects {
        filling_flow_factor: value.filling_flow_factor,
        casting_rate_g_cm2_min: value.casting_rate_g_cm2_min,
        predicted_green_moisture_percent: value.predicted_green_moisture_percent,
        predicted_drying_shrinkage_percent: value.predicted_drying_shrinkage_percent,
        drying_energy_factor: value.drying_energy_factor,
        green_strength_index: value.green_strength_index,
        fired_defect_risk_percent: value.fired_defect_risk_percent,
    }
}
