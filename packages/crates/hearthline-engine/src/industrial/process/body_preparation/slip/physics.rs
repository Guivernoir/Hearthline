use hearthline_model::{FixedValue, fixed};

use super::super::{BodyPreparationFault, BodyPreparationTrip, rheology};
use super::{SlipPhase, SlipRuntime, SlipSetpoints};

impl SlipRuntime {
    pub(super) fn recalculate(&mut self, setpoints: &SlipSetpoints) {
        let progress = self.progress(setpoints);
        match self.phase {
            SlipPhase::WaterCharge => {
                self.measurements.water_kg = setpoints.water_kg * progress;
            }
            SlipPhase::DeflocculantCharge => {
                self.measurements.water_kg = setpoints.water_kg;
                self.measurements.sodium_silicate_kg = setpoints.sodium_silicate_kg * progress;
            }
            SlipPhase::BallClayCharge => {
                self.measurements.water_kg = setpoints.water_kg;
                self.measurements.sodium_silicate_kg = setpoints.sodium_silicate_kg;
                self.measurements.ball_clay_kg = setpoints.ball_clay_kg * progress;
            }
            SlipPhase::KaolinCharge => {
                self.measurements.water_kg = setpoints.water_kg;
                self.measurements.sodium_silicate_kg = setpoints.sodium_silicate_kg;
                self.measurements.ball_clay_kg = setpoints.ball_clay_kg;
                self.measurements.kaolin_kg = setpoints.kaolin_kg * progress;
            }
            SlipPhase::FeldsparCharge => {
                self.measurements.water_kg = setpoints.water_kg;
                self.measurements.sodium_silicate_kg = setpoints.sodium_silicate_kg;
                self.measurements.ball_clay_kg = setpoints.ball_clay_kg;
                self.measurements.kaolin_kg = setpoints.kaolin_kg;
                self.measurements.feldspar_kg = setpoints.feldspar_kg * progress;
            }
            SlipPhase::QuartzCharge => {
                self.measurements.water_kg = setpoints.water_kg;
                self.measurements.sodium_silicate_kg = setpoints.sodium_silicate_kg;
                self.measurements.ball_clay_kg = setpoints.ball_clay_kg;
                self.measurements.kaolin_kg = setpoints.kaolin_kg;
                self.measurements.feldspar_kg = setpoints.feldspar_kg;
                self.measurements.quartz_kg = setpoints.quartz_kg * progress;
            }
            SlipPhase::WetMixing => {
                self.full_charge(setpoints);
                self.measurements.specific_energy_kwh_t = setpoints.mixing_energy_kwh_t * progress;
                self.measurements.structure_parameter = fixed!(0.9) - fixed!(0.75) * progress;
            }
            SlipPhase::Screening => {
                self.full_charge(setpoints);
                self.measurements.specific_energy_kwh_t = setpoints.mixing_energy_kwh_t;
                self.measurements.structure_parameter = fixed!(0.15);
            }
            SlipPhase::MagneticSeparation => self.full_mixed(setpoints, fixed!(0.2)),
            SlipPhase::Conditioning => {
                self.full_mixed(setpoints, fixed!(0.2) + fixed!(0.8) * progress)
            }
            SlipPhase::QualityCheck => self.full_mixed(setpoints, fixed!(1.0)),
            SlipPhase::TemperatureTrim => {
                self.full_mixed(setpoints, fixed!(1.0));
                let start = self.measurements.water.temperature_c;
                self.measurements.temperature_c =
                    start + (setpoints.target_temperature_c - start) * progress;
            }
            SlipPhase::Transfer | SlipPhase::Complete | SlipPhase::Idle => {
                if self.measurements.batch_mass_kg > fixed!(0.0) {
                    self.full_mixed(setpoints, fixed!(1.0));
                }
                if matches!(self.phase, SlipPhase::Transfer | SlipPhase::Complete) {
                    self.measurements.temperature_c = setpoints.target_temperature_c;
                }
                self.measurements.transfer_flow_l_min = if self.phase == SlipPhase::Transfer {
                    fixed!(68.0)
                } else {
                    fixed!(0.0)
                };
            }
            SlipPhase::Faulted => self.measurements.transfer_flow_l_min = fixed!(0.0),
        }
        rheology::update_slip_physics(&mut self.measurements, self.phase, setpoints);
    }

    fn full_charge(&mut self, setpoints: &SlipSetpoints) {
        self.measurements.ball_clay_kg = setpoints.ball_clay_kg;
        self.measurements.kaolin_kg = setpoints.kaolin_kg;
        self.measurements.feldspar_kg = setpoints.feldspar_kg;
        self.measurements.quartz_kg = setpoints.quartz_kg;
        self.measurements.water_kg = setpoints.water_kg;
        self.measurements.sodium_silicate_kg = setpoints.sodium_silicate_kg;
    }

    fn full_mixed(&mut self, setpoints: &SlipSetpoints, structure: FixedValue) {
        self.full_charge(setpoints);
        self.measurements.specific_energy_kwh_t = setpoints.mixing_energy_kwh_t;
        self.measurements.structure_parameter = structure;
    }

    pub(super) fn evaluate_fault(
        &self,
        fault: Option<BodyPreparationFault>,
    ) -> Option<BodyPreparationTrip> {
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

    pub(super) fn trip(&mut self) {
        self.running = false;
        self.held = false;
        self.phase = SlipPhase::Faulted;
        self.phase_elapsed_ms = 0;
        self.measurements.transfer_flow_l_min = fixed!(0.0);
    }
}
