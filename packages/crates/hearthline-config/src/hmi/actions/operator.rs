use sha2::{Digest, Sha256};

use super::super::builder::support::trace_entry;
use super::super::state::HmiSession;
use super::super::{HmiActionReport, HmiActionStatus, HmiControlMode};

impl HmiSession {
    pub(super) fn set_control_mode(
        &mut self,
        mode: HmiControlMode,
        password: Option<String>,
    ) -> HmiActionReport {
        if self
            .local_mould_target()
            .is_some_and(|target| self.mould_running(target))
            || (self.local_mould_target().is_none() && self.any_mould_running())
        {
            return self.mode_result(
                HmiActionStatus::Denied,
                mode,
                "Mode change denied while the automatic sequence is running.",
                "denied",
            );
        }
        let Some(station) = self.controller.stations.get_mut(&self.id) else {
            return self.mode_result(
                HmiActionStatus::Denied,
                mode,
                "This interface has no configured mode selector.",
                "denied",
            );
        };
        if !station.positions.contains(&mode) {
            return self.mode_result(
                HmiActionStatus::Denied,
                mode,
                "The requested selector position is not configured.",
                "denied",
            );
        }
        if mode == HmiControlMode::Setup {
            let valid = station
                .setup_password_sha256
                .as_ref()
                .zip(password.as_deref())
                .is_some_and(|(expected, supplied)| digest(supplied) == *expected);
            if !valid {
                station.setup_authenticated = false;
                return self.mode_result(
                    HmiActionStatus::Denied,
                    mode,
                    "Setup mode requires valid maintenance credentials.",
                    "denied",
                );
            }
            station.setup_authenticated = true;
        } else {
            station.setup_authenticated = false;
        }
        station.selected_mode = mode;
        if mode == HmiControlMode::Auto
            && let Some(robot) = &mut self.robot
        {
            robot.set_motion_enabled(false);
        }
        let message = if mode == HmiControlMode::Setup {
            "Setup mode active: process-sensor permissives are bypassed; emergency stops and hardwired travel limits remain authoritative."
        } else {
            "Control mode changed."
        };
        self.mode_result(HmiActionStatus::Applied, mode, message, "applied")
    }

    pub(super) fn set_parameter(&mut self, parameter_id: String, value: f64) -> HmiActionReport {
        if !self.has_permission("configure-parameters") {
            return self.parameter_result(
                HmiActionStatus::Denied,
                &parameter_id,
                "This interface cannot change machine parameters.",
                "denied",
            );
        }
        if self.any_mould_running() {
            return self.parameter_result(
                HmiActionStatus::Denied,
                &parameter_id,
                "Parameter changes are inhibited while the sequence is running.",
                "denied",
            );
        }
        let Some(parameter) = self
            .controller
            .parameters
            .iter_mut()
            .find(|parameter| parameter.id == parameter_id)
        else {
            return self.parameter_result(
                HmiActionStatus::Denied,
                &parameter_id,
                "Unknown machine parameter.",
                "denied",
            );
        };
        if !value.is_finite() || value < parameter.minimum || value > parameter.maximum {
            return self.parameter_result(
                HmiActionStatus::Denied,
                &parameter_id,
                "Parameter value is outside its configured engineering range.",
                "denied",
            );
        }
        parameter.value = value;
        let target = parameter.target.clone();
        if !self
            .moulds
            .get_mut(&target)
            .is_some_and(|mould| mould.apply_parameter(&parameter_id, value))
        {
            return self.parameter_result(
                HmiActionStatus::Denied,
                &parameter_id,
                "Parameter is not bound to the configured mould runtime.",
                "denied",
            );
        }
        self.parameter_result(
            HmiActionStatus::Applied,
            &parameter_id,
            "Machine parameter updated.",
            "applied",
        )
    }

    pub(super) fn select_recipe(&mut self, recipe_id: String) -> HmiActionReport {
        if !self.has_permission("select-recipe") {
            return self.recipe_result(
                HmiActionStatus::Denied,
                &recipe_id,
                "This interface cannot select recipes.",
                "denied",
            );
        }
        if self.any_mould_running() {
            return self.recipe_result(
                HmiActionStatus::Denied,
                &recipe_id,
                "Recipe changes are inhibited while the sequence is running.",
                "denied",
            );
        }
        if !self
            .controller
            .recipes
            .iter()
            .any(|recipe| recipe.id == recipe_id)
        {
            return self.recipe_result(
                HmiActionStatus::Denied,
                &recipe_id,
                "Unknown recipe.",
                "denied",
            );
        }
        self.controller.active_recipe = Some(recipe_id.clone());
        self.recipe_result(
            HmiActionStatus::Applied,
            &recipe_id,
            "Active recipe changed. The Structured Text control program was not modified.",
            "applied",
        )
    }

    pub(super) fn authorize_manual_command(&self, tag: &str) -> Result<(), String> {
        let Some(station) = self.controller.stations.get(&self.id) else {
            return Ok(());
        };
        match station.station_type.as_str() {
            "robot-joystick"
                if tag == "area-02-robot-01-command"
                    && !self
                        .robot
                        .as_ref()
                        .is_some_and(|robot| robot.motion_enabled()) =>
            {
                Err("Robot motion command denied: pendant motion enable is not active.".into())
            }
            "mould-panel" | "robot-joystick"
                if !matches!(
                    station.selected_mode,
                    HmiControlMode::Manual | HmiControlMode::Setup
                ) =>
            {
                Err("Manual command denied: move the keyed selector out of auto.".into())
            }
            "machine-pc" => {
                let Some(target) = mould_target_from_tag(tag) else {
                    return Ok(());
                };
                let manual = self.controller.stations.values().any(|candidate| {
                    candidate.station_type == "mould-panel"
                        && candidate.target == target
                        && candidate.selected_mode == HmiControlMode::Manual
                });
                if manual {
                    Ok(())
                } else {
                    Err(format!(
                        "Manual valve command denied: {target} must be selected to manual at its local HMI."
                    ))
                }
            }
            _ => Ok(()),
        }
    }

    pub(super) fn local_mould_target(&self) -> Option<&str> {
        self.controller
            .stations
            .get(&self.id)
            .filter(|station| station.station_type == "mould-panel")
            .map(|station| station.target.as_str())
    }

    fn mode_result(
        &mut self,
        status: HmiActionStatus,
        mode: HmiControlMode,
        message: &str,
        result: &str,
    ) -> HmiActionReport {
        self.finish(
            status,
            message.into(),
            "set-control-mode",
            mode.as_str(),
            result,
            vec![trace_entry(
                &self.id,
                "keyed selector",
                format!("requested {} mode", mode.as_str()),
            )],
        )
    }

    fn parameter_result(
        &mut self,
        status: HmiActionStatus,
        target: &str,
        message: &str,
        result: &str,
    ) -> HmiActionReport {
        self.finish(
            status,
            message.into(),
            "set-parameter",
            target,
            result,
            vec![trace_entry(&self.id, "parameter service", message.into())],
        )
    }

    fn recipe_result(
        &mut self,
        status: HmiActionStatus,
        target: &str,
        message: &str,
        result: &str,
    ) -> HmiActionReport {
        self.finish(
            status,
            message.into(),
            "select-recipe",
            target,
            result,
            vec![trace_entry(&self.id, "recipe service", message.into())],
        )
    }
}

fn mould_target_from_tag(tag: &str) -> Option<String> {
    if matches!(
        tag,
        "area-02-water-01-command" | "area-02-air-01-command" | "area-02-vac-01-command"
    ) {
        return Some("mould-01".into());
    }
    (1..=4).find_map(|index| {
        let prefix = format!("area-02-m{index:02}-");
        tag.starts_with(&prefix)
            .then(|| format!("mould-{index:02}"))
    })
}

fn digest(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
