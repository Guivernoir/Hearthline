use hearthline_engine::BodyPreparationSetpoints;
use hearthline_model::FixedValue;

use crate::hmi::state::RuntimeParameter;

pub(super) fn assign(sp: &mut BodyPreparationSetpoints, id: &str, value: FixedValue) -> bool {
    match id {
        "body-ball-clay-kg" => sp.slip.ball_clay_kg = value,
        "body-kaolin-kg" => sp.slip.kaolin_kg = value,
        "body-feldspar-kg" => sp.slip.feldspar_kg = value,
        "body-quartz-kg" => sp.slip.quartz_kg = value,
        "body-water-kg" => sp.slip.water_kg = value,
        "body-sodium-silicate-kg" => sp.slip.sodium_silicate_kg = value,
        "body-mixing-minutes" => sp.slip.mixing_minutes = value,
        "body-conditioning-hours" => sp.slip.conditioning_hours = value,
        "body-temperature-c" => sp.slip.target_temperature_c = value,
        "body-screen-micrometres" => sp.slip.screen_micrometres = value,
        "body-mixing-energy-kwh-t" => sp.slip.mixing_energy_kwh_t = value,
        "water-treatment-batch-l" => sp.water.treatment_batch_l = value,
        "water-ro-recovery-percent" => sp.water.ro_recovery_percent = value,
        "water-target-conductivity-us-cm" => sp.water.target_conductivity_us_cm = value,
        "water-target-hardness-mg-l" => sp.water.target_hardness_mg_l = value,
        "water-target-turbidity-ntu" => sp.water.target_turbidity_ntu = value,
        "water-body-reuse-percent" => sp.water.maximum_body_reuse_percent = value,
        "water-glaze-reuse-percent" => sp.water.maximum_glaze_reuse_percent = value,
        "return-water-batch-l" => sp.water.return_batch_l = value,
        "glaze-kaolin-kg" => sp.glaze.kaolin_kg = value,
        "glaze-feldspar-kg" => sp.glaze.sodium_feldspar_kg = value,
        "glaze-quartz-kg" => sp.glaze.quartz_kg = value,
        "glaze-calcite-kg" => sp.glaze.calcite_kg = value,
        "glaze-dolomite-kg" => sp.glaze.dolomite_kg = value,
        "glaze-zinc-oxide-kg" => sp.glaze.zinc_oxide_kg = value,
        "glaze-zircon-kg" => sp.glaze.zircon_kg = value,
        "glaze-water-kg" => sp.glaze.water_kg = value,
        "glaze-sodium-silicate-kg" => sp.glaze.sodium_silicate_kg = value,
        "glaze-milling-minutes" => sp.glaze.milling_minutes = value,
        "glaze-screen-micrometres" => sp.glaze.screen_micrometres = value,
        "glaze-density-kg-l" => sp.glaze.target_density_kg_l = value,
        "glaze-ford-cup-seconds" => sp.glaze.target_ford_cup_seconds = value,
        _ => return false,
    }
    true
}

pub(super) fn from_parameters(parameters: &[RuntimeParameter]) -> BodyPreparationSetpoints {
    from_parameter_groups([parameters])
}

pub(super) fn from_parameter_groups<'a>(
    groups: impl IntoIterator<Item = &'a [RuntimeParameter]>,
) -> BodyPreparationSetpoints {
    let mut setpoints = BodyPreparationSetpoints::default();
    for parameter in groups.into_iter().flatten() {
        assign(&mut setpoints, &parameter.id, parameter.value);
    }
    setpoints
}
