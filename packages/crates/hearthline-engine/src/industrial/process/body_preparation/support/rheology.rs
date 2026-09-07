use hearthline_model::{FixedValue, fixed};

use super::{DownstreamMaterialEffects, SlipMeasurements, SlipPhase, SlipSetpoints};

pub(super) fn update_slip_physics(m: &mut SlipMeasurements, phase: SlipPhase, sp: &SlipSetpoints) {
    let dry = m.ball_clay_kg + m.kaolin_kg + m.feldspar_kg + m.quartz_kg;
    m.batch_mass_kg = dry + m.water_kg + m.sodium_silicate_kg;
    m.solids_percent = ratio_percent(dry, m.batch_mass_kg);
    m.mixer_level_percent = (m.batch_mass_kg / sp.total_batch_mass_kg() * fixed!(82.0))
        .clamp(fixed!(0.0), fixed!(100.0));
    m.conditioning_tank_level_percent = if matches!(
        phase,
        SlipPhase::Conditioning
            | SlipPhase::QualityCheck
            | SlipPhase::TemperatureTrim
            | SlipPhase::Transfer
            | SlipPhase::Complete
    ) {
        fixed!(76.0)
    } else {
        fixed!(0.0)
    };
    if dry <= fixed!(0.0) || m.water_kg <= fixed!(0.0) {
        return;
    }

    let solid_volume = m.ball_clay_kg / fixed!(2.58)
        + m.kaolin_kg / fixed!(2.60)
        + m.feldspar_kg / fixed!(2.56)
        + m.quartz_kg / fixed!(2.65);
    let hydration_volume = dry * fixed!(0.018);
    let maturity = mixing_maturity(m, sp);
    let air_volume = (solid_volume + m.water_kg) * (fixed!(0.035) - fixed!(0.025) * maturity);
    let total_volume = solid_volume + hydration_volume + m.water_kg + air_volume;
    m.density_kg_l = m.batch_mass_kg / total_volume.max(fixed!(1.0));
    m.residue_44um_percent = fixed!(16.0) - fixed!(6.9) * maturity;
    m.median_particle_um = fixed!(82.0) - fixed!(34.0) * maturity;

    let hydrodynamic_fraction = (solid_volume + hydration_volume) / total_volume;
    let packing_gap =
        (fixed!(1.0) - hydrodynamic_fraction / fixed!(0.64)).clamp(fixed!(0.08), fixed!(0.8));
    let concentration_factor = fixed!(1.0) / (packing_gap * packing_gap);
    let clay_fraction = (m.ball_clay_kg + m.kaolin_kg) / dry;
    let shape_factor = fixed!(11.5) + clay_fraction * fixed!(13.0);
    let dose_percent = m.sodium_silicate_kg / dry * fixed!(100.0);
    let dose_error = (dose_percent - fixed!(0.20)) / fixed!(0.20);
    let dispersant_penalty = fixed!(1.0) + fixed!(1.8) * dose_error * dose_error;
    let ion_penalty = fixed!(1.0)
        + ((m.water.conductivity_us_cm - fixed!(150.0)).max(fixed!(0.0)) * fixed!(0.0010))
        + ((m.water.hardness_mg_l_caco3 - fixed!(30.0)).max(fixed!(0.0)) * fixed!(0.0025));
    let water_viscosity = (fixed!(0.89) - (m.temperature_c - fixed!(25.0)) * fixed!(0.015))
        .clamp(fixed!(0.60), fixed!(1.05));
    let dispersion_penalty = fixed!(2.1) - fixed!(1.1) * maturity;
    m.high_shear_viscosity_mpa_s = (water_viscosity
        * concentration_factor
        * shape_factor
        * dispersant_penalty
        * ion_penalty
        * dispersion_penalty)
        .clamp(fixed!(80.0), fixed!(12_000.0));
    m.thixotropic_index = fixed!(1.0) + fixed!(5.2) * m.structure_parameter;
    m.low_shear_viscosity_mpa_s = m.high_shear_viscosity_mpa_s * m.thixotropic_index;
    m.casting_rate_g_cm2_min = fixed!(0.152)
        * (fixed!(0.75) + fixed!(0.25) * m.thixotropic_index / fixed!(6.2))
        * (fixed!(1.0) + (m.solids_percent - fixed!(75.0)) * fixed!(0.012));
    let density_score = window_score(m.density_kg_l, fixed!(1.78), fixed!(1.84));
    let viscosity_score = window_score(m.high_shear_viscosity_mpa_s, fixed!(400.0), fixed!(850.0));
    let residue_score = window_score(m.residue_44um_percent, fixed!(7.0), fixed!(11.0));
    m.quality_index =
        (density_score + viscosity_score + residue_score) / fixed!(3.0) * fixed!(100.0);
}

pub(super) fn downstream_effects(
    m: SlipMeasurements,
    _sp: &SlipSetpoints,
) -> DownstreamMaterialEffects {
    let viscosity_factor = (fixed!(640.0) / m.high_shear_viscosity_mpa_s.max(fixed!(100.0)))
        .clamp(fixed!(0.55), fixed!(1.35));
    let density_factor = (m.density_kg_l / fixed!(1.81)).clamp(fixed!(0.92), fixed!(1.08));
    let thix_delta = (m.thixotropic_index - fixed!(6.2)) / fixed!(6.2);
    let moisture = (fixed!(20.5) - (m.solids_percent - fixed!(75.0)) * fixed!(0.7)
        + thix_delta * fixed!(2.0))
    .clamp(fixed!(16.0), fixed!(27.0));
    let shrinkage = (fixed!(2.1)
        + (moisture - fixed!(20.5)) * fixed!(0.10)
        + (m.residue_44um_percent - fixed!(9.1)) * fixed!(0.025))
    .clamp(fixed!(1.5), fixed!(3.5));
    DownstreamMaterialEffects {
        filling_flow_factor: viscosity_factor * density_factor,
        casting_rate_g_cm2_min: m.casting_rate_g_cm2_min,
        predicted_green_moisture_percent: moisture,
        predicted_drying_shrinkage_percent: shrinkage,
        drying_energy_factor: (moisture / fixed!(20.5)).clamp(fixed!(0.75), fixed!(1.35)),
        green_strength_index: (fixed!(100.0)
            - thix_delta.max(fixed!(0.0)) * fixed!(22.0)
            - (moisture - fixed!(20.5)).max(fixed!(0.0)) * fixed!(2.0))
        .clamp(fixed!(50.0), fixed!(115.0)),
        fired_defect_risk_percent: ((fixed!(100.0) - m.quality_index) * fixed!(0.32)
            + (shrinkage - fixed!(2.1)).abs() * fixed!(8.0))
        .clamp(fixed!(1.0), fixed!(45.0)),
    }
}

fn mixing_maturity(m: &SlipMeasurements, sp: &SlipSetpoints) -> FixedValue {
    if sp.mixing_energy_kwh_t <= fixed!(0.0) {
        fixed!(0.0)
    } else {
        (m.specific_energy_kwh_t / sp.mixing_energy_kwh_t).clamp(fixed!(0.0), fixed!(1.0))
    }
}

fn ratio_percent(value: FixedValue, total: FixedValue) -> FixedValue {
    if total <= fixed!(0.0) {
        fixed!(0.0)
    } else {
        value / total * fixed!(100.0)
    }
}

fn window_score(value: FixedValue, minimum: FixedValue, maximum: FixedValue) -> FixedValue {
    let midpoint = (minimum + maximum) / fixed!(2.0);
    let half = (maximum - minimum) / fixed!(2.0);
    (fixed!(1.0) - (value - midpoint).abs() / half.max(fixed!(0.001)))
        .clamp(fixed!(0.0), fixed!(1.0))
}
