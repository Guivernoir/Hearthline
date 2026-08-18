use super::rheology;
use super::{
    BodyPreparationFault, BodyPreparationOutputs, BodyPreparationStartError, BodyPreparationTrip,
    CeramicSlipBatch, SIMULATED_MS_PER_PROCESS_MINUTE, WaterQuality,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlipPhase {
    Idle,
    WaterCharge,
    DeflocculantCharge,
    BallClayCharge,
    KaolinCharge,
    FeldsparCharge,
    QuartzCharge,
    WetMixing,
    Screening,
    MagneticSeparation,
    Conditioning,
    QualityCheck,
    TemperatureTrim,
    Transfer,
    Complete,
    Faulted,
}

impl SlipPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::WaterCharge => "water-charge",
            Self::DeflocculantCharge => "deflocculant-charge",
            Self::BallClayCharge => "ball-clay-charge",
            Self::KaolinCharge => "kaolin-charge",
            Self::FeldsparCharge => "feldspar-charge",
            Self::QuartzCharge => "quartz-charge",
            Self::WetMixing => "wet-mixing",
            Self::Screening => "screening",
            Self::MagneticSeparation => "magnetic-separation",
            Self::Conditioning => "conditioning-ageing",
            Self::QualityCheck => "rheology-quality-release",
            Self::TemperatureTrim => "temperature-trim",
            Self::Transfer => "transfer-to-forming",
            Self::Complete => "batch-complete",
            Self::Faulted => "faulted",
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::Idle | Self::Faulted => self,
            Self::WaterCharge => Self::DeflocculantCharge,
            Self::DeflocculantCharge => Self::BallClayCharge,
            Self::BallClayCharge => Self::KaolinCharge,
            Self::KaolinCharge => Self::FeldsparCharge,
            Self::FeldsparCharge => Self::QuartzCharge,
            Self::QuartzCharge => Self::WetMixing,
            Self::WetMixing => Self::Screening,
            Self::Screening => Self::MagneticSeparation,
            Self::MagneticSeparation => Self::Conditioning,
            Self::Conditioning => Self::QualityCheck,
            Self::QualityCheck => Self::TemperatureTrim,
            Self::TemperatureTrim => Self::Transfer,
            Self::Transfer => Self::Complete,
            Self::Complete => Self::Idle,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlipSetpoints {
    pub ball_clay_kg: f64,
    pub kaolin_kg: f64,
    pub feldspar_kg: f64,
    pub quartz_kg: f64,
    pub water_kg: f64,
    pub sodium_silicate_kg: f64,
    pub mixing_minutes: f64,
    pub conditioning_hours: f64,
    pub target_temperature_c: f64,
    pub screen_micrometres: f64,
    pub mixing_energy_kwh_t: f64,
}

impl Default for SlipSetpoints {
    fn default() -> Self {
        Self {
            ball_clay_kg: 350.0,
            kaolin_kg: 150.0,
            feldspar_kg: 250.0,
            quartz_kg: 250.0,
            water_kg: 333.3,
            sodium_silicate_kg: 2.0,
            mixing_minutes: 90.0,
            conditioning_hours: 8.0,
            target_temperature_c: 40.0,
            screen_micrometres: 125.0,
            mixing_energy_kwh_t: 3.75,
        }
    }
}

impl SlipSetpoints {
    pub fn dry_mass_kg(self) -> f64 {
        self.ball_clay_kg + self.kaolin_kg + self.feldspar_kg + self.quartz_kg
    }

    pub fn total_batch_mass_kg(self) -> f64 {
        self.dry_mass_kg() + self.water_kg + self.sodium_silicate_kg
    }

    pub fn target_solids_percent(self) -> f64 {
        self.dry_mass_kg() / self.total_batch_mass_kg() * 100.0
    }

    pub fn phase_duration_ms(self, phase: SlipPhase) -> u64 {
        let minutes = match phase {
            SlipPhase::Idle | SlipPhase::Faulted => 0.0,
            SlipPhase::WaterCharge => 12.0,
            SlipPhase::DeflocculantCharge => 5.0,
            SlipPhase::BallClayCharge | SlipPhase::KaolinCharge => 10.0,
            SlipPhase::FeldsparCharge | SlipPhase::QuartzCharge => 8.0,
            SlipPhase::WetMixing => self.mixing_minutes,
            SlipPhase::Screening => 15.0,
            SlipPhase::MagneticSeparation => 5.0,
            SlipPhase::Conditioning => self.conditioning_hours * 60.0,
            SlipPhase::QualityCheck => 10.0,
            SlipPhase::TemperatureTrim => 30.0,
            SlipPhase::Transfer => 25.0,
            SlipPhase::Complete => 5.0,
        };
        (minutes * SIMULATED_MS_PER_PROCESS_MINUTE as f64 + 0.5) as u64
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlipMeasurements {
    pub ball_clay_kg: f64,
    pub kaolin_kg: f64,
    pub feldspar_kg: f64,
    pub quartz_kg: f64,
    pub water_kg: f64,
    pub sodium_silicate_kg: f64,
    pub batch_mass_kg: f64,
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
    pub water: WaterQuality,
}

impl SlipMeasurements {
    const fn empty() -> Self {
        Self {
            ball_clay_kg: 0.0,
            kaolin_kg: 0.0,
            feldspar_kg: 0.0,
            quartz_kg: 0.0,
            water_kg: 0.0,
            sodium_silicate_kg: 0.0,
            batch_mass_kg: 0.0,
            solids_percent: 0.0,
            density_kg_l: 1.0,
            high_shear_viscosity_mpa_s: 1.0,
            low_shear_viscosity_mpa_s: 1.0,
            thixotropic_index: 1.0,
            structure_parameter: 0.0,
            temperature_c: 25.0,
            mixer_level_percent: 0.0,
            conditioning_tank_level_percent: 0.0,
            transfer_flow_l_min: 0.0,
            specific_energy_kwh_t: 0.0,
            residue_44um_percent: 18.0,
            median_particle_um: 95.0,
            casting_rate_g_cm2_min: 0.0,
            quality_index: 0.0,
            water: WaterQuality::treated_default(),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct SlipRuntime {
    pub phase: SlipPhase,
    pub phase_elapsed_ms: u64,
    pub batch_count: u64,
    pub running: bool,
    pub held: bool,
    pub measurements: SlipMeasurements,
    release_pending: bool,
}

impl SlipRuntime {
    pub const fn new() -> Self {
        Self {
            phase: SlipPhase::Idle,
            phase_elapsed_ms: 0,
            batch_count: 0,
            running: false,
            held: false,
            measurements: SlipMeasurements::empty(),
            release_pending: false,
        }
    }

    pub fn start(&mut self, water: WaterQuality) -> Result<(), BodyPreparationStartError> {
        if self.running {
            return Err(BodyPreparationStartError::AlreadyRunning);
        }
        if self.held {
            self.running = true;
            self.held = false;
            return Ok(());
        }
        self.measurements = SlipMeasurements::empty();
        self.measurements.water = water;
        self.measurements.temperature_c = water.temperature_c;
        self.phase = SlipPhase::WaterCharge;
        self.phase_elapsed_ms = 0;
        self.running = true;
        self.release_pending = false;
        Ok(())
    }

    pub fn hold(&mut self) -> bool {
        if !self.running {
            return false;
        }
        self.running = false;
        self.held = true;
        true
    }

    pub fn reset_fault(&mut self) {
        if self.phase == SlipPhase::Faulted {
            self.phase = SlipPhase::Idle;
            self.phase_elapsed_ms = 0;
            self.running = false;
            self.held = false;
        }
    }

    pub fn progress(&self, setpoints: &SlipSetpoints) -> f64 {
        let duration = setpoints.phase_duration_ms(self.phase);
        if duration == 0 {
            0.0
        } else {
            (self.phase_elapsed_ms as f64 / duration as f64).clamp(0.0, 1.0)
        }
    }

    pub fn tick(
        &mut self,
        elapsed_ms: u64,
        setpoints: &SlipSetpoints,
        fault: Option<BodyPreparationFault>,
    ) -> (bool, Option<BodyPreparationTrip>) {
        if !self.running {
            return (false, None);
        }
        self.phase_elapsed_ms = self.phase_elapsed_ms.saturating_add(elapsed_ms);
        self.recalculate(setpoints);
        if let Some(trip) = self.evaluate_fault(fault) {
            self.trip();
            return (true, Some(trip));
        }
        let mut changed = false;
        while self.running && self.phase_elapsed_ms >= setpoints.phase_duration_ms(self.phase) {
            self.phase_elapsed_ms -= setpoints.phase_duration_ms(self.phase);
            self.phase = self.phase.next();
            changed = true;
            self.recalculate(setpoints);
            if self.phase == SlipPhase::QualityCheck && !self.quality_released() {
                self.trip();
                return (true, Some(BodyPreparationTrip::QualityReleaseDenied));
            }
            if self.phase == SlipPhase::Idle {
                self.running = false;
                self.batch_count = self.batch_count.saturating_add(1);
                self.release_pending = true;
            }
        }
        (changed, None)
    }

    pub fn apply_outputs(&self, outputs: &mut BodyPreparationOutputs) {
        if !self.running {
            return;
        }
        match self.phase {
            SlipPhase::WaterCharge => outputs.slip_water_valve = "open",
            SlipPhase::WetMixing => outputs.slip_blunger = "mixing",
            SlipPhase::Screening => outputs.slip_screen = "screening",
            SlipPhase::Transfer => outputs.slip_transfer_pump = "transferring",
            _ => {}
        }
    }

    pub fn take_release_pending(&mut self) -> bool {
        let pending = self.release_pending;
        self.release_pending = false;
        pending
    }

    pub fn release_batch(&self, setpoints: &SlipSetpoints) -> CeramicSlipBatch {
        let m = self.measurements;
        CeramicSlipBatch {
            batch_number: self.batch_count,
            density_kg_l: m.density_kg_l,
            high_shear_viscosity_mpa_s: m.high_shear_viscosity_mpa_s,
            low_shear_viscosity_mpa_s: m.low_shear_viscosity_mpa_s,
            thixotropic_index: m.thixotropic_index,
            temperature_c: m.temperature_c,
            solids_percent: m.solids_percent,
            residue_44um_percent: m.residue_44um_percent,
            median_particle_um: m.median_particle_um,
            water: m.water,
            entrained_air_percent: 0.15,
            effects: rheology::downstream_effects(m, setpoints),
        }
    }

    pub fn quality_released(&self) -> bool {
        let m = self.measurements;
        (1.78..=1.84).contains(&m.density_kg_l)
            && (400.0..=850.0).contains(&m.high_shear_viscosity_mpa_s)
            && (4.0..=7.5).contains(&m.thixotropic_index)
            && (7.0..=11.0).contains(&m.residue_44um_percent)
            && m.water.acceptable_for_slip()
    }

    fn recalculate(&mut self, sp: &SlipSetpoints) {
        let p = self.progress(sp);
        match self.phase {
            SlipPhase::WaterCharge => self.measurements.water_kg = sp.water_kg * p,
            SlipPhase::DeflocculantCharge => {
                self.measurements.water_kg = sp.water_kg;
                self.measurements.sodium_silicate_kg = sp.sodium_silicate_kg * p;
            }
            SlipPhase::BallClayCharge => {
                self.measurements.water_kg = sp.water_kg;
                self.measurements.sodium_silicate_kg = sp.sodium_silicate_kg;
                self.measurements.ball_clay_kg = sp.ball_clay_kg * p;
            }
            SlipPhase::KaolinCharge => {
                self.measurements.water_kg = sp.water_kg;
                self.measurements.sodium_silicate_kg = sp.sodium_silicate_kg;
                self.measurements.ball_clay_kg = sp.ball_clay_kg;
                self.measurements.kaolin_kg = sp.kaolin_kg * p;
            }
            SlipPhase::FeldsparCharge => {
                self.measurements.water_kg = sp.water_kg;
                self.measurements.sodium_silicate_kg = sp.sodium_silicate_kg;
                self.measurements.ball_clay_kg = sp.ball_clay_kg;
                self.measurements.kaolin_kg = sp.kaolin_kg;
                self.measurements.feldspar_kg = sp.feldspar_kg * p;
            }
            SlipPhase::QuartzCharge => {
                self.measurements.water_kg = sp.water_kg;
                self.measurements.sodium_silicate_kg = sp.sodium_silicate_kg;
                self.measurements.ball_clay_kg = sp.ball_clay_kg;
                self.measurements.kaolin_kg = sp.kaolin_kg;
                self.measurements.feldspar_kg = sp.feldspar_kg;
                self.measurements.quartz_kg = sp.quartz_kg * p;
            }
            SlipPhase::WetMixing => {
                self.full_charge(sp);
                self.measurements.specific_energy_kwh_t = sp.mixing_energy_kwh_t * p;
                self.measurements.structure_parameter = 0.9 - 0.75 * p;
            }
            SlipPhase::Screening => {
                self.full_charge(sp);
                self.measurements.specific_energy_kwh_t = sp.mixing_energy_kwh_t;
                self.measurements.structure_parameter = 0.15;
            }
            SlipPhase::MagneticSeparation => self.full_mixed(sp, 0.2),
            SlipPhase::Conditioning => self.full_mixed(sp, 0.2 + 0.8 * p),
            SlipPhase::QualityCheck => self.full_mixed(sp, 1.0),
            SlipPhase::TemperatureTrim => {
                self.full_mixed(sp, 1.0);
                let start = self.measurements.water.temperature_c;
                self.measurements.temperature_c = start + (sp.target_temperature_c - start) * p;
            }
            SlipPhase::Transfer | SlipPhase::Complete | SlipPhase::Idle => {
                if self.measurements.batch_mass_kg > 0.0 {
                    self.full_mixed(sp, 1.0);
                }
                if matches!(self.phase, SlipPhase::Transfer | SlipPhase::Complete) {
                    self.measurements.temperature_c = sp.target_temperature_c;
                }
                self.measurements.transfer_flow_l_min = if self.phase == SlipPhase::Transfer {
                    68.0
                } else {
                    0.0
                };
            }
            SlipPhase::Faulted => self.measurements.transfer_flow_l_min = 0.0,
        }
        rheology::update_slip_physics(&mut self.measurements, self.phase, sp);
    }

    fn full_charge(&mut self, sp: &SlipSetpoints) {
        self.measurements.ball_clay_kg = sp.ball_clay_kg;
        self.measurements.kaolin_kg = sp.kaolin_kg;
        self.measurements.feldspar_kg = sp.feldspar_kg;
        self.measurements.quartz_kg = sp.quartz_kg;
        self.measurements.water_kg = sp.water_kg;
        self.measurements.sodium_silicate_kg = sp.sodium_silicate_kg;
    }

    fn full_mixed(&mut self, sp: &SlipSetpoints, structure: f64) {
        self.full_charge(sp);
        self.measurements.specific_energy_kwh_t = sp.mixing_energy_kwh_t;
        self.measurements.structure_parameter = structure;
    }

    fn evaluate_fault(&self, fault: Option<BodyPreparationFault>) -> Option<BodyPreparationTrip> {
        match fault {
            Some(BodyPreparationFault::IngredientShortage)
                if matches!(
                    self.phase,
                    SlipPhase::BallClayCharge
                        | SlipPhase::KaolinCharge
                        | SlipPhase::FeldsparCharge
                        | SlipPhase::QuartzCharge
                ) =>
            {
                Some(BodyPreparationTrip::IngredientDoseNotEstablished)
            }
            Some(BodyPreparationFault::MixerOverload) if self.phase == SlipPhase::WetMixing => {
                Some(BodyPreparationTrip::MixerOverload)
            }
            Some(BodyPreparationFault::ScreenBlocked) if self.phase == SlipPhase::Screening => {
                Some(BodyPreparationTrip::ScreenDifferentialHigh)
            }
            Some(BodyPreparationFault::QualityOutOfSpec)
                if self.phase == SlipPhase::QualityCheck =>
            {
                Some(BodyPreparationTrip::QualityReleaseDenied)
            }
            Some(BodyPreparationFault::TransferNoFlow) if self.phase == SlipPhase::Transfer => {
                Some(BodyPreparationTrip::TransferFlowNotEstablished)
            }
            _ => None,
        }
    }

    fn trip(&mut self) {
        self.running = false;
        self.held = false;
        self.phase = SlipPhase::Faulted;
        self.phase_elapsed_ms = 0;
        self.measurements.transfer_flow_l_min = 0.0;
    }
}
