use hearthline_model::{FixedValue, fixed};

use super::return_water::ReturnWaterRuntime;
use super::{
    BodyPreparationFault, BodyPreparationOutputs, BodyPreparationStartError, BodyPreparationTrip,
    SIMULATED_MS_PER_PROCESS_MINUTE, WaterQuality,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaterPhase {
    Idle,
    RawWaterIntake,
    Equalization,
    MediaFiltration,
    ActivatedCarbon,
    Softening,
    ReverseOsmosis,
    QualityRelease,
    ProductTransfer,
    Complete,
    Faulted,
}

impl WaterPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::RawWaterIntake => "raw-water-intake",
            Self::Equalization => "equalization",
            Self::MediaFiltration => "multimedia-filtration",
            Self::ActivatedCarbon => "activated-carbon",
            Self::Softening => "ion-exchange-softening",
            Self::ReverseOsmosis => "reverse-osmosis-blend",
            Self::QualityRelease => "process-water-quality-release",
            Self::ProductTransfer => "treated-water-transfer",
            Self::Complete => "treatment-cycle-complete",
            Self::Faulted => "faulted",
        }
    }

    pub(super) const fn next(self) -> Self {
        match self {
            Self::Idle | Self::Faulted => self,
            Self::RawWaterIntake => Self::Equalization,
            Self::Equalization => Self::MediaFiltration,
            Self::MediaFiltration => Self::ActivatedCarbon,
            Self::ActivatedCarbon => Self::Softening,
            Self::Softening => Self::ReverseOsmosis,
            Self::ReverseOsmosis => Self::QualityRelease,
            Self::QualityRelease => Self::ProductTransfer,
            Self::ProductTransfer => Self::Complete,
            Self::Complete => Self::Idle,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReturnWaterPhase {
    Idle,
    SegregatedCollection,
    Equalization,
    CoagulationFlocculation,
    LamellaClarification,
    FilterPress,
    PolishingFiltration,
    QualityRouting,
    Complete,
    Faulted,
}

impl ReturnWaterPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::SegregatedCollection => "segregated-return-collection",
            Self::Equalization => "return-equalization",
            Self::CoagulationFlocculation => "coagulation-flocculation",
            Self::LamellaClarification => "lamella-clarification",
            Self::FilterPress => "filter-press-dewatering",
            Self::PolishingFiltration => "polishing-filtration",
            Self::QualityRouting => "reuse-quality-routing",
            Self::Complete => "recovery-cycle-complete",
            Self::Faulted => "faulted",
        }
    }

    pub(in crate::industrial::process::body_preparation) const fn next(self) -> Self {
        match self {
            Self::Idle | Self::Faulted => self,
            Self::SegregatedCollection => Self::Equalization,
            Self::Equalization => Self::CoagulationFlocculation,
            Self::CoagulationFlocculation => Self::LamellaClarification,
            Self::LamellaClarification => Self::FilterPress,
            Self::FilterPress => Self::PolishingFiltration,
            Self::PolishingFiltration => Self::QualityRouting,
            Self::QualityRouting => Self::Complete,
            Self::Complete => Self::Idle,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterSetpoints {
    pub treatment_batch_l: FixedValue,
    pub ro_recovery_percent: FixedValue,
    pub target_conductivity_us_cm: FixedValue,
    pub target_hardness_mg_l: FixedValue,
    pub target_turbidity_ntu: FixedValue,
    pub maximum_body_reuse_percent: FixedValue,
    pub maximum_glaze_reuse_percent: FixedValue,
    pub return_batch_l: FixedValue,
}

impl Default for WaterSetpoints {
    fn default() -> Self {
        Self {
            treatment_batch_l: fixed!(2_000.0),
            ro_recovery_percent: fixed!(75.0),
            target_conductivity_us_cm: fixed!(80.0),
            target_hardness_mg_l: fixed!(8.0),
            target_turbidity_ntu: fixed!(0.25),
            maximum_body_reuse_percent: fixed!(35.0),
            maximum_glaze_reuse_percent: fixed!(40.0),
            return_batch_l: fixed!(600.0),
        }
    }
}

impl WaterSetpoints {
    pub(in crate::industrial::process::body_preparation) fn phase_duration_ms(
        self,
        phase: WaterPhase,
    ) -> u64 {
        let minutes = match phase {
            WaterPhase::Idle | WaterPhase::Faulted => fixed!(0.0),
            WaterPhase::RawWaterIntake => fixed!(20.0),
            WaterPhase::Equalization => fixed!(30.0),
            WaterPhase::MediaFiltration => fixed!(35.0),
            WaterPhase::ActivatedCarbon => fixed!(25.0),
            WaterPhase::Softening => fixed!(30.0),
            WaterPhase::ReverseOsmosis => fixed!(80.0),
            WaterPhase::QualityRelease => fixed!(10.0),
            WaterPhase::ProductTransfer => fixed!(20.0),
            WaterPhase::Complete => fixed!(5.0),
        };
        (minutes * FixedValue::from_integer(SIMULATED_MS_PER_PROCESS_MINUTE as i64)).round_to_u64()
    }

    pub(in crate::industrial::process::body_preparation) fn return_phase_duration_ms(
        self,
        phase: ReturnWaterPhase,
    ) -> u64 {
        let minutes = match phase {
            ReturnWaterPhase::Idle | ReturnWaterPhase::Faulted => fixed!(0.0),
            ReturnWaterPhase::SegregatedCollection => fixed!(20.0),
            ReturnWaterPhase::Equalization => fixed!(45.0),
            ReturnWaterPhase::CoagulationFlocculation => fixed!(30.0),
            ReturnWaterPhase::LamellaClarification => fixed!(60.0),
            ReturnWaterPhase::FilterPress => fixed!(90.0),
            ReturnWaterPhase::PolishingFiltration => fixed!(30.0),
            ReturnWaterPhase::QualityRouting => fixed!(15.0),
            ReturnWaterPhase::Complete => fixed!(5.0),
        };
        (minutes * FixedValue::from_integer(SIMULATED_MS_PER_PROCESS_MINUTE as i64)).round_to_u64()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterMeasurements {
    pub raw_tank_l: FixedValue,
    pub treated_tank_l: FixedValue,
    pub feed_flow_l_min: FixedValue,
    pub permeate_flow_l_min: FixedValue,
    pub reject_flow_l_min: FixedValue,
    pub media_filter_dp_bar: FixedValue,
    pub ro_recovery_percent: FixedValue,
    pub raw: WaterQuality,
    pub product: WaterQuality,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReturnWaterMeasurements {
    pub active_stream: &'static str,
    pub body_equalization_l: FixedValue,
    pub glaze_equalization_l: FixedValue,
    pub body_reuse_tank_l: FixedValue,
    pub glaze_reuse_tank_l: FixedValue,
    pub feed_flow_l_min: FixedValue,
    pub clarified_flow_l_min: FixedValue,
    pub sludge_cake_kg: FixedValue,
    pub influent_turbidity_ntu: FixedValue,
    pub effluent_turbidity_ntu: FixedValue,
    pub body_reuse_quality: WaterQuality,
    pub glaze_reuse_quality: WaterQuality,
}

#[derive(Clone, Debug)]
pub(in crate::industrial::process::body_preparation) struct WaterRuntime {
    pub phase: WaterPhase,
    pub phase_elapsed_ms: u64,
    pub cycle_count: u64,
    pub running: bool,
    pub held: bool,
    pub measurements: WaterMeasurements,
    tank_quality: WaterQuality,
    product_pending: bool,
}

impl WaterRuntime {
    pub const fn new() -> Self {
        Self {
            phase: WaterPhase::Idle,
            phase_elapsed_ms: 0,
            cycle_count: 0,
            running: false,
            held: false,
            measurements: WaterMeasurements {
                raw_tank_l: fixed!(4_500.0),
                treated_tank_l: fixed!(2_500.0),
                feed_flow_l_min: fixed!(0.0),
                permeate_flow_l_min: fixed!(0.0),
                reject_flow_l_min: fixed!(0.0),
                media_filter_dp_bar: fixed!(0.12),
                ro_recovery_percent: fixed!(0.0),
                raw: WaterQuality::raw_default(),
                product: WaterQuality::treated_default(),
            },
            tank_quality: WaterQuality::treated_default(),
            product_pending: false,
        }
    }

    pub fn start(&mut self, setpoints: &WaterSetpoints) -> Result<(), BodyPreparationStartError> {
        if self.running {
            return Err(BodyPreparationStartError::AlreadyRunning);
        }
        if self.held {
            self.running = true;
            self.held = false;
            return Ok(());
        }
        let product_l = setpoints.treatment_batch_l * setpoints.ro_recovery_percent / fixed!(100.0);
        if self.measurements.raw_tank_l < setpoints.treatment_batch_l
            || self.measurements.treated_tank_l + product_l > fixed!(8_000.0)
        {
            return Err(BodyPreparationStartError::WaterUnavailable);
        }
        self.phase = WaterPhase::RawWaterIntake;
        self.phase_elapsed_ms = 0;
        self.running = true;
        self.product_pending = false;
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
        if self.phase == WaterPhase::Faulted {
            self.phase = WaterPhase::Idle;
            self.phase_elapsed_ms = 0;
            self.running = false;
            self.held = false;
        }
    }

    pub fn tick(
        &mut self,
        elapsed_ms: u64,
        sp: &WaterSetpoints,
        fault: Option<BodyPreparationFault>,
    ) -> (bool, Option<BodyPreparationTrip>) {
        if !self.running {
            return (false, None);
        }
        self.phase_elapsed_ms = self.phase_elapsed_ms.saturating_add(elapsed_ms);
        self.update_measurements(sp);
        let trip = match fault {
            Some(BodyPreparationFault::WaterFilterBlocked)
                if self.phase == WaterPhase::MediaFiltration =>
            {
                Some(BodyPreparationTrip::WaterQualityRejected)
            }
            Some(BodyPreparationFault::RawWaterQuality)
                if self.phase == WaterPhase::QualityRelease =>
            {
                Some(BodyPreparationTrip::WaterQualityRejected)
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
            self.update_measurements(sp);
            if self.phase == WaterPhase::QualityRelease
                && !self.measurements.product.acceptable_for_slip()
            {
                self.trip();
                return (true, Some(BodyPreparationTrip::WaterQualityRejected));
            }
            if self.phase == WaterPhase::Complete && !self.product_pending {
                let product = sp.treatment_batch_l * sp.ro_recovery_percent / fixed!(100.0);
                let stored = self.measurements.treated_tank_l;
                let total = stored + product;
                self.tank_quality = self
                    .tank_quality
                    .blend(self.measurements.product, product / total.max(fixed!(1.0)));
                self.measurements.treated_tank_l = total.min(fixed!(8_000.0));
                self.measurements.raw_tank_l =
                    (self.measurements.raw_tank_l - sp.treatment_batch_l).max(fixed!(0.0));
                self.product_pending = true;
            }
            if self.phase == WaterPhase::Idle {
                self.running = false;
                self.cycle_count = self.cycle_count.saturating_add(1);
            }
        }
        (changed, None)
    }

    pub fn apply_outputs(&self, outputs: &mut BodyPreparationOutputs) {
        if !self.running {
            return;
        }
        match self.phase {
            WaterPhase::RawWaterIntake => outputs.raw_water_pump = "filling",
            WaterPhase::MediaFiltration => outputs.media_filter = "filtering",
            WaterPhase::Softening => outputs.softener = "softening",
            WaterPhase::ReverseOsmosis => outputs.reverse_osmosis = "producing",
            WaterPhase::ProductTransfer => outputs.raw_water_pump = "product-transfer",
            _ => {}
        }
    }

    pub fn reserve_slip_water(
        &mut self,
        amount_kg: FixedValue,
        reuse_limit: FixedValue,
        returns: &mut ReturnWaterRuntime,
    ) -> Option<WaterQuality> {
        let reuse =
            (amount_kg * reuse_limit / fixed!(100.0)).min(returns.measurements.body_reuse_tank_l);
        let fresh = amount_kg - reuse;
        if fresh > self.measurements.treated_tank_l {
            return None;
        }
        let reuse_quality = returns.measurements.body_reuse_quality;
        if reuse > fixed!(0.0) && !reuse_quality.acceptable_for_slip() {
            return None;
        }
        self.measurements.treated_tank_l -= fresh;
        returns.measurements.body_reuse_tank_l -= reuse;
        Some(
            self.tank_quality
                .blend(reuse_quality, reuse / amount_kg.max(fixed!(1.0))),
        )
    }

    pub fn reserve_glaze_water(
        &mut self,
        amount_kg: FixedValue,
        reuse_limit: FixedValue,
        returns: &mut ReturnWaterRuntime,
    ) -> Option<WaterQuality> {
        let reuse =
            (amount_kg * reuse_limit / fixed!(100.0)).min(returns.measurements.glaze_reuse_tank_l);
        let fresh = amount_kg - reuse;
        if fresh > self.measurements.treated_tank_l {
            return None;
        }
        let reuse_quality = returns.measurements.glaze_reuse_quality;
        if reuse > fixed!(0.0) && !reuse_quality.acceptable_for_glaze() {
            return None;
        }
        self.measurements.treated_tank_l -= fresh;
        returns.measurements.glaze_reuse_tank_l -= reuse;
        Some(
            self.tank_quality
                .blend(reuse_quality, reuse / amount_kg.max(fixed!(1.0))),
        )
    }

    fn update_measurements(&mut self, sp: &WaterSetpoints) {
        let p = progress(self.phase_elapsed_ms, sp.phase_duration_ms(self.phase));
        let raw = self.measurements.raw;
        self.measurements.feed_flow_l_min = fixed!(0.0);
        self.measurements.permeate_flow_l_min = fixed!(0.0);
        self.measurements.reject_flow_l_min = fixed!(0.0);
        self.measurements.product = match self.phase {
            WaterPhase::Idle => self.tank_quality,
            WaterPhase::RawWaterIntake | WaterPhase::Equalization => raw,
            WaterPhase::MediaFiltration => WaterQuality {
                turbidity_ntu: raw.turbidity_ntu - fixed!(6.8) * p,
                suspended_solids_mg_l: raw.suspended_solids_mg_l - fixed!(15.0) * p,
                ..raw
            },
            WaterPhase::ActivatedCarbon => WaterQuality {
                turbidity_ntu: fixed!(1.2) - fixed!(0.35) * p,
                suspended_solids_mg_l: fixed!(3.0) - fixed!(1.0) * p,
                ..raw
            },
            WaterPhase::Softening => WaterQuality {
                turbidity_ntu: fixed!(0.85),
                suspended_solids_mg_l: fixed!(2.0),
                hardness_mg_l_caco3: raw.hardness_mg_l_caco3
                    - (raw.hardness_mg_l_caco3 - fixed!(18.0)) * p,
                ..raw
            },
            WaterPhase::ReverseOsmosis => WaterQuality {
                turbidity_ntu: fixed!(0.85) - (fixed!(0.85) - sp.target_turbidity_ntu) * p,
                suspended_solids_mg_l: fixed!(2.0) - fixed!(1.5) * p,
                hardness_mg_l_caco3: fixed!(18.0) - (fixed!(18.0) - sp.target_hardness_mg_l) * p,
                conductivity_us_cm: raw.conductivity_us_cm
                    - (raw.conductivity_us_cm - sp.target_conductivity_us_cm) * p,
                ph: fixed!(7.0),
                temperature_c: fixed!(25.0),
                ..raw
            },
            WaterPhase::QualityRelease | WaterPhase::ProductTransfer | WaterPhase::Complete => {
                released_product(sp)
            }
            WaterPhase::Faulted => self.tank_quality,
        };
        if self.phase == WaterPhase::MediaFiltration {
            self.measurements.feed_flow_l_min = fixed!(42.0);
            self.measurements.media_filter_dp_bar = fixed!(0.12) + fixed!(0.25) * p;
        }
        if self.phase == WaterPhase::ReverseOsmosis {
            self.measurements.feed_flow_l_min = fixed!(32.0);
            self.measurements.ro_recovery_percent = sp.ro_recovery_percent * p;
            self.measurements.permeate_flow_l_min =
                fixed!(32.0) * sp.ro_recovery_percent / fixed!(100.0);
            self.measurements.reject_flow_l_min =
                fixed!(32.0) - self.measurements.permeate_flow_l_min;
        }
    }

    fn trip(&mut self) {
        self.running = false;
        self.held = false;
        self.phase = WaterPhase::Faulted;
        self.phase_elapsed_ms = 0;
        self.measurements.feed_flow_l_min = fixed!(0.0);
        self.measurements.permeate_flow_l_min = fixed!(0.0);
        self.measurements.reject_flow_l_min = fixed!(0.0);
    }
}

fn released_product(sp: &WaterSetpoints) -> WaterQuality {
    WaterQuality {
        turbidity_ntu: sp.target_turbidity_ntu,
        conductivity_us_cm: sp.target_conductivity_us_cm,
        hardness_mg_l_caco3: sp.target_hardness_mg_l,
        ..WaterQuality::treated_default()
    }
}

fn progress(elapsed_ms: u64, duration_ms: u64) -> FixedValue {
    if duration_ms == 0 {
        fixed!(0.0)
    } else {
        FixedValue::from_u64_ratio(elapsed_ms, duration_ms).clamp(fixed!(0.0), fixed!(1.0))
    }
}
