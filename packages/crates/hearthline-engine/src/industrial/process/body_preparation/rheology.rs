use super::{DownstreamMaterialEffects, SlipMeasurements, SlipPhase, SlipSetpoints};

pub(super) fn update_slip_physics(m: &mut SlipMeasurements, phase: SlipPhase, sp: &SlipSetpoints) {
    let dry = m.ball_clay_kg + m.kaolin_kg + m.feldspar_kg + m.quartz_kg;
    m.batch_mass_kg = dry + m.water_kg + m.sodium_silicate_kg;
    m.solids_percent = ratio_percent(dry, m.batch_mass_kg);
    m.mixer_level_percent = (m.batch_mass_kg / sp.total_batch_mass_kg() * 82.0).clamp(0.0, 100.0);
    m.conditioning_tank_level_percent = if matches!(
        phase,
        SlipPhase::Conditioning
            | SlipPhase::QualityCheck
            | SlipPhase::TemperatureTrim
            | SlipPhase::Transfer
            | SlipPhase::Complete
    ) {
        76.0
    } else {
        0.0
    };
    if dry <= 0.0 || m.water_kg <= 0.0 {
        return;
    }

    let solid_volume =
        m.ball_clay_kg / 2.58 + m.kaolin_kg / 2.60 + m.feldspar_kg / 2.56 + m.quartz_kg / 2.65;
    let hydration_volume = dry * 0.018;
    let maturity = mixing_maturity(m, sp);
    let air_volume = (solid_volume + m.water_kg) * (0.035 - 0.025 * maturity);
    let total_volume = solid_volume + hydration_volume + m.water_kg + air_volume;
    m.density_kg_l = m.batch_mass_kg / total_volume.max(1.0);
    m.residue_44um_percent = 16.0 - 6.9 * maturity;
    m.median_particle_um = 82.0 - 34.0 * maturity;

    let hydrodynamic_fraction = (solid_volume + hydration_volume) / total_volume;
    let packing_gap = (1.0 - hydrodynamic_fraction / 0.64).clamp(0.08, 0.8);
    let concentration_factor = 1.0 / (packing_gap * packing_gap);
    let clay_fraction = (m.ball_clay_kg + m.kaolin_kg) / dry;
    let shape_factor = 11.5 + clay_fraction * 13.0;
    let dose_percent = m.sodium_silicate_kg / dry * 100.0;
    let dose_error = (dose_percent - 0.20) / 0.20;
    let dispersant_penalty = 1.0 + 1.8 * dose_error * dose_error;
    let ion_penalty = 1.0
        + ((m.water.conductivity_us_cm - 150.0).max(0.0) * 0.0010)
        + ((m.water.hardness_mg_l_caco3 - 30.0).max(0.0) * 0.0025);
    let water_viscosity = (0.89 - (m.temperature_c - 25.0) * 0.015).clamp(0.60, 1.05);
    let dispersion_penalty = 2.1 - 1.1 * maturity;
    m.high_shear_viscosity_mpa_s = (water_viscosity
        * concentration_factor
        * shape_factor
        * dispersant_penalty
        * ion_penalty
        * dispersion_penalty)
        .clamp(80.0, 12_000.0);
    m.thixotropic_index = 1.0 + 5.2 * m.structure_parameter;
    m.low_shear_viscosity_mpa_s = m.high_shear_viscosity_mpa_s * m.thixotropic_index;
    m.casting_rate_g_cm2_min = 0.152
        * (0.75 + 0.25 * m.thixotropic_index / 6.2)
        * (1.0 + (m.solids_percent - 75.0) * 0.012);
    let density_score = window_score(m.density_kg_l, 1.78, 1.84);
    let viscosity_score = window_score(m.high_shear_viscosity_mpa_s, 400.0, 850.0);
    let residue_score = window_score(m.residue_44um_percent, 7.0, 11.0);
    m.quality_index = (density_score + viscosity_score + residue_score) / 3.0 * 100.0;
}

pub(super) fn downstream_effects(
    m: SlipMeasurements,
    _sp: &SlipSetpoints,
) -> DownstreamMaterialEffects {
    let viscosity_factor = (640.0 / m.high_shear_viscosity_mpa_s.max(100.0)).clamp(0.55, 1.35);
    let density_factor = (m.density_kg_l / 1.81).clamp(0.92, 1.08);
    let thix_delta = (m.thixotropic_index - 6.2) / 6.2;
    let moisture = (20.5 - (m.solids_percent - 75.0) * 0.7 + thix_delta * 2.0).clamp(16.0, 27.0);
    let shrinkage =
        (2.1 + (moisture - 20.5) * 0.10 + (m.residue_44um_percent - 9.1) * 0.025).clamp(1.5, 3.5);
    DownstreamMaterialEffects {
        filling_flow_factor: viscosity_factor * density_factor,
        casting_rate_g_cm2_min: m.casting_rate_g_cm2_min,
        predicted_green_moisture_percent: moisture,
        predicted_drying_shrinkage_percent: shrinkage,
        drying_energy_factor: (moisture / 20.5).clamp(0.75, 1.35),
        green_strength_index: (100.0
            - thix_delta.max(0.0) * 22.0
            - (moisture - 20.5).max(0.0) * 2.0)
            .clamp(50.0, 115.0),
        fired_defect_risk_percent: ((100.0 - m.quality_index) * 0.32
            + (shrinkage - 2.1).abs() * 8.0)
            .clamp(1.0, 45.0),
    }
}

fn mixing_maturity(m: &SlipMeasurements, sp: &SlipSetpoints) -> f64 {
    if sp.mixing_energy_kwh_t <= 0.0 {
        0.0
    } else {
        (m.specific_energy_kwh_t / sp.mixing_energy_kwh_t).clamp(0.0, 1.0)
    }
}

fn ratio_percent(value: f64, total: f64) -> f64 {
    if total <= 0.0 {
        0.0
    } else {
        value / total * 100.0
    }
}

fn window_score(value: f64, minimum: f64, maximum: f64) -> f64 {
    let midpoint = (minimum + maximum) / 2.0;
    let half = (maximum - minimum) / 2.0;
    (1.0 - (value - midpoint).abs() / half.max(0.001)).clamp(0.0, 1.0)
}
