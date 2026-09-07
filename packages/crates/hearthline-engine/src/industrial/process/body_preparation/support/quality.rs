use hearthline_model::{FixedValue, fixed};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterQuality {
    pub temperature_c: FixedValue,
    pub ph: FixedValue,
    pub turbidity_ntu: FixedValue,
    pub conductivity_us_cm: FixedValue,
    pub hardness_mg_l_caco3: FixedValue,
    pub suspended_solids_mg_l: FixedValue,
    pub glaze_contamination_percent: FixedValue,
    pub recovered_fraction_percent: FixedValue,
}

impl WaterQuality {
    pub const fn raw_default() -> Self {
        Self {
            temperature_c: fixed!(24.0),
            ph: fixed!(7.4),
            turbidity_ntu: fixed!(8.0),
            conductivity_us_cm: fixed!(650.0),
            hardness_mg_l_caco3: fixed!(220.0),
            suspended_solids_mg_l: fixed!(18.0),
            glaze_contamination_percent: fixed!(0.0),
            recovered_fraction_percent: fixed!(0.0),
        }
    }

    pub const fn treated_default() -> Self {
        Self {
            temperature_c: fixed!(25.0),
            ph: fixed!(7.0),
            turbidity_ntu: fixed!(0.25),
            conductivity_us_cm: fixed!(80.0),
            hardness_mg_l_caco3: fixed!(8.0),
            suspended_solids_mg_l: fixed!(0.5),
            glaze_contamination_percent: fixed!(0.0),
            recovered_fraction_percent: fixed!(0.0),
        }
    }

    pub fn blend(self, other: Self, other_fraction: FixedValue) -> Self {
        let fraction = other_fraction.clamp(fixed!(0.0), fixed!(1.0));
        let keep = fixed!(1.0) - fraction;
        Self {
            temperature_c: self.temperature_c * keep + other.temperature_c * fraction,
            ph: self.ph * keep + other.ph * fraction,
            turbidity_ntu: self.turbidity_ntu * keep + other.turbidity_ntu * fraction,
            conductivity_us_cm: self.conductivity_us_cm * keep
                + other.conductivity_us_cm * fraction,
            hardness_mg_l_caco3: self.hardness_mg_l_caco3 * keep
                + other.hardness_mg_l_caco3 * fraction,
            suspended_solids_mg_l: self.suspended_solids_mg_l * keep
                + other.suspended_solids_mg_l * fraction,
            glaze_contamination_percent: self.glaze_contamination_percent * keep
                + other.glaze_contamination_percent * fraction,
            recovered_fraction_percent: fraction * fixed!(100.0),
        }
    }

    pub fn acceptable_for_slip(self) -> bool {
        self.turbidity_ntu <= fixed!(2.0)
            && self.conductivity_us_cm <= fixed!(350.0)
            && self.hardness_mg_l_caco3 <= fixed!(80.0)
            && self.glaze_contamination_percent <= fixed!(0.05)
    }

    pub fn acceptable_for_glaze(self) -> bool {
        self.turbidity_ntu <= fixed!(3.0) && self.conductivity_us_cm <= fixed!(500.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DownstreamMaterialEffects {
    pub filling_flow_factor: FixedValue,
    pub casting_rate_g_cm2_min: FixedValue,
    pub predicted_green_moisture_percent: FixedValue,
    pub predicted_drying_shrinkage_percent: FixedValue,
    pub drying_energy_factor: FixedValue,
    pub green_strength_index: FixedValue,
    pub fired_defect_risk_percent: FixedValue,
}

impl DownstreamMaterialEffects {
    pub const fn reference_baseline() -> Self {
        Self {
            filling_flow_factor: fixed!(1.0),
            casting_rate_g_cm2_min: fixed!(0.152),
            predicted_green_moisture_percent: fixed!(20.5),
            predicted_drying_shrinkage_percent: fixed!(2.1),
            drying_energy_factor: fixed!(1.0),
            green_strength_index: fixed!(100.0),
            fired_defect_risk_percent: fixed!(1.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CeramicSlipBatch {
    pub batch_number: u64,
    pub density_kg_l: FixedValue,
    pub high_shear_viscosity_mpa_s: FixedValue,
    pub low_shear_viscosity_mpa_s: FixedValue,
    pub thixotropic_index: FixedValue,
    pub temperature_c: FixedValue,
    pub solids_percent: FixedValue,
    pub residue_44um_percent: FixedValue,
    pub median_particle_um: FixedValue,
    pub water: WaterQuality,
    pub entrained_air_percent: FixedValue,
    pub effects: DownstreamMaterialEffects,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlazeBatch {
    pub batch_number: u64,
    pub density_kg_l: FixedValue,
    pub ford_cup_seconds: FixedValue,
    pub residue_63um_percent: FixedValue,
    pub median_particle_um: FixedValue,
    pub solids_percent: FixedValue,
    pub water: WaterQuality,
}

use super::{BodyPreparationProcess, SIMULATED_MS_PER_PROCESS_MINUTE};

impl BodyPreparationProcess {
    pub(super) fn update_handoff_pipelines(&mut self) {
        let fault = self.fault;
        self.pipelines.water_to_slip = if self.slip.phase == SlipPhase::WaterCharge {
            HandoffPipelineMeasurements::flowing(
                fixed!(80.0),
                fixed!(2.4),
                fault == Some(BodyPreparationFault::WaterToSlipLeak),
                false,
            )
        } else {
            self.pipelines.water_to_slip.stopped()
        };
        self.pipelines.water_to_glaze = if self.glaze.phase == GlazePhase::WaterCharge {
            HandoffPipelineMeasurements::flowing(
                fixed!(55.0),
                fixed!(2.2),
                fault == Some(BodyPreparationFault::WaterToGlazeLeak),
                false,
            )
        } else {
            self.pipelines.water_to_glaze.stopped()
        };
        self.pipelines.slip_to_forming = if self.slip.phase == SlipPhase::Transfer {
            HandoffPipelineMeasurements::flowing(
                self.slip.measurements.transfer_flow_l_min.max(fixed!(68.0)),
                fixed!(3.2),
                fault == Some(BodyPreparationFault::SlipPipelineLeak),
                true,
            )
        } else {
            self.pipelines.slip_to_forming.stopped()
        };
        self.pipelines.glaze_to_glazing = if self.glaze.phase == GlazePhase::Transfer {
            HandoffPipelineMeasurements::flowing(
                self.glaze
                    .measurements
                    .transfer_flow_l_min
                    .max(fixed!(42.0)),
                fixed!(2.8),
                fault == Some(BodyPreparationFault::GlazePipelineLeak),
                false,
            )
        } else {
            self.pipelines.glaze_to_glazing.stopped()
        };
    }

    pub(super) fn apply_slip_handoff(&self, mut batch: CeramicSlipBatch) -> CeramicSlipBatch {
        let leaking = self.fault == Some(BodyPreparationFault::SlipPipelineLeak);
        batch.entrained_air_percent = if leaking { fixed!(3.5) } else { fixed!(0.15) };
        if leaking {
            batch.effects.filling_flow_factor *= fixed!(0.78);
            batch.effects.casting_rate_g_cm2_min *= fixed!(0.88);
            batch.effects.green_strength_index *= fixed!(0.82);
            batch.effects.fired_defect_risk_percent = (batch.effects.fired_defect_risk_percent
                + fixed!(22.0))
            .clamp(fixed!(1.0), fixed!(75.0));
        }
        batch
    }

    pub const fn train_phase(&self, train: PreparationTrain) -> &'static str {
        match train {
            PreparationTrain::Slip => self.slip.phase.as_str(),
            PreparationTrain::Water => self.water.phase.as_str(),
            PreparationTrain::ReturnWater => self.return_water.phase.as_str(),
            PreparationTrain::Glaze => self.glaze.phase.as_str(),
        }
    }

    pub const fn train_running(&self, train: PreparationTrain) -> bool {
        match train {
            PreparationTrain::Slip => self.slip.running,
            PreparationTrain::Water => self.water.running,
            PreparationTrain::ReturnWater => self.return_water.running,
            PreparationTrain::Glaze => self.glaze.running,
        }
    }

    pub const fn train_held(&self, train: PreparationTrain) -> bool {
        match train {
            PreparationTrain::Slip => self.slip.held,
            PreparationTrain::Water => self.water.held,
            PreparationTrain::ReturnWater => self.return_water.held,
            PreparationTrain::Glaze => self.glaze.held,
        }
    }

    pub const fn train_elapsed_ms(&self, train: PreparationTrain) -> u64 {
        match train {
            PreparationTrain::Slip => self.slip.phase_elapsed_ms,
            PreparationTrain::Water => self.water.phase_elapsed_ms,
            PreparationTrain::ReturnWater => self.return_water.phase_elapsed_ms,
            PreparationTrain::Glaze => self.glaze.phase_elapsed_ms,
        }
    }

    pub const fn train_cycle_count(&self, train: PreparationTrain) -> u64 {
        match train {
            PreparationTrain::Slip => self.slip.batch_count,
            PreparationTrain::Water => self.water.cycle_count,
            PreparationTrain::ReturnWater => self.return_water.cycle_count,
            PreparationTrain::Glaze => self.glaze.batch_count,
        }
    }

    pub fn train_target_process_minutes(&self, train: PreparationTrain) -> FixedValue {
        let duration = match train {
            PreparationTrain::Slip => self.setpoints.slip.phase_duration_ms(self.slip.phase),
            PreparationTrain::Water => self.setpoints.water.phase_duration_ms(self.water.phase),
            PreparationTrain::ReturnWater => self
                .setpoints
                .water
                .return_phase_duration_ms(self.return_water.phase),
            PreparationTrain::Glaze => self.setpoints.glaze.phase_duration_ms(self.glaze.phase),
        };
        FixedValue::from_u64_ratio(duration, SIMULATED_MS_PER_PROCESS_MINUTE)
    }

    pub fn train_progress_percent(&self, train: PreparationTrain) -> FixedValue {
        let target = (self.train_target_process_minutes(train)
            * FixedValue::from_integer(SIMULATED_MS_PER_PROCESS_MINUTE as i64))
        .round_to_u64();
        if target == 0 {
            fixed!(0.0)
        } else {
            (FixedValue::from_u64_ratio(self.train_elapsed_ms(train), target) * fixed!(100.0))
                .clamp(fixed!(0.0), fixed!(100.0))
        }
    }

    pub fn slip_effects_preview(&self) -> DownstreamMaterialEffects {
        if let Some(batch) = self.released_slip {
            return batch.effects;
        }
        if self.slip.measurements.batch_mass_kg <= fixed!(0.0) {
            return DownstreamMaterialEffects::reference_baseline();
        }
        super::rheology::downstream_effects(self.slip.measurements, &self.setpoints.slip)
    }

    pub fn slip_quality_released(&self) -> bool {
        self.slip.quality_released()
    }

    pub fn glaze_quality_released(&self) -> bool {
        self.glaze.quality_released()
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationTrain {
    Slip,
    Water,
    ReturnWater,
    Glaze,
}

impl PreparationTrain {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Slip => "slip",
            Self::Water => "water",
            Self::ReturnWater => "return-water",
            Self::Glaze => "glaze",
        }
    }
}
use super::{GlazePhase, SlipPhase};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyPreparationFault {
    IngredientShortage,
    MixerOverload,
    ScreenBlocked,
    QualityOutOfSpec,
    TransferNoFlow,
    RawWaterQuality,
    WaterFilterBlocked,
    ReturnWaterContamination,
    GlazeMillOverload,
    GlazeQualityOutOfSpec,
    SlipPipelineLeak,
    WaterToSlipLeak,
    WaterToGlazeLeak,
    GlazePipelineLeak,
}

impl BodyPreparationFault {
    pub const fn prevents_start(self) -> bool {
        !matches!(
            self,
            Self::SlipPipelineLeak
                | Self::WaterToSlipLeak
                | Self::WaterToGlazeLeak
                | Self::GlazePipelineLeak
        )
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IngredientShortage => "ingredient-shortage",
            Self::MixerOverload => "mixer-overload",
            Self::ScreenBlocked => "screen-blocked",
            Self::QualityOutOfSpec => "quality-out-of-spec",
            Self::TransferNoFlow => "transfer-no-flow",
            Self::RawWaterQuality => "raw-water-quality",
            Self::WaterFilterBlocked => "water-filter-blocked",
            Self::ReturnWaterContamination => "return-water-contamination",
            Self::GlazeMillOverload => "glaze-mill-overload",
            Self::GlazeQualityOutOfSpec => "glaze-quality-out-of-spec",
            Self::SlipPipelineLeak => "slip-pipeline-leak",
            Self::WaterToSlipLeak => "water-to-slip-leak",
            Self::WaterToGlazeLeak => "water-to-glaze-leak",
            Self::GlazePipelineLeak => "glaze-pipeline-leak",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyPreparationTrip {
    IngredientDoseNotEstablished,
    MixerOverload,
    ScreenDifferentialHigh,
    QualityReleaseDenied,
    TransferFlowNotEstablished,
    ProcessWaterUnavailable,
    WaterQualityRejected,
    ReturnWaterCrossContamination,
    GlazeMillOverload,
    GlazeQualityReleaseDenied,
}

impl BodyPreparationTrip {
    pub const fn code(self) -> &'static str {
        match self {
            Self::IngredientDoseNotEstablished => "BODY-SLIP-INGREDIENT-DOSE-FAILED",
            Self::MixerOverload => "BODY-SLIP-MIXER-OVERLOAD",
            Self::ScreenDifferentialHigh => "BODY-SLIP-SCREEN-BLOCKED",
            Self::QualityReleaseDenied => "BODY-SLIP-QUALITY-RELEASE-DENIED",
            Self::TransferFlowNotEstablished => "BODY-SLIP-TRANSFER-NO-FLOW",
            Self::ProcessWaterUnavailable => "BODY-PROCESS-WATER-UNAVAILABLE",
            Self::WaterQualityRejected => "BODY-WATER-QUALITY-REJECTED",
            Self::ReturnWaterCrossContamination => "BODY-RETURN-CROSS-CONTAMINATION",
            Self::GlazeMillOverload => "BODY-GLAZE-MILL-OVERLOAD",
            Self::GlazeQualityReleaseDenied => "BODY-GLAZE-QUALITY-RELEASE-DENIED",
        }
    }

    pub const fn message(self) -> &'static str {
        match self {
            Self::IngredientDoseNotEstablished => {
                "A commanded slip raw-material dose was not established."
            }
            Self::MixerOverload => "The slip blunger exceeded its simulated operating load.",
            Self::ScreenDifferentialHigh => {
                "The slip screening stage reached its differential-pressure limit."
            }
            Self::QualityReleaseDenied => {
                "The slip failed its rheology, density, residue, or water-quality release window."
            }
            Self::TransferFlowNotEstablished => {
                "The released-slip transfer pump did not establish flow."
            }
            Self::ProcessWaterUnavailable => {
                "The selected train could not reserve enough released process water."
            }
            Self::WaterQualityRejected => {
                "The process-water train could not meet its release specification."
            }
            Self::ReturnWaterCrossContamination => {
                "Glaze-derived return water was detected in the body-water reuse route."
            }
            Self::GlazeMillOverload => "The glaze wet mill exceeded its simulated operating load.",
            Self::GlazeQualityReleaseDenied => {
                "The glaze failed density, flow-time, or screen-residue release limits."
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyPreparationStartError {
    AlreadyRunning,
    SafetyNotReady,
    FaultActive,
    WaterUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BodyPreparationTick {
    pub phase_changed: bool,
    pub trip: Option<BodyPreparationTrip>,
    pub train: Option<PreparationTrain>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandoffPipelineMeasurements {
    pub inlet_flow_l_min: FixedValue,
    pub outlet_flow_l_min: FixedValue,
    pub inlet_pressure_bar: FixedValue,
    pub outlet_pressure_bar: FixedValue,
    pub line_loss_percent: FixedValue,
    pub entrained_air_percent: FixedValue,
    pub delivered_quality_percent: FixedValue,
    pub leak_detected: bool,
}

impl HandoffPipelineMeasurements {
    const fn idle() -> Self {
        Self {
            inlet_flow_l_min: fixed!(0.0),
            outlet_flow_l_min: fixed!(0.0),
            inlet_pressure_bar: fixed!(0.0),
            outlet_pressure_bar: fixed!(0.0),
            line_loss_percent: fixed!(0.0),
            entrained_air_percent: fixed!(0.0),
            delivered_quality_percent: fixed!(100.0),
            leak_detected: false,
        }
    }

    fn flowing(
        flow_l_min: FixedValue,
        pressure_bar: FixedValue,
        leak: bool,
        tracks_air: bool,
    ) -> Self {
        let loss = if leak { fixed!(24.0) } else { fixed!(1.2) };
        let air = if tracks_air && leak {
            fixed!(3.5)
        } else if tracks_air {
            fixed!(0.15)
        } else {
            fixed!(0.0)
        };
        Self {
            inlet_flow_l_min: flow_l_min,
            outlet_flow_l_min: flow_l_min * (fixed!(1.0) - loss / fixed!(100.0)),
            inlet_pressure_bar: pressure_bar,
            outlet_pressure_bar: pressure_bar * if leak { fixed!(0.43) } else { fixed!(0.91) },
            line_loss_percent: loss,
            entrained_air_percent: air,
            delivered_quality_percent: if leak { fixed!(64.0) } else { fixed!(99.5) },
            leak_detected: leak,
        }
    }

    fn stopped(mut self) -> Self {
        self.inlet_flow_l_min = fixed!(0.0);
        self.outlet_flow_l_min = fixed!(0.0);
        self.inlet_pressure_bar = fixed!(0.0);
        self.outlet_pressure_bar = fixed!(0.0);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyPreparationPipelineMeasurements {
    pub water_to_slip: HandoffPipelineMeasurements,
    pub water_to_glaze: HandoffPipelineMeasurements,
    pub slip_to_forming: HandoffPipelineMeasurements,
    pub glaze_to_glazing: HandoffPipelineMeasurements,
}

impl BodyPreparationPipelineMeasurements {
    pub const fn idle() -> Self {
        Self {
            water_to_slip: HandoffPipelineMeasurements::idle(),
            water_to_glaze: HandoffPipelineMeasurements::idle(),
            slip_to_forming: HandoffPipelineMeasurements::idle(),
            glaze_to_glazing: HandoffPipelineMeasurements::idle(),
        }
    }
}
