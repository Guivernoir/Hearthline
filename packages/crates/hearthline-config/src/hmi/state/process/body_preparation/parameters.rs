use hearthline_engine::{BodyPreparationSetpoints, GlazeSetpoints, SlipSetpoints, WaterSetpoints};

use crate::hmi::HmiParameter;

pub(super) fn assign(sp: &mut BodyPreparationSetpoints, id: &str, value: f64) -> bool {
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

pub(super) fn from_parameters(parameters: &[HmiParameter]) -> BodyPreparationSetpoints {
    let defaults = BodyPreparationSetpoints::default();
    let value = |id: &str, fallback: f64| {
        parameters
            .iter()
            .find(|parameter| parameter.id == id)
            .map_or(fallback, |parameter| parameter.value)
    };
    BodyPreparationSetpoints {
        slip: SlipSetpoints {
            ball_clay_kg: value("body-ball-clay-kg", defaults.slip.ball_clay_kg),
            kaolin_kg: value("body-kaolin-kg", defaults.slip.kaolin_kg),
            feldspar_kg: value("body-feldspar-kg", defaults.slip.feldspar_kg),
            quartz_kg: value("body-quartz-kg", defaults.slip.quartz_kg),
            water_kg: value("body-water-kg", defaults.slip.water_kg),
            sodium_silicate_kg: value("body-sodium-silicate-kg", defaults.slip.sodium_silicate_kg),
            mixing_minutes: value("body-mixing-minutes", defaults.slip.mixing_minutes),
            conditioning_hours: value("body-conditioning-hours", defaults.slip.conditioning_hours),
            target_temperature_c: value("body-temperature-c", defaults.slip.target_temperature_c),
            screen_micrometres: value("body-screen-micrometres", defaults.slip.screen_micrometres),
            mixing_energy_kwh_t: value(
                "body-mixing-energy-kwh-t",
                defaults.slip.mixing_energy_kwh_t,
            ),
        },
        water: WaterSetpoints {
            treatment_batch_l: value("water-treatment-batch-l", defaults.water.treatment_batch_l),
            ro_recovery_percent: value(
                "water-ro-recovery-percent",
                defaults.water.ro_recovery_percent,
            ),
            target_conductivity_us_cm: value(
                "water-target-conductivity-us-cm",
                defaults.water.target_conductivity_us_cm,
            ),
            target_hardness_mg_l: value(
                "water-target-hardness-mg-l",
                defaults.water.target_hardness_mg_l,
            ),
            target_turbidity_ntu: value(
                "water-target-turbidity-ntu",
                defaults.water.target_turbidity_ntu,
            ),
            maximum_body_reuse_percent: value(
                "water-body-reuse-percent",
                defaults.water.maximum_body_reuse_percent,
            ),
            maximum_glaze_reuse_percent: value(
                "water-glaze-reuse-percent",
                defaults.water.maximum_glaze_reuse_percent,
            ),
            return_batch_l: value("return-water-batch-l", defaults.water.return_batch_l),
        },
        glaze: GlazeSetpoints {
            kaolin_kg: value("glaze-kaolin-kg", defaults.glaze.kaolin_kg),
            sodium_feldspar_kg: value("glaze-feldspar-kg", defaults.glaze.sodium_feldspar_kg),
            quartz_kg: value("glaze-quartz-kg", defaults.glaze.quartz_kg),
            calcite_kg: value("glaze-calcite-kg", defaults.glaze.calcite_kg),
            dolomite_kg: value("glaze-dolomite-kg", defaults.glaze.dolomite_kg),
            zinc_oxide_kg: value("glaze-zinc-oxide-kg", defaults.glaze.zinc_oxide_kg),
            zircon_kg: value("glaze-zircon-kg", defaults.glaze.zircon_kg),
            water_kg: value("glaze-water-kg", defaults.glaze.water_kg),
            sodium_silicate_kg: value(
                "glaze-sodium-silicate-kg",
                defaults.glaze.sodium_silicate_kg,
            ),
            milling_minutes: value("glaze-milling-minutes", defaults.glaze.milling_minutes),
            screen_micrometres: value(
                "glaze-screen-micrometres",
                defaults.glaze.screen_micrometres,
            ),
            target_density_kg_l: value("glaze-density-kg-l", defaults.glaze.target_density_kg_l),
            target_ford_cup_seconds: value(
                "glaze-ford-cup-seconds",
                defaults.glaze.target_ford_cup_seconds,
            ),
        },
    }
}
