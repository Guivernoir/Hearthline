use hearthline_engine::{
    BodyPreparationFault, BodyPreparationSetpoints, PreparationTrain, SlipPhase,
};

use super::HmiSession;
use crate::hmi::schema::{
    GLAZE_PREPARATION_PHASES, RETURN_WATER_PHASES, SLIP_PREPARATION_PHASES,
    WATER_PREPARATION_PHASES,
};
use crate::hmi::{HmiAlarmSeverity, HmiBodyPreparationState, HmiParameter, HmiProcessState};

mod io;
mod parameters;
mod snapshot;

impl HmiSession {
    pub(super) fn tick_body_preparation(&mut self, elapsed_ms: u64) -> bool {
        let Some(process) = self.body_preparation.as_mut() else {
            return false;
        };
        let tick = process.tick(elapsed_ms);
        self.sequence = self.sequence.saturating_add(u64::from(tick.phase_changed));
        if let Some(trip) = tick.trip {
            self.raise_alarm(
                trip.code(),
                tick.train
                    .map_or("body-preparation", PreparationTrain::as_str),
                trip.message(),
                HmiAlarmSeverity::Trip,
            );
        }
        let pipelines = self
            .body_preparation
            .as_ref()
            .expect("Body Preparation runtime exists")
            .measurements()
            .pipelines;
        for (active, code, source, message) in [
            (
                pipelines.slip_to_forming.leak_detected,
                "BODY-SLIP-PIPELINE-LEAK",
                "area-01-slip-ld-01",
                "Slip transfer flow balance and pressure indicate a leak with entrained-air ingress.",
            ),
            (
                pipelines.water_to_slip.leak_detected,
                "BODY-WATER-SLIP-BRANCH-LEAK",
                "area-01-ws-ld-01",
                "The process-water branch to slip preparation has an abnormal flow balance.",
            ),
            (
                pipelines.water_to_glaze.leak_detected,
                "BODY-WATER-GLAZE-BRANCH-LEAK",
                "area-01-wg-ld-01",
                "The process-water branch to glaze preparation has an abnormal flow balance.",
            ),
            (
                pipelines.glaze_to_glazing.leak_detected,
                "BODY-GLAZE-PIPELINE-LEAK",
                "area-01-gl-ld-01",
                "The released-glaze transfer line has an abnormal flow and pressure balance.",
            ),
        ] {
            if active {
                self.raise_alarm(code, source, message, HmiAlarmSeverity::Warning);
            }
        }
        let pumps = self
            .body_preparation
            .as_ref()
            .expect("Body Preparation runtime exists")
            .measurements()
            .water_networks
            .pumps;
        for pump in pumps {
            if pump.heartbeat_ok {
                for alarm in &mut self.alarms {
                    if alarm.code == "BODY-WATER-PUMP-HEARTBEAT-LOST" && alarm.source == pump.id {
                        alarm.active = false;
                    }
                }
            } else {
                self.raise_alarm(
                    "BODY-WATER-PUMP-HEARTBEAT-LOST",
                    pump.id,
                    "Pump control heartbeat is stale. The unit is unavailable and requires maintenance review.",
                    HmiAlarmSeverity::Warning,
                );
            }
        }
        self.sync_body_preparation_io();
        if let Some(supervisory) = &mut self.supervisory {
            supervisory.tick(elapsed_ms, &self.signals);
        }
        true
    }

    pub(in crate::hmi) fn control_program_current_step(&self, fallback: i64) -> i64 {
        let Some(process) = self.body_preparation.as_ref() else {
            return fallback;
        };
        match process.phase() {
            SlipPhase::Idle => 0,
            SlipPhase::WaterCharge => 10,
            SlipPhase::DeflocculantCharge => 20,
            SlipPhase::BallClayCharge => 30,
            SlipPhase::KaolinCharge => 40,
            SlipPhase::FeldsparCharge => 50,
            SlipPhase::QuartzCharge => 60,
            SlipPhase::WetMixing => 70,
            SlipPhase::Screening => 80,
            SlipPhase::MagneticSeparation => 90,
            SlipPhase::Conditioning => 100,
            SlipPhase::QualityCheck => 110,
            SlipPhase::TemperatureTrim => 120,
            SlipPhase::Transfer => 130,
            SlipPhase::Complete => 140,
            SlipPhase::Faulted => 900,
        }
    }

    pub(in crate::hmi) fn body_preparation_snapshot(&self) -> Option<HmiBodyPreparationState> {
        snapshot::build(self.body_preparation.as_ref()?)
    }

    pub(in crate::hmi) fn body_process_snapshot(&self) -> Option<HmiProcessState> {
        let process = self.body_preparation.as_ref()?;
        let train = self.body_primary_train();
        let phases: &'static [_] = match train {
            PreparationTrain::Slip => &SLIP_PREPARATION_PHASES,
            PreparationTrain::Water => &WATER_PREPARATION_PHASES,
            PreparationTrain::ReturnWater => &RETURN_WATER_PHASES,
            PreparationTrain::Glaze => &GLAZE_PREPARATION_PHASES,
        };
        Some(HmiProcessState {
            model: match train {
                PreparationTrain::Slip => "ceramic-slip-preparation-cell",
                PreparationTrain::Water | PreparationTrain::ReturnWater => {
                    "ceramic-process-water-cell"
                }
                PreparationTrain::Glaze => "ceramic-glaze-preparation-cell",
            },
            phase: process.train_phase(train),
            running: process.train_running(train),
            phase_elapsed_ms: process.train_elapsed_ms(train),
            scan_count: process.scan_count(),
            cycle_count: process.train_cycle_count(train),
            fault: process
                .fault()
                .filter(|fault| self.body_fault_in_scope(*fault))
                .map(BodyPreparationFault::as_str),
            phases,
        })
    }

    pub(in crate::hmi) fn body_controls_train(&self, train: PreparationTrain) -> bool {
        match self.controller.id.as_str() {
            "area-01-vplc-01" => train == PreparationTrain::Slip,
            "area-01-wt-vplc-01" => train == PreparationTrain::Water,
            "area-01-rw-vplc-01" => train == PreparationTrain::ReturnWater,
            "area-01-gl-vplc-01" => train == PreparationTrain::Glaze,
            _ => false,
        }
    }

    pub(in crate::hmi) fn body_pump_in_scope(&self, pump_id: &str) -> bool {
        match self.controller.id.as_str() {
            "area-01-wd-vplc-01" => pump_id.starts_with("area-01-wd-pmp-"),
            "area-01-rc-vplc-01" => pump_id.starts_with("area-01-rc-pmp-"),
            _ => false,
        }
    }

    pub(in crate::hmi) fn body_fault_in_scope(&self, fault: BodyPreparationFault) -> bool {
        match fault {
            BodyPreparationFault::IngredientShortage
            | BodyPreparationFault::MixerOverload
            | BodyPreparationFault::ScreenBlocked
            | BodyPreparationFault::QualityOutOfSpec
            | BodyPreparationFault::TransferNoFlow
            | BodyPreparationFault::SlipPipelineLeak => {
                self.body_controls_train(PreparationTrain::Slip)
            }
            BodyPreparationFault::RawWaterQuality | BodyPreparationFault::WaterFilterBlocked => {
                self.body_controls_train(PreparationTrain::Water)
            }
            BodyPreparationFault::ReturnWaterContamination => {
                self.body_controls_train(PreparationTrain::ReturnWater)
            }
            BodyPreparationFault::WaterToSlipLeak | BodyPreparationFault::WaterToGlazeLeak => {
                self.controller.id == "area-01-wd-vplc-01"
            }
            BodyPreparationFault::GlazeMillOverload
            | BodyPreparationFault::GlazeQualityOutOfSpec
            | BodyPreparationFault::GlazePipelineLeak => {
                self.body_controls_train(PreparationTrain::Glaze)
            }
        }
    }

    pub(in crate::hmi) fn body_alarm_in_scope(&self, source: &str, code: &str) -> bool {
        if source == "area-01-intlk-01"
            || source == "area-01-wt-intlk-01"
            || source == "area-01-wd-intlk-01"
            || source == "area-01-rw-intlk-01"
            || source == "area-01-rc-intlk-01"
            || source == "area-01-gl-intlk-01"
        {
            return self.safety_in_scope(source);
        }
        match source {
            "slip" | "area-01-slip-ld-01" => self.body_controls_train(PreparationTrain::Slip),
            "water" => self.body_controls_train(PreparationTrain::Water),
            "return-water" => self.body_controls_train(PreparationTrain::ReturnWater),
            "glaze" | "area-01-gl-ld-01" => self.body_controls_train(PreparationTrain::Glaze),
            "area-01-ws-ld-01" => {
                self.body_controls_train(PreparationTrain::Slip)
                    || self.controller.id == "area-01-wd-vplc-01"
            }
            "area-01-wg-ld-01" => {
                self.controller.id == "area-01-wd-vplc-01"
                    || self.body_controls_train(PreparationTrain::Glaze)
            }
            _ if code == "BODY-WATER-PUMP-HEARTBEAT-LOST" => self.body_pump_in_scope(source),
            _ => !code.starts_with("BODY-"),
        }
    }

    fn body_primary_train(&self) -> PreparationTrain {
        if matches!(
            self.controller.id.as_str(),
            "area-01-wt-vplc-01" | "area-01-wd-vplc-01"
        ) {
            PreparationTrain::Water
        } else if matches!(
            self.controller.id.as_str(),
            "area-01-rw-vplc-01" | "area-01-rc-vplc-01"
        ) {
            PreparationTrain::ReturnWater
        } else if self.body_controls_train(PreparationTrain::Glaze) {
            PreparationTrain::Glaze
        } else {
            PreparationTrain::Slip
        }
    }

    pub(in crate::hmi) fn body_safety_ready(&self) -> bool {
        self.safety
            .iter()
            .filter(|state| self.safety_in_scope(&state.component_id))
            .all(|state| {
                !state.trip_latched
                    && state
                        .permissives
                        .iter()
                        .all(|permissive| permissive.satisfied)
            })
    }

    pub(in crate::hmi) fn apply_body_parameter(&mut self, id: &str, value: f64) -> bool {
        let Some(process) = &mut self.body_preparation else {
            return false;
        };
        let mut setpoints = process.setpoints();
        if !parameters::assign(&mut setpoints, id, value) {
            return false;
        }
        process.set_setpoints(setpoints)
    }

    pub(super) fn sync_body_preparation_io(&mut self) {
        io::sync(self);
    }
}

pub(in crate::hmi) fn setpoints_from_parameters(
    parameters: &[HmiParameter],
) -> Result<BodyPreparationSetpoints, crate::ConfigError> {
    Ok(parameters::from_parameters(parameters))
}
