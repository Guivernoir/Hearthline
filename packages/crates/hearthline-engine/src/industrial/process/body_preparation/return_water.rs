use hearthline_model::{FixedValue, fixed};

use super::{
    BodyPreparationFault, BodyPreparationOutputs, BodyPreparationStartError, BodyPreparationTrip,
    ReturnWaterMeasurements, ReturnWaterPhase, WaterQuality, WaterSetpoints,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReturnStream {
    Body,
    Glaze,
}

#[derive(Clone, Debug)]
pub(super) struct ReturnWaterRuntime {
    pub phase: ReturnWaterPhase,
    pub phase_elapsed_ms: u64,
    pub cycle_count: u64,
    pub running: bool,
    pub held: bool,
    pub measurements: ReturnWaterMeasurements,
    stream: ReturnStream,
    cycle_sludge_start_kg: FixedValue,
    product_pending: bool,
}

impl ReturnWaterRuntime {
    pub const fn new() -> Self {
        Self {
            phase: ReturnWaterPhase::Idle,
            phase_elapsed_ms: 0,
            cycle_count: 0,
            running: false,
            held: false,
            measurements: ReturnWaterMeasurements {
                active_stream: "body-return",
                body_equalization_l: fixed!(1_200.0),
                glaze_equalization_l: fixed!(650.0),
                body_reuse_tank_l: fixed!(300.0),
                glaze_reuse_tank_l: fixed!(180.0),
                feed_flow_l_min: fixed!(0.0),
                clarified_flow_l_min: fixed!(0.0),
                sludge_cake_kg: fixed!(0.0),
                influent_turbidity_ntu: fixed!(480.0),
                effluent_turbidity_ntu: fixed!(1.5),
                body_reuse_quality: WaterQuality {
                    temperature_c: fixed!(28.0),
                    ph: fixed!(7.8),
                    turbidity_ntu: fixed!(1.2),
                    conductivity_us_cm: fixed!(230.0),
                    hardness_mg_l_caco3: fixed!(42.0),
                    suspended_solids_mg_l: fixed!(8.0),
                    glaze_contamination_percent: fixed!(0.0),
                    recovered_fraction_percent: fixed!(100.0),
                },
                glaze_reuse_quality: WaterQuality {
                    temperature_c: fixed!(27.0),
                    ph: fixed!(7.6),
                    turbidity_ntu: fixed!(2.0),
                    conductivity_us_cm: fixed!(340.0),
                    hardness_mg_l_caco3: fixed!(55.0),
                    suspended_solids_mg_l: fixed!(12.0),
                    glaze_contamination_percent: fixed!(1.0),
                    recovered_fraction_percent: fixed!(100.0),
                },
            },
            stream: ReturnStream::Body,
            cycle_sludge_start_kg: fixed!(0.0),
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
        self.stream =
            if self.measurements.body_equalization_l >= self.measurements.glaze_equalization_l {
                ReturnStream::Body
            } else {
                ReturnStream::Glaze
            };
        self.measurements.active_stream = if self.stream == ReturnStream::Body {
            "body-return"
        } else {
            "glaze-return"
        };
        let (feed_l, product_l, product_capacity_l) = match self.stream {
            ReturnStream::Body => (
                self.measurements.body_equalization_l,
                self.measurements.body_reuse_tank_l,
                fixed!(4_000.0),
            ),
            ReturnStream::Glaze => (
                self.measurements.glaze_equalization_l,
                self.measurements.glaze_reuse_tank_l,
                fixed!(3_000.0),
            ),
        };
        let recovered_l = setpoints.return_batch_l * fixed!(0.88);
        if feed_l < setpoints.return_batch_l || product_l + recovered_l > product_capacity_l {
            return Err(BodyPreparationStartError::WaterUnavailable);
        }
        self.cycle_sludge_start_kg = self.measurements.sludge_cake_kg;
        self.phase = ReturnWaterPhase::SegregatedCollection;
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
        if self.phase == ReturnWaterPhase::Faulted {
            self.phase = ReturnWaterPhase::Idle;
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
        if fault == Some(BodyPreparationFault::ReturnWaterContamination)
            && self.stream == ReturnStream::Body
            && self.phase == ReturnWaterPhase::QualityRouting
        {
            self.trip();
            return (
                true,
                Some(BodyPreparationTrip::ReturnWaterCrossContamination),
            );
        }
        let mut changed = false;
        while self.running && self.phase_elapsed_ms >= sp.return_phase_duration_ms(self.phase) {
            self.phase_elapsed_ms -= sp.return_phase_duration_ms(self.phase);
            self.phase = self.phase.next();
            changed = true;
            self.update_measurements(sp);
            if self.phase == ReturnWaterPhase::Complete && !self.product_pending {
                self.route_product(sp.return_batch_l * fixed!(0.88));
                self.product_pending = true;
            }
            if self.phase == ReturnWaterPhase::Idle {
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
            ReturnWaterPhase::Equalization => outputs.return_equalization = "mixing",
            ReturnWaterPhase::CoagulationFlocculation => outputs.flocculant_pump = "dosing",
            ReturnWaterPhase::LamellaClarification => outputs.clarifier = "settling",
            ReturnWaterPhase::FilterPress => outputs.filter_press = "dewatering",
            ReturnWaterPhase::QualityRouting => {
                outputs.reuse_diverter = if self.stream == ReturnStream::Body {
                    "body-reuse"
                } else {
                    "glaze-reuse"
                }
            }
            _ => {}
        }
    }

    fn update_measurements(&mut self, sp: &WaterSetpoints) {
        let p = progress(
            self.phase_elapsed_ms,
            sp.return_phase_duration_ms(self.phase),
        );
        self.measurements.feed_flow_l_min = fixed!(0.0);
        self.measurements.clarified_flow_l_min = fixed!(0.0);
        self.measurements.influent_turbidity_ntu = if self.stream == ReturnStream::Body {
            fixed!(480.0)
        } else {
            fixed!(720.0)
        };
        self.measurements.effluent_turbidity_ntu = match self.phase {
            ReturnWaterPhase::Idle
            | ReturnWaterPhase::SegregatedCollection
            | ReturnWaterPhase::Equalization => self.measurements.influent_turbidity_ntu,
            ReturnWaterPhase::CoagulationFlocculation => {
                self.measurements.influent_turbidity_ntu * (fixed!(1.0) - fixed!(0.45) * p)
            }
            ReturnWaterPhase::LamellaClarification => fixed!(264.0) - fixed!(245.0) * p,
            ReturnWaterPhase::FilterPress => fixed!(19.0) - fixed!(13.0) * p,
            ReturnWaterPhase::PolishingFiltration => fixed!(6.0) - fixed!(4.5) * p,
            _ => fixed!(1.5),
        };
        if matches!(
            self.phase,
            ReturnWaterPhase::CoagulationFlocculation
                | ReturnWaterPhase::LamellaClarification
                | ReturnWaterPhase::FilterPress
                | ReturnWaterPhase::PolishingFiltration
        ) {
            self.measurements.feed_flow_l_min = fixed!(18.0);
        }
        if self.phase == ReturnWaterPhase::LamellaClarification {
            self.measurements.clarified_flow_l_min = fixed!(15.8);
        }
        if self.phase == ReturnWaterPhase::FilterPress {
            let wet_cake_kg = sp.return_batch_l * fixed!(0.002);
            self.measurements.sludge_cake_kg = self.cycle_sludge_start_kg + wet_cake_kg * p;
        }
    }

    fn route_product(&mut self, volume_l: FixedValue) {
        match self.stream {
            ReturnStream::Body => {
                self.measurements.body_equalization_l = (self.measurements.body_equalization_l
                    - volume_l / fixed!(0.88))
                .max(fixed!(0.0));
                self.measurements.body_reuse_tank_l =
                    (self.measurements.body_reuse_tank_l + volume_l).min(fixed!(4_000.0));
            }
            ReturnStream::Glaze => {
                self.measurements.glaze_equalization_l = (self.measurements.glaze_equalization_l
                    - volume_l / fixed!(0.88))
                .max(fixed!(0.0));
                self.measurements.glaze_reuse_tank_l =
                    (self.measurements.glaze_reuse_tank_l + volume_l).min(fixed!(3_000.0));
            }
        }
    }

    fn trip(&mut self) {
        self.running = false;
        self.held = false;
        self.phase = ReturnWaterPhase::Faulted;
        self.phase_elapsed_ms = 0;
        self.measurements.feed_flow_l_min = fixed!(0.0);
        self.measurements.clarified_flow_l_min = fixed!(0.0);
    }
}

fn progress(elapsed_ms: u64, duration_ms: u64) -> FixedValue {
    if duration_ms == 0 {
        fixed!(0.0)
    } else {
        FixedValue::from_u64_ratio(elapsed_ms, duration_ms).clamp(fixed!(0.0), fixed!(1.0))
    }
}
