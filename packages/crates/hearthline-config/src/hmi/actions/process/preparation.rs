use hearthline_engine::{BodyPreparationStartError, PreparationTrain};

use super::HmiSession;
use crate::hmi::{HmiActionReport, HmiActionStatus, HmiPreparationTrain};

impl HmiSession {
    pub(in crate::hmi) fn start_preparation_train(
        &mut self,
        train: HmiPreparationTrain,
    ) -> HmiActionReport {
        if self.body_preparation.is_none() || !self.has_permission("start-process") {
            return self.body_action_result(
                HmiActionStatus::Denied,
                "start-preparation-train",
                "This interface cannot start a material-preparation train.",
                "denied",
            );
        }
        let modeled_train = preparation_train(train);
        if !self.body_controls_train(modeled_train) {
            return self.body_action_result(
                HmiActionStatus::Denied,
                "start-preparation-train",
                "This local HMI does not own the selected preparation train.",
                "denied",
            );
        }
        let safety_ready = self.body_safety_ready();
        let start = self
            .body_preparation
            .as_mut()
            .expect("Body Preparation runtime exists")
            .start_train(modeled_train, safety_ready);
        if let Err(error) = start {
            return self.body_action_result(
                HmiActionStatus::Denied,
                "start-preparation-train",
                start_error_message(error),
                "denied",
            );
        }
        self.tick(0);
        self.body_action_result(
            HmiActionStatus::Applied,
            "start-preparation-train",
            "Selected material-preparation train started.",
            "applied",
        )
    }

    pub(in crate::hmi) fn hold_preparation_train(
        &mut self,
        train: HmiPreparationTrain,
    ) -> HmiActionReport {
        let modeled_train = preparation_train(train);
        if !self.body_controls_train(modeled_train) {
            return self.body_action_result(
                HmiActionStatus::Denied,
                "hold-preparation-train",
                "This local HMI does not own the selected preparation train.",
                "denied",
            );
        }
        let held = self
            .body_preparation
            .as_mut()
            .is_some_and(|process| process.hold_train(modeled_train));
        if !held {
            return self.body_action_result(
                HmiActionStatus::Denied,
                "hold-preparation-train",
                "Only a running material-preparation train can be held.",
                "denied",
            );
        }
        self.tick(0);
        self.body_action_result(
            HmiActionStatus::Applied,
            "hold-preparation-train",
            "Selected train held with automatic outputs in safe state.",
            "applied",
        )
    }

    pub(in crate::hmi) fn set_water_pump_failure(
        &mut self,
        pump_id: String,
        failed: bool,
    ) -> HmiActionReport {
        if !self.has_permission("inject-faults") || !self.body_pump_in_scope(&pump_id) {
            return self.body_action_result(
                HmiActionStatus::Denied,
                "set-water-pump-failure",
                "This HMI cannot inject a failure for the selected pump.",
                "denied",
            );
        }
        let applied = self
            .body_preparation
            .as_mut()
            .is_some_and(|process| process.set_water_pump_failed(&pump_id, failed));
        if !applied {
            return self.body_action_result(
                HmiActionStatus::Denied,
                "set-water-pump-failure",
                "The selected pump is not part of the simulated water networks.",
                "denied",
            );
        }
        self.tick(0);
        self.finish(
            HmiActionStatus::Applied,
            format!(
                "Pump heartbeat failure {} for {pump_id}.",
                if failed { "enabled" } else { "cleared" }
            ),
            "set-water-pump-failure",
            &pump_id,
            "applied",
            Vec::new(),
        )
    }

    pub(in crate::hmi) fn dispatch_water_pump_maintenance(
        &mut self,
        pump_id: String,
    ) -> HmiActionReport {
        if !self.has_permission("acknowledge-alarms") || !self.body_pump_in_scope(&pump_id) {
            return self.body_action_result(
                HmiActionStatus::Denied,
                "dispatch-water-pump-maintenance",
                "This HMI cannot dispatch maintenance for the selected pump.",
                "denied",
            );
        }
        let applied = self
            .body_preparation
            .as_mut()
            .is_some_and(|process| process.dispatch_water_pump_maintenance(&pump_id));
        if !applied {
            return self.body_action_result(
                HmiActionStatus::Denied,
                "dispatch-water-pump-maintenance",
                "Maintenance can be dispatched only after the pump heartbeat is lost.",
                "denied",
            );
        }
        self.tick(0);
        self.finish(
            HmiActionStatus::Applied,
            format!("Maintenance dispatch recorded for {pump_id}."),
            "dispatch-water-pump-maintenance",
            &pump_id,
            "dispatched",
            Vec::new(),
        )
    }
}

const fn preparation_train(train: HmiPreparationTrain) -> PreparationTrain {
    match train {
        HmiPreparationTrain::Slip => PreparationTrain::Slip,
        HmiPreparationTrain::Water => PreparationTrain::Water,
        HmiPreparationTrain::ReturnWater => PreparationTrain::ReturnWater,
        HmiPreparationTrain::Glaze => PreparationTrain::Glaze,
    }
}

const fn start_error_message(error: BodyPreparationStartError) -> &'static str {
    match error {
        BodyPreparationStartError::AlreadyRunning => "The selected train is already running.",
        BodyPreparationStartError::SafetyNotReady => {
            "Start requires healthy, reset process permissives."
        }
        BodyPreparationStartError::FaultActive => {
            "Clear the active simulation disturbance before starting."
        }
        BodyPreparationStartError::WaterUnavailable => {
            "The selected train could not reserve enough released process water."
        }
    }
}
