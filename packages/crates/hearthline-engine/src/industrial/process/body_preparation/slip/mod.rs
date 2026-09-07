use hearthline_model::{FixedValue, fixed};

use super::rheology;
use super::{
    BodyPreparationFault, BodyPreparationOutputs, BodyPreparationStartError, BodyPreparationTrip,
    CeramicSlipBatch, SIMULATED_MS_PER_PROCESS_MINUTE, WaterQuality,
};

mod physics;

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
    pub ball_clay_kg: FixedValue,
    pub kaolin_kg: FixedValue,
    pub feldspar_kg: FixedValue,
    pub quartz_kg: FixedValue,
    pub water_kg: FixedValue,
    pub sodium_silicate_kg: FixedValue,
    pub mixing_minutes: FixedValue,
    pub conditioning_hours: FixedValue,
    pub target_temperature_c: FixedValue,
    pub screen_micrometres: FixedValue,
    pub mixing_energy_kwh_t: FixedValue,
}

impl Default for SlipSetpoints {
    fn default() -> Self {
        Self {
            ball_clay_kg: fixed!(350.0),
            kaolin_kg: fixed!(150.0),
            feldspar_kg: fixed!(250.0),
            quartz_kg: fixed!(250.0),
            water_kg: fixed!(333.3),
            sodium_silicate_kg: fixed!(2.0),
            mixing_minutes: fixed!(90.0),
            conditioning_hours: fixed!(8.0),
            target_temperature_c: fixed!(40.0),
            screen_micrometres: fixed!(125.0),
            mixing_energy_kwh_t: fixed!(3.75),
        }
    }
}

impl SlipSetpoints {
    pub fn dry_mass_kg(self) -> FixedValue {
        self.ball_clay_kg + self.kaolin_kg + self.feldspar_kg + self.quartz_kg
    }

    pub fn total_batch_mass_kg(self) -> FixedValue {
        self.dry_mass_kg() + self.water_kg + self.sodium_silicate_kg
    }

    pub fn target_solids_percent(self) -> FixedValue {
        self.dry_mass_kg() / self.total_batch_mass_kg() * fixed!(100.0)
    }

    pub fn phase_duration_ms(self, phase: SlipPhase) -> u64 {
        let minutes = match phase {
            SlipPhase::Idle | SlipPhase::Faulted => fixed!(0.0),
            SlipPhase::WaterCharge => fixed!(12.0),
            SlipPhase::DeflocculantCharge => fixed!(5.0),
            SlipPhase::BallClayCharge | SlipPhase::KaolinCharge => fixed!(10.0),
            SlipPhase::FeldsparCharge | SlipPhase::QuartzCharge => fixed!(8.0),
            SlipPhase::WetMixing => self.mixing_minutes,
            SlipPhase::Screening => fixed!(15.0),
            SlipPhase::MagneticSeparation => fixed!(5.0),
            SlipPhase::Conditioning => self.conditioning_hours * fixed!(60.0),
            SlipPhase::QualityCheck => fixed!(10.0),
            SlipPhase::TemperatureTrim => fixed!(30.0),
            SlipPhase::Transfer => fixed!(25.0),
            SlipPhase::Complete => fixed!(5.0),
        };
        (minutes * FixedValue::from_integer(SIMULATED_MS_PER_PROCESS_MINUTE as i64)).round_to_u64()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlipMeasurements {
    pub ball_clay_kg: FixedValue,
    pub kaolin_kg: FixedValue,
    pub feldspar_kg: FixedValue,
    pub quartz_kg: FixedValue,
    pub water_kg: FixedValue,
    pub sodium_silicate_kg: FixedValue,
    pub batch_mass_kg: FixedValue,
    pub solids_percent: FixedValue,
    pub density_kg_l: FixedValue,
    pub high_shear_viscosity_mpa_s: FixedValue,
    pub low_shear_viscosity_mpa_s: FixedValue,
    pub thixotropic_index: FixedValue,
    pub structure_parameter: FixedValue,
    pub temperature_c: FixedValue,
    pub mixer_level_percent: FixedValue,
    pub conditioning_tank_level_percent: FixedValue,
    pub transfer_flow_l_min: FixedValue,
    pub specific_energy_kwh_t: FixedValue,
    pub residue_44um_percent: FixedValue,
    pub median_particle_um: FixedValue,
    pub casting_rate_g_cm2_min: FixedValue,
    pub quality_index: FixedValue,
    pub water: WaterQuality,
}

impl SlipMeasurements {
    const fn empty() -> Self {
        Self {
            ball_clay_kg: fixed!(0.0),
            kaolin_kg: fixed!(0.0),
            feldspar_kg: fixed!(0.0),
            quartz_kg: fixed!(0.0),
            water_kg: fixed!(0.0),
            sodium_silicate_kg: fixed!(0.0),
            batch_mass_kg: fixed!(0.0),
            solids_percent: fixed!(0.0),
            density_kg_l: fixed!(1.0),
            high_shear_viscosity_mpa_s: fixed!(1.0),
            low_shear_viscosity_mpa_s: fixed!(1.0),
            thixotropic_index: fixed!(1.0),
            structure_parameter: fixed!(0.0),
            temperature_c: fixed!(25.0),
            mixer_level_percent: fixed!(0.0),
            conditioning_tank_level_percent: fixed!(0.0),
            transfer_flow_l_min: fixed!(0.0),
            specific_energy_kwh_t: fixed!(0.0),
            residue_44um_percent: fixed!(18.0),
            median_particle_um: fixed!(95.0),
            casting_rate_g_cm2_min: fixed!(0.0),
            quality_index: fixed!(0.0),
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

    pub fn progress(&self, setpoints: &SlipSetpoints) -> FixedValue {
        let duration = setpoints.phase_duration_ms(self.phase);
        if duration == 0 {
            fixed!(0.0)
        } else {
            FixedValue::from_u64_ratio(self.phase_elapsed_ms, duration)
                .clamp(fixed!(0.0), fixed!(1.0))
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

    pub fn apply_control_state(&mut self, control: super::BodyPreparationControlState) {
        let release_completed = self.phase == SlipPhase::Complete
            && control.phase == SlipPhase::Idle
            && control.batch_count > self.batch_count;
        if self.phase != control.phase {
            self.phase = control.phase;
            self.phase_elapsed_ms = 0;
        }
        self.running = control.running;
        self.batch_count = control.batch_count;
        if control.running {
            self.held = false;
        }
        if release_completed {
            self.release_pending = true;
        }
        if control.phase == SlipPhase::Faulted {
            self.held = false;
            self.measurements.transfer_flow_l_min = fixed!(0.0);
        }
    }

    pub fn advance_controlled(
        &mut self,
        elapsed_ms: u64,
        setpoints: &SlipSetpoints,
        fault: Option<BodyPreparationFault>,
        automatic_enabled: bool,
    ) -> super::BodyPreparationPhysicsFeedback {
        if !self.running || !automatic_enabled {
            return super::BodyPreparationPhysicsFeedback::default();
        }
        self.phase_elapsed_ms = self.phase_elapsed_ms.saturating_add(elapsed_ms);
        self.recalculate(setpoints);
        if let Some(trip) = self.evaluate_fault(fault) {
            self.trip();
            return super::BodyPreparationPhysicsFeedback {
                trip: Some(trip),
                phase_complete: false,
            };
        }
        let phase_complete = self.phase_elapsed_ms >= setpoints.phase_duration_ms(self.phase);
        if phase_complete && self.phase == SlipPhase::QualityCheck && !self.quality_released() {
            self.trip();
            return super::BodyPreparationPhysicsFeedback {
                trip: Some(BodyPreparationTrip::QualityReleaseDenied),
                phase_complete: false,
            };
        }
        super::BodyPreparationPhysicsFeedback {
            trip: None,
            phase_complete,
        }
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
            entrained_air_percent: fixed!(0.15),
            effects: rheology::downstream_effects(m, setpoints),
        }
    }

    pub fn quality_released(&self) -> bool {
        let m = self.measurements;
        (fixed!(1.78)..=fixed!(1.84)).contains(&m.density_kg_l)
            && (fixed!(400.0)..=fixed!(850.0)).contains(&m.high_shear_viscosity_mpa_s)
            && (fixed!(4.0)..=fixed!(7.5)).contains(&m.thixotropic_index)
            && (fixed!(7.0)..=fixed!(11.0)).contains(&m.residue_44um_percent)
            && m.water.acceptable_for_slip()
    }
}
