use serde::Serialize;

use super::HmiProcessPhase;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiBodyPreparationState {
    pub recipe_basis: &'static str,
    pub simulated_ms_per_process_minute: u64,
    pub slip: HmiSlipPreparationState,
    pub water: HmiWaterPreparationState,
    pub return_water: HmiReturnWaterState,
    pub glaze: HmiGlazePreparationState,
    pub pipelines: HmiBodyPreparationPipelineState,
    pub water_networks: HmiWaterNetworkState,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiWaterNetworkState {
    pub pumps: Vec<HmiWaterPumpState>,
    pub routes: Vec<HmiWaterRouteState>,
    pub heartbeat_interval_ms: u64,
    pub heartbeat_timeout_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiWaterPumpState {
    pub id: &'static str,
    pub label: &'static str,
    pub group_id: &'static str,
    pub service: &'static str,
    pub preferred_duty: bool,
    pub commanded: bool,
    pub running_feedback: bool,
    pub heartbeat_sequence: u64,
    pub heartbeat_age_ms: u64,
    pub heartbeat_ok: bool,
    pub maintenance: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiWaterRouteState {
    pub id: &'static str,
    pub label: &'static str,
    pub network: &'static str,
    pub source: &'static str,
    pub destination: &'static str,
    pub pump_group: &'static str,
    pub demanded: bool,
    pub available: bool,
    pub inlet_flow_l_min: f64,
    pub outlet_flow_l_min: f64,
    pub inlet_pressure_bar: f64,
    pub outlet_pressure_bar: f64,
    pub leak_detected: bool,
    pub quality: HmiWaterQuality,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiBodyPreparationPipelineState {
    pub water_to_slip: HmiHandoffPipelineState,
    pub water_to_glaze: HmiHandoffPipelineState,
    pub slip_to_forming: HmiHandoffPipelineState,
    pub glaze_to_glazing: HmiHandoffPipelineState,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiHandoffPipelineState {
    pub inlet_flow_l_min: f64,
    pub outlet_flow_l_min: f64,
    pub inlet_pressure_bar: f64,
    pub outlet_pressure_bar: f64,
    pub line_loss_percent: f64,
    pub entrained_air_percent: f64,
    pub delivered_quality_percent: f64,
    pub leak_detected: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiPreparationTrainState {
    pub id: &'static str,
    pub label: &'static str,
    pub running: bool,
    pub held: bool,
    pub phase: &'static str,
    pub phase_progress_percent: f64,
    pub phase_elapsed_process_minutes: f64,
    pub phase_target_process_minutes: f64,
    pub cycle_count: u64,
    pub phases: &'static [HmiProcessPhase],
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiSlipPreparationState {
    pub train: HmiPreparationTrainState,
    pub batch_mass_kg: f64,
    pub target_batch_mass_kg: f64,
    pub solids_percent: f64,
    pub density_kg_l: f64,
    pub high_shear_viscosity_mpa_s: f64,
    pub low_shear_viscosity_mpa_s: f64,
    pub thixotropic_index: f64,
    pub structure_parameter: f64,
    pub temperature_c: f64,
    pub mixer_level_percent: f64,
    pub conditioning_tank_level_percent: f64,
    pub transfer_flow_l_min: f64,
    pub specific_energy_kwh_t: f64,
    pub residue_44um_percent: f64,
    pub median_particle_um: f64,
    pub casting_rate_g_cm2_min: f64,
    pub quality_index: f64,
    pub quality_released: bool,
    pub ingredients: Vec<HmiBodyIngredientState>,
    pub quality_checks: Vec<HmiBodyQualityCheck>,
    pub water: HmiWaterQuality,
    pub downstream: HmiDownstreamMaterialEffects,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiWaterPreparationState {
    pub train: HmiPreparationTrainState,
    pub raw_tank_l: f64,
    pub treated_tank_l: f64,
    pub feed_flow_l_min: f64,
    pub permeate_flow_l_min: f64,
    pub reject_flow_l_min: f64,
    pub media_filter_dp_bar: f64,
    pub ro_recovery_percent: f64,
    pub raw: HmiWaterQuality,
    pub product: HmiWaterQuality,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiReturnWaterState {
    pub train: HmiPreparationTrainState,
    pub active_stream: &'static str,
    pub body_equalization_l: f64,
    pub glaze_equalization_l: f64,
    pub body_reuse_tank_l: f64,
    pub glaze_reuse_tank_l: f64,
    pub feed_flow_l_min: f64,
    pub clarified_flow_l_min: f64,
    pub sludge_cake_kg: f64,
    pub influent_turbidity_ntu: f64,
    pub effluent_turbidity_ntu: f64,
    pub body_reuse_quality: HmiWaterQuality,
    pub glaze_reuse_quality: HmiWaterQuality,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiGlazePreparationState {
    pub train: HmiPreparationTrainState,
    pub powder_mass_kg: f64,
    pub target_powder_mass_kg: f64,
    pub batch_mass_kg: f64,
    pub solids_percent: f64,
    pub density_kg_l: f64,
    pub ford_cup_seconds: f64,
    pub median_particle_um: f64,
    pub residue_63um_percent: f64,
    pub mill_energy_kwh_t: f64,
    pub storage_level_percent: f64,
    pub transfer_flow_l_min: f64,
    pub settling_risk_percent: f64,
    pub quality_index: f64,
    pub quality_released: bool,
    pub ingredients: Vec<HmiBodyIngredientState>,
    pub quality_checks: Vec<HmiBodyQualityCheck>,
    pub water: HmiWaterQuality,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiWaterQuality {
    pub temperature_c: f64,
    pub ph: f64,
    pub turbidity_ntu: f64,
    pub conductivity_us_cm: f64,
    pub hardness_mg_l_caco3: f64,
    pub suspended_solids_mg_l: f64,
    pub glaze_contamination_percent: f64,
    pub recovered_fraction_percent: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiDownstreamMaterialEffects {
    pub filling_flow_factor: f64,
    pub casting_rate_g_cm2_min: f64,
    pub predicted_green_moisture_percent: f64,
    pub predicted_drying_shrinkage_percent: f64,
    pub drying_energy_factor: f64,
    pub green_strength_index: f64,
    pub fired_defect_risk_percent: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiBodyIngredientState {
    pub id: &'static str,
    pub label: &'static str,
    pub target_kg: f64,
    pub actual_kg: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiBodyQualityCheck {
    pub id: &'static str,
    pub label: &'static str,
    pub value: f64,
    pub unit: &'static str,
    pub minimum: f64,
    pub maximum: f64,
    pub within_limit: bool,
}
