use hearthline_engine::{RobotCartesianAxis, RobotMotionError};
use hearthline_model::ComponentId;

use super::super::builder::support::trace_entry;
use super::super::state::{HmiSession, robot_pose};
use super::super::{
    HmiActionReport, HmiActionStatus, HmiControlMode, HmiRobotAxis, HmiRobotCoordinateSystem,
    HmiRobotPose,
};
use crate::appliance::source_revision;

impl HmiSession {
    pub(super) fn set_robot_motion_enable(&mut self, enabled: bool) -> HmiActionReport {
        if enabled && let Err(message) = self.require_robot_manual_authority(false) {
            return self.robot_result(HmiActionStatus::Denied, "motion-enable", &message, "denied");
        }
        let Some(robot) = &mut self.robot else {
            return self.robot_result(
                HmiActionStatus::Denied,
                "motion-enable",
                "No robot motion profile is configured.",
                "denied",
            );
        };
        robot.set_motion_enabled(enabled);
        self.robot_result(
            HmiActionStatus::Applied,
            "motion-enable",
            if enabled {
                "Pendant motion enable is active."
            } else {
                "Pendant motion enable is released; active manual motion stopped."
            },
            "applied",
        )
    }

    pub(super) fn move_robot(
        &mut self,
        target: HmiRobotPose,
        speed_percent: f64,
    ) -> HmiActionReport {
        if let Err(message) = self.require_robot_manual_authority(true) {
            return self.robot_result(
                HmiActionStatus::Denied,
                "cartesian-target",
                &message,
                "denied",
            );
        }
        let result = self
            .robot
            .as_mut()
            .expect("robot authority requires runtime")
            .command_pose(robot_pose(target), speed_percent);
        self.motion_result(result, "cartesian-target")
    }

    pub(super) fn move_robot_to_position(
        &mut self,
        position_id: String,
        speed_percent: f64,
    ) -> HmiActionReport {
        if let Err(message) = self.require_robot_manual_authority(true) {
            return self.robot_result(HmiActionStatus::Denied, &position_id, &message, "denied");
        }
        let result = self
            .robot
            .as_mut()
            .expect("robot authority requires runtime")
            .command_taught_position(&position_id, speed_percent);
        match result {
            Ok(true) => self.robot_result(
                HmiActionStatus::Applied,
                &position_id,
                "Robot is moving to the selected taught position.",
                "applied",
            ),
            Ok(false) => self.robot_result(
                HmiActionStatus::Denied,
                &position_id,
                "Unknown taught position.",
                "denied",
            ),
            Err(error) => self.motion_result(Err(error), &position_id),
        }
    }

    pub(super) fn jog_robot(
        &mut self,
        coordinate_system: HmiRobotCoordinateSystem,
        axis: HmiRobotAxis,
        increment: f64,
        speed_percent: f64,
    ) -> HmiActionReport {
        if !increment.is_finite() || increment == 0.0 {
            return self.robot_result(
                HmiActionStatus::Denied,
                "jog",
                "Jog increment must be a finite non-zero value.",
                "denied",
            );
        }
        if let Err(message) = self.require_robot_manual_authority(true) {
            return self.robot_result(HmiActionStatus::Denied, "jog", &message, "denied");
        }
        let robot = self
            .robot
            .as_mut()
            .expect("robot authority requires runtime");
        let result = match coordinate_system {
            HmiRobotCoordinateSystem::World => cartesian_axis(axis)
                .ok_or(RobotMotionError::OutsideWorkspace)
                .and_then(|axis| robot.jog_cartesian(axis, increment, speed_percent)),
            HmiRobotCoordinateSystem::Joint => joint_axis(axis)
                .ok_or(RobotMotionError::OutsideWorkspace)
                .and_then(|axis| robot.jog_joint(axis, increment, speed_percent)),
        };
        self.motion_result(result, "jog")
    }

    pub(super) fn teach_robot_position(
        &mut self,
        position_id: String,
        label: String,
    ) -> HmiActionReport {
        if let Err(message) = self.require_robot_setup_authority("robot-teach") {
            return self.robot_result(HmiActionStatus::Denied, &position_id, &message, "denied");
        }
        if ComponentId::new(&position_id).is_err() || label.trim().is_empty() || label.len() > 64 {
            return self.robot_result(
                HmiActionStatus::Denied,
                &position_id,
                "Taught position requires a valid ID and a label of 1 to 64 characters.",
                "denied",
            );
        }
        self.robot
            .as_mut()
            .expect("robot setup authority requires runtime")
            .teach(&position_id, &label);
        self.robot_result(
            HmiActionStatus::Applied,
            &position_id,
            "Current Cartesian pose stored as a taught position for this session.",
            "applied",
        )
    }

    pub(super) fn run_robot_program(&mut self, single_step: bool) -> HmiActionReport {
        if let Err(message) = self.require_robot_setup_authority("robot-program") {
            return self.robot_result(HmiActionStatus::Denied, "robot-program", &message, "denied");
        }
        if !self
            .robot
            .as_ref()
            .is_some_and(|robot| robot.motion_enabled())
        {
            return self.robot_result(
                HmiActionStatus::Denied,
                "robot-program",
                "Program motion requires the pendant motion-enable control.",
                "denied",
            );
        }
        let robot = self
            .robot
            .as_mut()
            .expect("robot setup authority requires runtime");
        let started = if single_step {
            robot.step_program()
        } else {
            robot.start_program()
        };
        self.robot_result(
            if started {
                HmiActionStatus::Applied
            } else {
                HmiActionStatus::Denied
            },
            "robot-program",
            if started {
                if single_step {
                    "Single-line execution started."
                } else {
                    "Robot program execution started."
                }
            } else {
                "Robot program contains no executable instructions."
            },
            if started { "applied" } else { "denied" },
        )
    }

    pub(super) fn pause_robot_program(&mut self) -> HmiActionReport {
        if let Err(message) = self.require_robot_setup_authority("robot-program") {
            return self.robot_result(HmiActionStatus::Denied, "robot-program", &message, "denied");
        }
        let Some(robot) = &mut self.robot else {
            return self.robot_result(
                HmiActionStatus::Denied,
                "robot-program",
                "No robot motion profile is configured.",
                "denied",
            );
        };
        robot.pause_program();
        self.robot_result(
            HmiActionStatus::Applied,
            "robot-program",
            "Program paused after the current motion update.",
            "applied",
        )
    }

    pub(super) fn reset_robot_program(&mut self) -> HmiActionReport {
        if let Err(message) = self.require_robot_setup_authority("robot-program") {
            return self.robot_result(HmiActionStatus::Denied, "robot-program", &message, "denied");
        }
        let Some(robot) = &mut self.robot else {
            return self.robot_result(
                HmiActionStatus::Denied,
                "robot-program",
                "No robot motion profile is configured.",
                "denied",
            );
        };
        robot.reset_program();
        self.robot_result(
            HmiActionStatus::Applied,
            "robot-program",
            "Program pointer and robot motion were reset.",
            "applied",
        )
    }

    pub(super) fn load_robot_program(&mut self, name: String, source: String) -> HmiActionReport {
        if let Err(message) = self.require_robot_setup_authority("robot-program") {
            return self.robot_result(HmiActionStatus::Denied, "program-load", &message, "denied");
        }
        if name.len() > 64 {
            return self.robot_result(
                HmiActionStatus::Denied,
                "program-load",
                "Robot program name must not exceed 64 characters.",
                "denied",
            );
        }
        let revision = source_revision(&source);
        let result = self
            .robot
            .as_mut()
            .expect("robot setup authority requires runtime")
            .load_program(name, source, revision);
        match result {
            Ok(()) => self.robot_result(
                HmiActionStatus::Applied,
                "program-load",
                "Robot program parsed and loaded for this session.",
                "applied",
            ),
            Err(error) => self.robot_result(
                HmiActionStatus::Denied,
                "program-load",
                &error.to_string(),
                "denied",
            ),
        }
    }

    fn require_robot_manual_authority(&self, require_enable: bool) -> Result<(), String> {
        if !self.has_permission("robot-jog") {
            return Err("This interface is not permitted to jog the robot.".into());
        }
        let station = self.robot_station()?;
        if !matches!(
            station.selected_mode,
            HmiControlMode::Manual | HmiControlMode::Setup
        ) {
            return Err("Manual robot motion requires manual or authenticated setup mode.".into());
        }
        if station.selected_mode == HmiControlMode::Setup && !station.setup_authenticated {
            return Err("Setup mode is not authenticated.".into());
        }
        if self.any_mould_running() {
            return Err(
                "Manual robot motion is inhibited while a mould automatic cycle is running.".into(),
            );
        }
        for safety in self
            .safety
            .iter()
            .filter(|safety| self.safety_in_scope(&safety.component_id))
        {
            if safety.trip_latched {
                return Err("Robot safety trip is latched.".into());
            }
            if safety.permissives.iter().any(|permissive| {
                !(permissive.satisfied
                    || (station.selected_mode == HmiControlMode::Setup
                        && station.setup_authenticated
                        && station.bypassed_permissives.contains(&permissive.tag)))
            }) {
                return Err("A required robot safety permissive is not satisfied.".into());
            }
        }
        if require_enable
            && !self
                .robot
                .as_ref()
                .is_some_and(|robot| robot.motion_enabled())
        {
            return Err("Pendant motion enable is not active.".into());
        }
        Ok(())
    }

    fn require_robot_setup_authority(&self, permission: &str) -> Result<(), String> {
        if !self.has_permission(permission) {
            return Err(
                "This interface is not permitted to change robot programs or taught positions."
                    .into(),
            );
        }
        let station = self.robot_station()?;
        if station.selected_mode != HmiControlMode::Setup || !station.setup_authenticated {
            return Err("Robot programming requires authenticated setup mode.".into());
        }
        if self.any_mould_running() {
            return Err(
                "Robot programming is inhibited while a mould automatic cycle is running.".into(),
            );
        }
        Ok(())
    }

    fn robot_station(&self) -> Result<&super::super::state::ControlStationRuntime, String> {
        if self.robot.is_none() {
            return Err("No robot motion profile is configured.".into());
        }
        self.controller
            .stations
            .get(&self.id)
            .filter(|station| station.station_type == "robot-joystick")
            .ok_or_else(|| "This interface is not the robot pendant.".into())
    }

    fn motion_result(
        &mut self,
        result: Result<(), RobotMotionError>,
        target: &str,
    ) -> HmiActionReport {
        let (status, message, outcome) = match result {
            Ok(()) => (
                HmiActionStatus::Applied,
                "Robot motion accepted; live progress is available on the pendant.",
                "applied",
            ),
            Err(RobotMotionError::OutsideWorkspace) => (
                HmiActionStatus::Denied,
                "Requested motion exceeds the configured Cartesian or joint workspace.",
                "denied",
            ),
            Err(RobotMotionError::InvalidSpeed) => (
                HmiActionStatus::Denied,
                "Speed override must be greater than zero and no more than 100 percent.",
                "denied",
            ),
            Err(RobotMotionError::MotionActive) => (
                HmiActionStatus::Denied,
                "A robot motion is already active.",
                "denied",
            ),
        };
        self.robot_result(status, target, message, outcome)
    }

    fn robot_result(
        &mut self,
        status: HmiActionStatus,
        target: &str,
        message: &str,
        result: &str,
    ) -> HmiActionReport {
        self.finish(
            status,
            message.into(),
            "robot-motion",
            target,
            result,
            vec![trace_entry(&self.id, "robot controller", message.into())],
        )
    }
}

fn cartesian_axis(axis: HmiRobotAxis) -> Option<RobotCartesianAxis> {
    match axis {
        HmiRobotAxis::X => Some(RobotCartesianAxis::X),
        HmiRobotAxis::Y => Some(RobotCartesianAxis::Y),
        HmiRobotAxis::Z => Some(RobotCartesianAxis::Z),
        HmiRobotAxis::W => Some(RobotCartesianAxis::W),
        HmiRobotAxis::P => Some(RobotCartesianAxis::P),
        HmiRobotAxis::R => Some(RobotCartesianAxis::R),
        _ => None,
    }
}

fn joint_axis(axis: HmiRobotAxis) -> Option<usize> {
    match axis {
        HmiRobotAxis::J1 => Some(0),
        HmiRobotAxis::J2 => Some(1),
        HmiRobotAxis::J3 => Some(2),
        HmiRobotAxis::J4 => Some(3),
        HmiRobotAxis::J5 => Some(4),
        HmiRobotAxis::J6 => Some(5),
        _ => None,
    }
}
