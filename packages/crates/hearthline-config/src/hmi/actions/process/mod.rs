use hearthline_engine::{BodyPreparationFault, FormingFault};

use super::super::builder::support::trace_entry;
use super::super::state::HmiSession;
use super::super::{HmiActionReport, HmiActionStatus, HmiControlMode, HmiProcessFault};

mod binding;
mod compiler;
mod control;
mod parser;
mod preparation;

pub(crate) use control::{ConfiguredControlProgram, load_control_program};

impl HmiSession {
    pub(super) fn start_process(&mut self) -> HmiActionReport {
        if self.body_preparation.is_some() {
            if !self.has_permission("start-process") {
                return self.body_action_result(
                    HmiActionStatus::Denied,
                    "start-process",
                    "This interface cannot start the batch sequence.",
                    "denied",
                );
            }
            if !self.body_controls_train(hearthline_engine::PreparationTrain::Slip) {
                return self.body_action_result(
                    HmiActionStatus::Denied,
                    "start-process",
                    "Cell-wide start is disabled; use this HMI's local train control.",
                    "denied",
                );
            }
            let safety_ready = self.body_safety_ready();
            let start = self
                .body_preparation
                .as_mut()
                .expect("Body Preparation runtime exists")
                .start(safety_ready);
            if let Err(error) = start {
                let message = match error {
                    hearthline_engine::BodyPreparationStartError::AlreadyRunning => {
                        "The batch sequence is already running."
                    }
                    hearthline_engine::BodyPreparationStartError::SafetyNotReady => {
                        "Batch start requires a healthy, reset safety and process-permissive state."
                    }
                    hearthline_engine::BodyPreparationStartError::FaultActive => {
                        "Clear the injected process disturbance before starting a batch."
                    }
                    hearthline_engine::BodyPreparationStartError::WaterUnavailable => {
                        "The train could not reserve enough released process water."
                    }
                };
                return self.body_action_result(
                    HmiActionStatus::Denied,
                    "start-process",
                    message,
                    "denied",
                );
            }
            self.tick(0);
            return self.body_action_result(
                HmiActionStatus::Applied,
                "start-process",
                "Body Preparation batch started from the selected development recipe.",
                "applied",
            );
        }
        self.finish(
            HmiActionStatus::Denied,
            "Cell-wide start is not available. Start each mould from its local HMI.".into(),
            "start-process",
            "forming-cell",
            "denied",
            Vec::new(),
        )
    }

    pub(super) fn hold_process(&mut self) -> HmiActionReport {
        if self.body_preparation.is_none() {
            return self.body_action_result(
                HmiActionStatus::Denied,
                "hold-process",
                "A batch hold is not available for this process model.",
                "denied",
            );
        }
        if !self.body_controls_train(hearthline_engine::PreparationTrain::Slip) {
            return self.body_action_result(
                HmiActionStatus::Denied,
                "hold-process",
                "Cell-wide hold is disabled; use this HMI's local train control.",
                "denied",
            );
        }
        let held = self
            .body_preparation
            .as_mut()
            .expect("Body Preparation runtime exists")
            .hold();
        if !held {
            return self.body_action_result(
                HmiActionStatus::Denied,
                "hold-process",
                "Only a running batch can be placed on hold.",
                "denied",
            );
        }
        self.tick(0);
        self.body_action_result(
            HmiActionStatus::Applied,
            "hold-process",
            "Batch held in its current phase; automatic outputs were moved to their safe states.",
            "applied",
        )
    }

    pub(super) fn start_mould(&mut self) -> HmiActionReport {
        let Some(target) = self.authorize_local_mould_production("start-mould") else {
            return self.mould_action_denied(
                "start-mould",
                "Only a mould-local HMI in auto can start its mould.",
            );
        };
        let safety_ready = self.mould_safety_ready(&target);
        let start = self
            .moulds
            .get_mut(&target)
            .expect("local mould runtime exists")
            .start(safety_ready);
        if let Err(message) = start {
            return self.mould_result(
                HmiActionStatus::Denied,
                "start-mould",
                &target,
                message,
                "denied",
            );
        }
        self.tick(0);
        self.mould_result(
            HmiActionStatus::Applied,
            "start-mould",
            &target,
            "Production enabled. The mould will repeat cycles until Stop or End is requested.",
            "applied",
        )
    }

    pub(super) fn stop_mould_after_phase(&mut self) -> HmiActionReport {
        let Some(target) = self.authorize_local_mould_production("stop-mould-after-phase") else {
            return self.mould_action_denied(
                "stop-mould-after-phase",
                "Only a mould-local HMI in auto can stop its mould.",
            );
        };
        let stop = self
            .moulds
            .get_mut(&target)
            .expect("local mould runtime exists")
            .stop_after_phase();
        if let Err(message) = stop {
            return self.mould_result(
                HmiActionStatus::Denied,
                "stop-mould-after-phase",
                &target,
                message,
                "denied",
            );
        }
        self.mould_result(
            HmiActionStatus::Applied,
            "stop-mould-after-phase",
            &target,
            "Stop requested. The current phase will finish before the mould pauses.",
            "applied",
        )
    }

    pub(super) fn end_mould_after_cycle(&mut self) -> HmiActionReport {
        let Some(target) = self.authorize_local_mould_production("end-mould-after-cycle") else {
            return self.mould_action_denied(
                "end-mould-after-cycle",
                "Only a mould-local HMI in auto can end its mould production.",
            );
        };
        let safety_ready = self.mould_safety_ready(&target);
        let end = self
            .moulds
            .get_mut(&target)
            .expect("local mould runtime exists")
            .end_after_cycle(safety_ready);
        if let Err(message) = end {
            return self.mould_result(
                HmiActionStatus::Denied,
                "end-mould-after-cycle",
                &target,
                message,
                "denied",
            );
        }
        self.mould_result(
            HmiActionStatus::Applied,
            "end-mould-after-cycle",
            &target,
            "End requested. The mould will complete its current cycle and then stop.",
            "applied",
        )
    }

    pub(super) fn reset_process(&mut self) -> HmiActionReport {
        if !self.has_permission("reset-safety") {
            return self.finish(
                HmiActionStatus::Denied,
                "This interface is not authorized to reset a process trip.".into(),
                "reset-process",
                "forming-cell",
                "denied",
                Vec::new(),
            );
        }
        if self.body_preparation.is_some() {
            let safety_ready = self.body_safety_ready();
            let reset = self
                .body_preparation
                .as_mut()
                .expect("Body Preparation runtime exists")
                .reset_after_trip(safety_ready);
            if !reset {
                return self.body_action_result(
                    HmiActionStatus::Denied,
                    "reset-process",
                    "Reset requires a cleared disturbance and healthy batch permissives.",
                    "denied",
                );
            }
            for alarm in &mut self.alarms {
                if alarm.code.starts_with("BODY-") {
                    alarm.active = false;
                }
            }
            self.tick(0);
            return self.body_action_result(
                HmiActionStatus::Applied,
                "reset-process",
                "Body Preparation sequence returned to idle.",
                "applied",
            );
        }
        let reset = self.reset_faulted_moulds();
        if reset.is_empty() {
            return self.finish(
                HmiActionStatus::Denied,
                "Reset requires a cleared fault and healthy mould safety state.".into(),
                "reset-process",
                "forming-cell",
                "denied",
                Vec::new(),
            );
        }
        self.clear_process_alarms_for(&reset);
        self.tick(0);
        self.finish(
            HmiActionStatus::Applied,
            format!("Reset {} faulted mould sequence(s).", reset.len()),
            "reset-process",
            "forming-cell",
            "applied",
            vec![trace_entry(
                &self.controller.id,
                "sequence reset",
                format!("returned {} mould sequence(s) to idle", reset.len()),
            )],
        )
    }

    pub(super) fn set_process_fault(
        &mut self,
        fault: HmiProcessFault,
        active: bool,
    ) -> HmiActionReport {
        if !self.has_permission("inject-faults") {
            return self.finish(
                HmiActionStatus::Denied,
                "Fault injection is restricted to the simulator supervisory interface.".into(),
                "set-process-fault",
                "forming-cell",
                "denied",
                Vec::new(),
            );
        }
        if self.body_preparation.is_some() {
            let modeled = match fault {
                HmiProcessFault::IngredientShortage => BodyPreparationFault::IngredientShortage,
                HmiProcessFault::MixerOverload => BodyPreparationFault::MixerOverload,
                HmiProcessFault::ScreenBlocked => BodyPreparationFault::ScreenBlocked,
                HmiProcessFault::QualityOutOfSpec => BodyPreparationFault::QualityOutOfSpec,
                HmiProcessFault::TransferNoFlow => BodyPreparationFault::TransferNoFlow,
                HmiProcessFault::RawWaterQuality => BodyPreparationFault::RawWaterQuality,
                HmiProcessFault::WaterFilterBlocked => BodyPreparationFault::WaterFilterBlocked,
                HmiProcessFault::ReturnWaterContamination => {
                    BodyPreparationFault::ReturnWaterContamination
                }
                HmiProcessFault::GlazeMillOverload => BodyPreparationFault::GlazeMillOverload,
                HmiProcessFault::GlazeQualityOutOfSpec => {
                    BodyPreparationFault::GlazeQualityOutOfSpec
                }
                HmiProcessFault::SlipPipelineLeak => BodyPreparationFault::SlipPipelineLeak,
                HmiProcessFault::WaterToSlipLeak => BodyPreparationFault::WaterToSlipLeak,
                HmiProcessFault::WaterToGlazeLeak => BodyPreparationFault::WaterToGlazeLeak,
                HmiProcessFault::GlazePipelineLeak => BodyPreparationFault::GlazePipelineLeak,
                _ => {
                    return self.body_action_result(
                        HmiActionStatus::Denied,
                        "set-process-fault",
                        "That disturbance belongs to the Forming process model.",
                        "denied",
                    );
                }
            };
            if !self.body_fault_in_scope(modeled) {
                return self.body_action_result(
                    HmiActionStatus::Denied,
                    "set-process-fault",
                    "That disturbance belongs to another Body Preparation control cell.",
                    "denied",
                );
            }
            self.body_preparation
                .as_mut()
                .expect("Body Preparation runtime exists")
                .set_fault(active.then_some(modeled));
            self.tick(0);
            if !active {
                let warning = match modeled {
                    BodyPreparationFault::SlipPipelineLeak => Some("BODY-SLIP-PIPELINE-LEAK"),
                    BodyPreparationFault::WaterToSlipLeak => Some("BODY-WATER-SLIP-BRANCH-LEAK"),
                    BodyPreparationFault::WaterToGlazeLeak => Some("BODY-WATER-GLAZE-BRANCH-LEAK"),
                    BodyPreparationFault::GlazePipelineLeak => Some("BODY-GLAZE-PIPELINE-LEAK"),
                    _ => None,
                };
                if let Some(code) = warning {
                    for alarm in &mut self.alarms {
                        if alarm.code == code {
                            alarm.active = false;
                        }
                    }
                }
            }
            let message = format!(
                "Body Preparation disturbance {} {}.",
                modeled.as_str(),
                if active { "enabled" } else { "cleared" }
            );
            return self.body_action_result(
                HmiActionStatus::Applied,
                "set-process-fault",
                &message,
                "applied",
            );
        }
        let modeled = match fault {
            HmiProcessFault::SlipSupplyLoss => FormingFault::SlipSupplyLoss,
            HmiProcessFault::CompressedAirLoss => FormingFault::CompressedAirLoss,
            HmiProcessFault::MouldOverpressure => FormingFault::MouldOverpressure,
            HmiProcessFault::VacuumLoss => FormingFault::VacuumLoss,
            HmiProcessFault::RobotPickupFailure => FormingFault::RobotPickupFailure,
            _ => {
                return self.body_action_result(
                    HmiActionStatus::Denied,
                    "set-process-fault",
                    "That disturbance belongs to the Body Preparation process model.",
                    "denied",
                );
            }
        };
        for mould in self.moulds.values_mut() {
            mould.set_fault(active.then_some(modeled));
        }
        self.tick(0);
        let target = modeled.as_str();
        self.finish(
            HmiActionStatus::Applied,
            format!(
                "Process disturbance {target} {} for all moulds.",
                if active { "enabled" } else { "cleared" }
            ),
            "set-process-fault",
            target,
            "applied",
            vec![trace_entry(
                &self.id,
                "simulation harness",
                format!("{target} active={active}"),
            )],
        )
    }

    pub(super) fn clear_process_alarms_for(&mut self, targets: &[String]) {
        for alarm in &mut self.alarms {
            if alarm.code.starts_with("FORMING-")
                && targets.iter().any(|target| target == &alarm.source)
            {
                alarm.active = false;
            }
        }
    }

    pub(super) fn authorize_local_mould_production(&self, permission: &str) -> Option<String> {
        let station = self.controller.stations.get(&self.id)?;
        (station.station_type == "mould-panel"
            && station.selected_mode == HmiControlMode::Auto
            && self.has_permission(permission))
        .then(|| station.target.clone())
    }

    fn mould_action_denied(&mut self, action: &str, message: &str) -> HmiActionReport {
        self.finish(
            HmiActionStatus::Denied,
            message.into(),
            action,
            "local-mould",
            "denied",
            Vec::new(),
        )
    }

    fn mould_result(
        &mut self,
        status: HmiActionStatus,
        action: &str,
        target: &str,
        message: &str,
        result: &str,
    ) -> HmiActionReport {
        self.finish(
            status,
            message.into(),
            action,
            target,
            result,
            vec![
                trace_entry(&self.id, "local operator request", message.into()),
                trace_entry(
                    &self.controller.id,
                    "mould sequence control",
                    format!("{action} accepted for {target}"),
                ),
            ],
        )
    }

    fn body_action_result(
        &mut self,
        status: HmiActionStatus,
        action: &str,
        message: &str,
        result: &str,
    ) -> HmiActionReport {
        self.finish(
            status,
            message.into(),
            action,
            "body-preparation-batch",
            result,
            vec![
                trace_entry(&self.id, "operator request", message.into()),
                trace_entry(
                    &self.controller.id,
                    "batch sequence control",
                    format!("{action} {result}"),
                ),
            ],
        )
    }
}
