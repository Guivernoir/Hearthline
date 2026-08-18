use hearthline_engine::{
    BodyPreparationMeasurements, BodyPreparationProcess, PreparationTrain, SlipPhase,
};

use super::super::HmiSession;

pub(super) fn sync(session: &mut HmiSession) {
    let Some((signals, outputs, timestamp)) = session.body_preparation.as_ref().map(|process| {
        (
            signal_values(process.measurements()),
            output_states(process),
            process
                .scan_count()
                .saturating_mul(BodyPreparationProcess::SCAN_INTERVAL_MS),
        )
    }) else {
        return;
    };
    for (tag, value) in signals {
        session.set_signal(tag, value, timestamp);
    }
    for (tag, state) in outputs {
        session.set_actuator(tag, state);
    }
}

fn signal_values(m: BodyPreparationMeasurements) -> Vec<(&'static str, f64)> {
    let route = |id| {
        m.water_networks
            .routes
            .iter()
            .find(|route| route.id == id)
            .copied()
            .expect("configured water route exists")
    };
    let industrial_header = route("industrial-header");
    let industrial_slip = route("industrial-slip");
    let industrial_glaze = route("industrial-glaze");
    let industrial_forming = route("industrial-forming");
    let return_body_collection = route("return-body-collection");
    let return_glaze_collection = route("return-glaze-collection");
    let return_body_reuse = route("return-body-reuse");
    let return_glaze_reuse = route("return-glaze-reuse");
    let active_return_quality = if m.return_water.active_stream == "body-return" {
        m.return_water.body_reuse_quality
    } else {
        m.return_water.glaze_reuse_quality
    };
    vec![
        ("area-01-wit-01", m.slip.batch_mass_kg),
        (
            "area-01-ft-02",
            if m.slip.water_kg > 0.0 { 80.0 } else { 0.0 },
        ),
        ("area-01-lt-02", m.slip.mixer_level_percent),
        ("area-01-tt-01", m.slip.temperature_c),
        ("area-01-pwt-01", m.slip.specific_energy_kwh_t),
        ("area-01-dt-01", m.slip.density_kg_l),
        ("area-01-vis-01", m.slip.high_shear_viscosity_mpa_s),
        ("area-01-vis-02", m.slip.low_shear_viscosity_mpa_s),
        ("area-01-thix-01", m.slip.thixotropic_index),
        ("area-01-psa-01", m.slip.median_particle_um),
        ("area-01-res-01", m.slip.residue_44um_percent),
        ("area-01-cr-01", m.slip.casting_rate_g_cm2_min),
        ("area-01-tt-02", m.slip.temperature_c),
        ("area-01-lt-01", m.slip.conditioning_tank_level_percent),
        ("area-01-ft-01", m.slip.transfer_flow_l_min),
        ("area-01-wt-lt-01", m.water.raw_tank_l),
        ("area-01-wt-tur-01", m.water.raw.turbidity_ntu),
        ("area-01-wt-cnd-01", m.water.raw.conductivity_us_cm),
        ("area-01-wt-hard-01", m.water.raw.hardness_mg_l_caco3),
        ("area-01-wt-ph-01", m.water.raw.ph),
        ("area-01-wt-tt-01", m.water.raw.temperature_c),
        ("area-01-wt-dpit-01", m.water.media_filter_dp_bar),
        ("area-01-wt-ft-01", m.water.feed_flow_l_min),
        ("area-01-wt-ft-02", m.water.permeate_flow_l_min),
        ("area-01-wt-cnd-02", m.water.product.conductivity_us_cm),
        ("area-01-wt-hard-02", m.water.product.hardness_mg_l_caco3),
        ("area-01-wt-tur-02", m.water.product.turbidity_ntu),
        ("area-01-wt-ph-02", m.water.product.ph),
        ("area-01-wt-tt-02", m.water.product.temperature_c),
        ("area-01-wt-lt-02", m.water.treated_tank_l),
        ("area-01-rw-lt-01", m.return_water.body_equalization_l),
        ("area-01-rw-lt-02", m.return_water.glaze_equalization_l),
        ("area-01-rw-tur-01", m.return_water.influent_turbidity_ntu),
        ("area-01-rw-tur-02", m.return_water.effluent_turbidity_ntu),
        ("area-01-rw-ph-01", active_return_quality.ph + 0.2),
        (
            "area-01-rw-cnd-01",
            active_return_quality.conductivity_us_cm + 600.0,
        ),
        (
            "area-01-rw-tt-01",
            active_return_quality.temperature_c + 1.0,
        ),
        ("area-01-rw-ph-02", active_return_quality.ph),
        (
            "area-01-rw-cnd-02",
            active_return_quality.conductivity_us_cm,
        ),
        ("area-01-rw-tt-02", active_return_quality.temperature_c),
        ("area-01-rw-ft-01", m.return_water.clarified_flow_l_min),
        ("area-01-rw-wit-01", m.return_water.sludge_cake_kg),
        ("area-01-rw-lt-03", m.return_water.body_reuse_tank_l),
        ("area-01-rw-lt-04", m.return_water.glaze_reuse_tank_l),
        ("area-01-gl-wit-01", m.glaze.powder_mass_kg),
        ("area-01-gl-dt-01", m.glaze.density_kg_l),
        ("area-01-gl-fc-01", m.glaze.ford_cup_seconds),
        ("area-01-gl-psa-01", m.glaze.median_particle_um),
        ("area-01-gl-res-01", m.glaze.residue_63um_percent),
        ("area-01-gl-lt-01", m.glaze.storage_level_percent),
        ("area-01-gl-ft-01", m.glaze.transfer_flow_l_min),
        ("area-01-ws-ft-01", industrial_slip.outlet_flow_l_min),
        ("area-01-ws-pit-01", industrial_slip.outlet_pressure_bar),
        ("area-01-ws-ld-01", route_loss_percent(industrial_slip)),
        ("area-01-wg-ft-01", industrial_glaze.outlet_flow_l_min),
        ("area-01-wg-pit-01", industrial_glaze.outlet_pressure_bar),
        ("area-01-wg-ld-01", route_loss_percent(industrial_glaze)),
        ("area-01-wd-ph-01", industrial_header.quality.ph),
        (
            "area-01-wd-cnd-01",
            industrial_header.quality.conductivity_us_cm,
        ),
        ("area-01-wd-tur-01", industrial_header.quality.turbidity_ntu),
        ("area-01-wd-tt-01", industrial_header.quality.temperature_c),
        ("area-01-wf-pit-01", industrial_forming.outlet_pressure_bar),
        ("area-01-wf-ft-01", industrial_forming.outlet_flow_l_min),
        ("area-01-wf-ld-01", route_loss_percent(industrial_forming)),
        (
            "area-01-rb-pit-01",
            return_body_collection.outlet_pressure_bar,
        ),
        ("area-01-rb-ft-01", return_body_collection.outlet_flow_l_min),
        (
            "area-01-rb-ld-01",
            route_loss_percent(return_body_collection),
        ),
        (
            "area-01-rg-pit-01",
            return_glaze_collection.outlet_pressure_bar,
        ),
        (
            "area-01-rg-ft-01",
            return_glaze_collection.outlet_flow_l_min,
        ),
        (
            "area-01-rg-ld-01",
            route_loss_percent(return_glaze_collection),
        ),
        ("area-01-rbd-pit-01", return_body_reuse.outlet_pressure_bar),
        ("area-01-rbd-ft-01", return_body_reuse.outlet_flow_l_min),
        ("area-01-rbd-ld-01", route_loss_percent(return_body_reuse)),
        ("area-01-rgd-pit-01", return_glaze_reuse.outlet_pressure_bar),
        ("area-01-rgd-ft-01", return_glaze_reuse.outlet_flow_l_min),
        ("area-01-rgd-ld-01", route_loss_percent(return_glaze_reuse)),
        (
            "area-01-slip-pit-01",
            m.pipelines.slip_to_forming.inlet_pressure_bar,
        ),
        (
            "area-01-slip-pit-02",
            m.pipelines.slip_to_forming.outlet_pressure_bar,
        ),
        (
            "area-01-slip-ft-02",
            m.pipelines.slip_to_forming.outlet_flow_l_min,
        ),
        (
            "area-01-slip-ae-01",
            m.pipelines.slip_to_forming.entrained_air_percent,
        ),
        (
            "area-01-slip-ld-01",
            m.pipelines.slip_to_forming.line_loss_percent,
        ),
        (
            "area-01-gl-pit-01",
            m.pipelines.glaze_to_glazing.inlet_pressure_bar,
        ),
        (
            "area-01-gl-pit-02",
            m.pipelines.glaze_to_glazing.outlet_pressure_bar,
        ),
        (
            "area-01-gl-ft-02",
            m.pipelines.glaze_to_glazing.outlet_flow_l_min,
        ),
        (
            "area-01-gl-ld-01",
            m.pipelines.glaze_to_glazing.line_loss_percent,
        ),
    ]
}

fn route_loss_percent(route: hearthline_engine::WaterRouteMeasurements) -> f64 {
    if route.inlet_flow_l_min <= f64::EPSILON {
        0.0
    } else {
        ((route.inlet_flow_l_min - route.outlet_flow_l_min) / route.inlet_flow_l_min * 100.0)
            .clamp(0.0, 100.0)
    }
}

fn output_states(process: &BodyPreparationProcess) -> Vec<(&'static str, &'static str)> {
    let out = process.outputs();
    let phase = process.phase();
    let water_phase = process.train_phase(PreparationTrain::Water);
    let glaze_phase = process.train_phase(PreparationTrain::Glaze);
    let mut states = vec![
        ("area-01-xv-02-command", out.slip_water_valve),
        (
            "area-01-dp-01-command",
            if phase == SlipPhase::DeflocculantCharge {
                "dosing"
            } else {
                "stopped"
            },
        ),
        (
            "area-01-feed-01-command",
            if phase == SlipPhase::BallClayCharge {
                "feeding"
            } else {
                "stopped"
            },
        ),
        (
            "area-01-feed-02-command",
            if phase == SlipPhase::KaolinCharge {
                "feeding"
            } else {
                "stopped"
            },
        ),
        (
            "area-01-feed-03-command",
            if phase == SlipPhase::FeldsparCharge {
                "feeding"
            } else {
                "stopped"
            },
        ),
        (
            "area-01-feed-04-command",
            if phase == SlipPhase::QuartzCharge {
                "feeding"
            } else {
                "stopped"
            },
        ),
        ("area-01-ag-01-command", out.slip_blunger),
        ("area-01-scr-01-command", out.slip_screen),
        (
            "area-01-mag-01-command",
            if !process.train_running(PreparationTrain::Slip) {
                "isolated"
            } else if phase == SlipPhase::MagneticSeparation {
                "separating"
            } else {
                "energized"
            },
        ),
        (
            "area-01-ag-02-command",
            if phase == SlipPhase::Conditioning {
                "agitating"
            } else {
                "stopped"
            },
        ),
        (
            "area-01-ht-01-command",
            if phase == SlipPhase::TemperatureTrim {
                "heating"
            } else {
                "off"
            },
        ),
        (
            "area-01-xv-01-command",
            if phase == SlipPhase::Transfer {
                "open"
            } else {
                "closed"
            },
        ),
        ("area-01-pmp-01-command", out.slip_transfer_pump),
        ("area-01-wt-pmp-01-command", out.raw_water_pump),
        ("area-01-wt-fil-01-command", out.media_filter),
        (
            "area-01-wt-carb-01-command",
            if water_phase == "activated-carbon" {
                "service"
            } else {
                "isolated"
            },
        ),
        ("area-01-wt-soft-01-command", out.softener),
        ("area-01-wt-ro-01-command", out.reverse_osmosis),
        ("area-01-rw-ag-01-command", out.return_equalization),
        ("area-01-rw-dp-01-command", out.flocculant_pump),
        ("area-01-rw-clar-01-command", out.clarifier),
        ("area-01-rw-fp-01-command", out.filter_press),
        ("area-01-rw-xv-01-command", out.reuse_diverter),
        ("area-01-gl-mill-01-command", out.glaze_mill),
        ("area-01-gl-scr-01-command", out.glaze_screen),
        ("area-01-gl-ag-01-command", out.glaze_agitator),
        ("area-01-gl-pmp-01-command", out.glaze_transfer_pump),
        (
            "area-01-gl-xv-01-command",
            if glaze_phase == "glaze-water-charge" {
                "open"
            } else {
                "closed"
            },
        ),
        (
            "area-01-gl-dp-01-command",
            if glaze_phase == "glaze-dispersant-charge" {
                "dosing"
            } else {
                "stopped"
            },
        ),
        (
            "area-01-gl-mag-01-command",
            if glaze_phase == "glaze-magnetic-separation" {
                "energized"
            } else {
                "isolated"
            },
        ),
    ];
    for tag in [
        "area-01-gl-feed-01-command",
        "area-01-gl-feed-02-command",
        "area-01-gl-feed-03-command",
        "area-01-gl-feed-04-command",
        "area-01-gl-feed-05-command",
        "area-01-gl-feed-06-command",
        "area-01-gl-feed-07-command",
    ] {
        states.push((
            tag,
            if glaze_phase == "seven-powder-weighing" {
                "feeding"
            } else {
                "stopped"
            },
        ));
    }
    for (tag, id) in [
        ("area-01-wd-pmp-01a-command", "area-01-wd-pmp-01a"),
        ("area-01-wd-pmp-01b-command", "area-01-wd-pmp-01b"),
        ("area-01-wd-pmp-02a-command", "area-01-wd-pmp-02a"),
        ("area-01-wd-pmp-02b-command", "area-01-wd-pmp-02b"),
        ("area-01-wd-pmp-03a-command", "area-01-wd-pmp-03a"),
        ("area-01-wd-pmp-03b-command", "area-01-wd-pmp-03b"),
        ("area-01-wd-pmp-04a-command", "area-01-wd-pmp-04a"),
        ("area-01-wd-pmp-04b-command", "area-01-wd-pmp-04b"),
        ("area-01-rc-pmp-01a-command", "area-01-rc-pmp-01a"),
        ("area-01-rc-pmp-01b-command", "area-01-rc-pmp-01b"),
        ("area-01-rc-pmp-02a-command", "area-01-rc-pmp-02a"),
        ("area-01-rc-pmp-02b-command", "area-01-rc-pmp-02b"),
        ("area-01-rc-pmp-03a-command", "area-01-rc-pmp-03a"),
        ("area-01-rc-pmp-03b-command", "area-01-rc-pmp-03b"),
        ("area-01-rc-pmp-04a-command", "area-01-rc-pmp-04a"),
        ("area-01-rc-pmp-04b-command", "area-01-rc-pmp-04b"),
    ] {
        let running = process
            .measurements()
            .water_networks
            .pumps
            .iter()
            .find(|pump| pump.id == id)
            .is_some_and(|pump| pump.running_feedback);
        states.push((tag, if running { "running" } else { "stopped" }));
    }
    states
}
