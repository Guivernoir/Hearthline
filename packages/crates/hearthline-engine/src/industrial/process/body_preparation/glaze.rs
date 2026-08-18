use super::{
    BodyPreparationFault, BodyPreparationOutputs, BodyPreparationStartError, BodyPreparationTrip,
    GlazeBatch, SIMULATED_MS_PER_PROCESS_MINUTE, WaterQuality,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GlazePhase {
    Idle,
    WaterCharge,
    PowderWeighing,
    DispersantCharge,
    WetMilling,
    Screening,
    MagneticSeparation,
    PropertyAdjustment,
    QualityRelease,
    AgitatedStorage,
    Transfer,
    Complete,
    Faulted,
}

impl GlazePhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::WaterCharge => "glaze-water-charge",
            Self::PowderWeighing => "seven-powder-weighing",
            Self::DispersantCharge => "glaze-dispersant-charge",
            Self::WetMilling => "glaze-wet-milling",
            Self::Screening => "63-micrometre-screening",
            Self::MagneticSeparation => "glaze-magnetic-separation",
            Self::PropertyAdjustment => "density-flow-adjustment",
            Self::QualityRelease => "glaze-quality-release",
            Self::AgitatedStorage => "agitated-glaze-storage",
            Self::Transfer => "transfer-to-glazing",
            Self::Complete => "glaze-batch-complete",
            Self::Faulted => "faulted",
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::Idle | Self::Faulted => self,
            Self::WaterCharge => Self::PowderWeighing,
            Self::PowderWeighing => Self::DispersantCharge,
            Self::DispersantCharge => Self::WetMilling,
            Self::WetMilling => Self::Screening,
            Self::Screening => Self::MagneticSeparation,
            Self::MagneticSeparation => Self::PropertyAdjustment,
            Self::PropertyAdjustment => Self::QualityRelease,
            Self::QualityRelease => Self::AgitatedStorage,
            Self::AgitatedStorage => Self::Transfer,
            Self::Transfer => Self::Complete,
            Self::Complete => Self::Idle,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlazeSetpoints {
    pub kaolin_kg: f64,
    pub sodium_feldspar_kg: f64,
    pub quartz_kg: f64,
    pub calcite_kg: f64,
    pub dolomite_kg: f64,
    pub zinc_oxide_kg: f64,
    pub zircon_kg: f64,
    pub water_kg: f64,
    pub sodium_silicate_kg: f64,
    pub milling_minutes: f64,
    pub screen_micrometres: f64,
    pub target_density_kg_l: f64,
    pub target_ford_cup_seconds: f64,
}

impl Default for GlazeSetpoints {
    fn default() -> Self {
        Self {
            kaolin_kg: 30.0,
            sodium_feldspar_kg: 170.0,
            quartz_kg: 130.0,
            calcite_kg: 50.0,
            dolomite_kg: 35.0,
            zinc_oxide_kg: 6.25,
            zircon_kg: 78.75,
            water_kg: 250.0,
            sodium_silicate_kg: 2.5,
            milling_minutes: 180.0,
            screen_micrometres: 63.0,
            target_density_kg_l: 1.71,
            target_ford_cup_seconds: 25.0,
        }
    }
}

impl GlazeSetpoints {
    pub fn dry_mass_kg(self) -> f64 {
        self.kaolin_kg
            + self.sodium_feldspar_kg
            + self.quartz_kg
            + self.calcite_kg
            + self.dolomite_kg
            + self.zinc_oxide_kg
            + self.zircon_kg
    }

    pub fn total_batch_mass_kg(self) -> f64 {
        self.dry_mass_kg() + self.water_kg + self.sodium_silicate_kg
    }

    pub(super) fn phase_duration_ms(self, phase: GlazePhase) -> u64 {
        let minutes = match phase {
            GlazePhase::Idle | GlazePhase::Faulted => 0.0,
            GlazePhase::WaterCharge => 12.0,
            GlazePhase::PowderWeighing => 35.0,
            GlazePhase::DispersantCharge => 5.0,
            GlazePhase::WetMilling => self.milling_minutes,
            GlazePhase::Screening => 20.0,
            GlazePhase::MagneticSeparation => 5.0,
            GlazePhase::PropertyAdjustment => 20.0,
            GlazePhase::QualityRelease => 10.0,
            GlazePhase::AgitatedStorage => 30.0,
            GlazePhase::Transfer => 20.0,
            GlazePhase::Complete => 5.0,
        };
        (minutes * SIMULATED_MS_PER_PROCESS_MINUTE as f64 + 0.5) as u64
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlazeMeasurements {
    pub powder_mass_kg: f64,
    pub water_kg: f64,
    pub sodium_silicate_kg: f64,
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
    pub water: WaterQuality,
}

impl GlazeMeasurements {
    const fn empty() -> Self {
        Self {
            powder_mass_kg: 0.0,
            water_kg: 0.0,
            sodium_silicate_kg: 0.0,
            batch_mass_kg: 0.0,
            solids_percent: 0.0,
            density_kg_l: 1.0,
            ford_cup_seconds: 0.0,
            median_particle_um: 180.0,
            residue_63um_percent: 24.0,
            mill_energy_kwh_t: 0.0,
            storage_level_percent: 0.0,
            transfer_flow_l_min: 0.0,
            settling_risk_percent: 100.0,
            quality_index: 0.0,
            water: WaterQuality::treated_default(),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct GlazeRuntime {
    pub phase: GlazePhase,
    pub phase_elapsed_ms: u64,
    pub batch_count: u64,
    pub running: bool,
    pub held: bool,
    pub measurements: GlazeMeasurements,
    release_pending: bool,
}

impl GlazeRuntime {
    pub const fn new() -> Self {
        Self {
            phase: GlazePhase::Idle,
            phase_elapsed_ms: 0,
            batch_count: 0,
            running: false,
            held: false,
            measurements: GlazeMeasurements::empty(),
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
        self.measurements = GlazeMeasurements::empty();
        self.measurements.water = water;
        self.phase = GlazePhase::WaterCharge;
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
        if self.phase == GlazePhase::Faulted {
            self.phase = GlazePhase::Idle;
            self.phase_elapsed_ms = 0;
            self.running = false;
            self.held = false;
        }
    }

    pub fn tick(
        &mut self,
        elapsed_ms: u64,
        sp: &GlazeSetpoints,
        fault: Option<BodyPreparationFault>,
    ) -> (bool, Option<BodyPreparationTrip>) {
        if !self.running {
            return (false, None);
        }
        self.phase_elapsed_ms = self.phase_elapsed_ms.saturating_add(elapsed_ms);
        self.recalculate(sp);
        let trip = match fault {
            Some(BodyPreparationFault::GlazeMillOverload)
                if self.phase == GlazePhase::WetMilling =>
            {
                Some(BodyPreparationTrip::GlazeMillOverload)
            }
            Some(BodyPreparationFault::GlazeQualityOutOfSpec)
                if self.phase == GlazePhase::QualityRelease =>
            {
                Some(BodyPreparationTrip::GlazeQualityReleaseDenied)
            }
            _ => None,
        };
        if trip.is_some() {
            self.trip();
            return (true, trip);
        }
        let mut changed = false;
        while self.running && self.phase_elapsed_ms >= sp.phase_duration_ms(self.phase) {
            self.phase_elapsed_ms -= sp.phase_duration_ms(self.phase);
            self.phase = self.phase.next();
            changed = true;
            self.recalculate(sp);
            if self.phase == GlazePhase::QualityRelease && !self.quality_released() {
                self.trip();
                return (true, Some(BodyPreparationTrip::GlazeQualityReleaseDenied));
            }
            if self.phase == GlazePhase::Idle {
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
            GlazePhase::WetMilling => outputs.glaze_mill = "milling",
            GlazePhase::Screening => outputs.glaze_screen = "screening",
            GlazePhase::AgitatedStorage => outputs.glaze_agitator = "agitating",
            GlazePhase::Transfer => outputs.glaze_transfer_pump = "transferring",
            _ => {}
        }
    }

    pub fn take_release_pending(&mut self) -> bool {
        let pending = self.release_pending;
        self.release_pending = false;
        pending
    }

    pub fn release_batch(&self, _sp: &GlazeSetpoints) -> GlazeBatch {
        let m = self.measurements;
        GlazeBatch {
            batch_number: self.batch_count,
            density_kg_l: m.density_kg_l,
            ford_cup_seconds: m.ford_cup_seconds,
            residue_63um_percent: m.residue_63um_percent,
            median_particle_um: m.median_particle_um,
            solids_percent: m.solids_percent,
            water: m.water,
        }
    }

    pub fn quality_released(&self) -> bool {
        let m = self.measurements;
        (1.70..=1.72).contains(&m.density_kg_l)
            && (20.0..=30.0).contains(&m.ford_cup_seconds)
            && m.residue_63um_percent <= 2.0
            && m.water.acceptable_for_glaze()
    }

    fn recalculate(&mut self, sp: &GlazeSetpoints) {
        let p = progress(self.phase_elapsed_ms, sp.phase_duration_ms(self.phase));
        match self.phase {
            GlazePhase::WaterCharge => self.measurements.water_kg = sp.water_kg * p,
            GlazePhase::PowderWeighing => {
                self.measurements.water_kg = sp.water_kg;
                self.measurements.powder_mass_kg = sp.dry_mass_kg() * p;
            }
            GlazePhase::DispersantCharge => {
                self.full_powder_charge(sp);
                self.measurements.sodium_silicate_kg = sp.sodium_silicate_kg * p;
            }
            GlazePhase::WetMilling => {
                self.full_charge(sp);
                self.measurements.mill_energy_kwh_t = 12.0 * p;
                self.update_particle_size(p);
            }
            GlazePhase::Screening
            | GlazePhase::MagneticSeparation
            | GlazePhase::PropertyAdjustment
            | GlazePhase::QualityRelease
            | GlazePhase::AgitatedStorage
            | GlazePhase::Transfer
            | GlazePhase::Complete => {
                self.full_charge(sp);
                self.measurements.mill_energy_kwh_t = 12.0;
                self.update_particle_size(1.0);
                self.measurements.storage_level_percent = if matches!(
                    self.phase,
                    GlazePhase::AgitatedStorage | GlazePhase::Transfer | GlazePhase::Complete
                ) {
                    68.0
                } else {
                    0.0
                };
                self.measurements.transfer_flow_l_min = if self.phase == GlazePhase::Transfer {
                    42.0
                } else {
                    0.0
                };
            }
            GlazePhase::Idle | GlazePhase::Faulted => self.measurements.transfer_flow_l_min = 0.0,
        }
        self.update_properties(sp);
    }

    fn full_powder_charge(&mut self, sp: &GlazeSetpoints) {
        self.measurements.powder_mass_kg = sp.dry_mass_kg();
        self.measurements.water_kg = sp.water_kg;
    }

    fn full_charge(&mut self, sp: &GlazeSetpoints) {
        self.full_powder_charge(sp);
        self.measurements.sodium_silicate_kg = sp.sodium_silicate_kg;
    }

    fn update_particle_size(&mut self, maturity: f64) {
        self.measurements.median_particle_um = 180.0 / (1.0 + 3.3 * maturity);
        self.measurements.residue_63um_percent = 24.0 * (1.0 - maturity) + 1.0 * maturity;
    }

    fn update_properties(&mut self, sp: &GlazeSetpoints) {
        let m = &mut self.measurements;
        m.batch_mass_kg = m.powder_mass_kg + m.water_kg + m.sodium_silicate_kg;
        m.solids_percent = if m.batch_mass_kg <= 0.0 {
            0.0
        } else {
            m.powder_mass_kg / m.batch_mass_kg * 100.0
        };
        if m.powder_mass_kg <= 0.0 || m.water_kg <= 0.0 {
            return;
        }
        let maturity = (m.mill_energy_kwh_t / 12.0).clamp(0.0, 1.0);
        let unadjusted_density = 1.62 + 0.09 * maturity;
        m.density_kg_l = unadjusted_density;
        let dose_error = (m.sodium_silicate_kg / sp.dry_mass_kg() * 100.0 - 0.5) / 0.5;
        let water_penalty = 1.0 + (m.water.conductivity_us_cm - 200.0).max(0.0) * 0.0005;
        m.ford_cup_seconds =
            (25.0 + dose_error * dose_error * 14.0 + (1.0 - maturity) * 35.0) * water_penalty;
        let kaolin_percent = sp.kaolin_kg / sp.dry_mass_kg() * 100.0;
        m.settling_risk_percent = (34.0 - kaolin_percent * 3.0 - maturity * 12.0).clamp(3.0, 70.0);
        let density_score = window_score(m.density_kg_l, 1.70, 1.72);
        let flow_score = window_score(m.ford_cup_seconds, 20.0, 30.0);
        let residue_score = (1.0 - (m.residue_63um_percent - 1.0).max(0.0) / 3.0).clamp(0.0, 1.0);
        m.quality_index = (density_score + flow_score + residue_score) / 3.0 * 100.0;
    }

    fn trip(&mut self) {
        self.running = false;
        self.held = false;
        self.phase = GlazePhase::Faulted;
        self.phase_elapsed_ms = 0;
        self.measurements.transfer_flow_l_min = 0.0;
    }
}

fn progress(elapsed_ms: u64, duration_ms: u64) -> f64 {
    if duration_ms == 0 {
        0.0
    } else {
        (elapsed_ms as f64 / duration_ms as f64).clamp(0.0, 1.0)
    }
}

fn window_score(value: f64, minimum: f64, maximum: f64) -> f64 {
    let midpoint = (minimum + maximum) / 2.0;
    let half = (maximum - minimum) / 2.0;
    (1.0 - (value - midpoint).abs() / half.max(0.001)).clamp(0.0, 1.0)
}
