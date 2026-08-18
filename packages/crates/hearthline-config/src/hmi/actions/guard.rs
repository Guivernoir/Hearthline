use super::super::builder::support::trace_entry;
use super::super::state::HmiSession;
use super::super::{
    HmiAction, HmiActionReport, HmiActionStatus, HmiAlarmSeverity, HmiRobotAxis,
    HmiRobotCoordinateSystem, HmiRobotPose,
};

const GUARD_SAFETY: &str = "area-02-cell-guard-safe-01";
const GATE_POSITION: &str = "area-02-cell-gate-pos-01";

impl HmiSession {
    pub(super) fn inhibit_open_guard_motion(
        &mut self,
        action: &HmiAction,
    ) -> Option<HmiActionReport> {
        if !self
            .guarded_cell
            .as_ref()
            .is_some_and(|cell| cell.gate_open())
        {
            return None;
        }
        let target = self.authorized_guarded_motion_target(action)?;
        self.latch_guard_motion_trip(&target);
        Some(self.finish(
            HmiActionStatus::Denied,
            "Motion denied: close the guarded-cell gate, then reset the safety circuit.".into(),
            "guarded-motion",
            &target,
            "denied",
            vec![trace_entry(
                GUARD_SAFETY,
                "guard interlock",
                format!("motion demand for {target} occurred with the access gate open"),
            )],
        ))
    }

    fn authorized_guarded_motion_target(&self, action: &HmiAction) -> Option<String> {
        match action {
            HmiAction::StartMould => self.authorize_local_mould_production("start-mould"),
            HmiAction::EndMouldAfterCycle => {
                self.authorize_local_mould_production("end-mould-after-cycle")
            }
            HmiAction::Command { tag, value }
                if guarded_command(tag)
                    && value != "stopped"
                    && value != "isolated"
                    && self.command_tags.iter().any(|candidate| candidate == tag)
                    && self.authorize_manual_command(tag).is_ok()
                    && self.actuators.iter().any(|actuator| {
                        actuator.command_tag == *tag && actuator.states.contains(value)
                    }) =>
            {
                Some(tag.clone())
            }
            HmiAction::MoveRobot {
                target,
                speed_percent,
            } if valid_pose(*target)
                && valid_speed(*speed_percent)
                && self.require_robot_manual_station_authority(true).is_ok() =>
            {
                Some("robot-cell".into())
            }
            HmiAction::MoveRobotToPosition {
                position_id,
                speed_percent,
            } if valid_speed(*speed_percent)
                && self.require_robot_manual_station_authority(true).is_ok()
                && self
                    .robot
                    .as_ref()
                    .is_some_and(|robot| robot.has_taught_position(position_id)) =>
            {
                Some("robot-cell".into())
            }
            HmiAction::JogRobot {
                coordinate_system,
                axis,
                increment,
                speed_percent,
            } if increment.is_finite()
                && *increment != 0.0
                && valid_speed(*speed_percent)
                && valid_jog_axis(*coordinate_system, *axis)
                && self.require_robot_manual_station_authority(true).is_ok() =>
            {
                Some("robot-cell".into())
            }
            HmiAction::RunRobotProgram | HmiAction::StepRobotProgram
                if self.require_robot_setup_authority("robot-program").is_ok()
                    && self
                        .robot
                        .as_ref()
                        .is_some_and(|robot| robot.motion_enabled() && robot.has_program()) =>
            {
                Some("robot-cell".into())
            }
            _ => None,
        }
    }

    pub(super) fn set_guard_door(&mut self, open: bool) -> HmiActionReport {
        if !self.has_permission("operate-guard-door") {
            return self.finish(
                HmiActionStatus::Denied,
                "This interface cannot change the simulated access-gate position.".into(),
                "set-guard-door",
                GUARD_SAFETY,
                "denied",
                Vec::new(),
            );
        }
        let active_motion = self.guarded_motion_active();
        let Some(cell) = &mut self.guarded_cell else {
            return self.finish(
                HmiActionStatus::Denied,
                "No guarded-cell equipment is configured.".into(),
                "set-guard-door",
                GUARD_SAFETY,
                "denied",
                Vec::new(),
            );
        };
        cell.set_gate_open(open);
        self.set_guard_closed_permissive(!open);
        if open && active_motion {
            self.latch_guard_motion_trip("forming-cell");
        }
        self.sync_guarded_cell_io();
        let reset_required = self
            .safety
            .iter()
            .find(|safety| safety.component_id == GUARD_SAFETY)
            .is_some_and(|safety| safety.trip_latched);
        let position = if open { "open" } else { "closed" };
        self.finish(
            HmiActionStatus::Applied,
            if open && active_motion {
                "Gate opened during active motion. Hazardous motion stopped and the guard trip latched."
                    .into()
            } else if open {
                "Gate opened. Guarded-cell motion is inhibited.".into()
            } else if reset_required {
                "Gate closed. A latched guard trip still requires an authorized safety reset."
                    .into()
            } else {
                "Gate closed. The guarded-cell motion permissive is healthy.".into()
            },
            "set-guard-door",
            GUARD_SAFETY,
            "applied",
            vec![trace_entry(
                GATE_POSITION,
                "gate position",
                format!("access gate reported {position}"),
            )],
        )
    }

    pub(super) fn latch_guard_motion_trip(&mut self, target: &str) {
        if let Some(safety) = self
            .safety
            .iter_mut()
            .find(|safety| safety.component_id == GUARD_SAFETY)
        {
            safety.trip_latched = true;
        }
        for mould in self.moulds.values_mut().filter(|mould| mould.running()) {
            mould.trip_safety();
        }
        let robot_active = self
            .robot
            .as_ref()
            .is_some_and(|robot| robot.guarded_motion_active());
        if (robot_active || target.contains("robot"))
            && let Some(robot) = &mut self.robot
        {
            robot.trip_guard();
        }
        if let Some(cell) = &mut self.guarded_cell {
            cell.trip_motion();
        }
        self.raise_alarm(
            "CELL-GUARD-MOTION-INHIBITED",
            GUARD_SAFETY,
            &format!(
                "Hazardous motion demand for {target} occurred while the guarded-cell gate was open."
            ),
            HmiAlarmSeverity::Trip,
        );
        self.sync_guarded_cell_io();
    }

    pub(super) fn clear_guard_motion_trip(&mut self) {
        if let Some(cell) = &mut self.guarded_cell {
            cell.clear_trip();
        }
        if let Some(robot) = &mut self.robot {
            robot.clear_guard_trip();
        }
        self.sync_guarded_cell_io();
    }

    pub(super) fn guarded_motion_active(&self) -> bool {
        self.any_mould_running()
            || self
                .robot
                .as_ref()
                .is_some_and(|robot| robot.guarded_motion_active())
            || self
                .guarded_cell
                .as_ref()
                .is_some_and(|cell| cell.motion_active())
    }

    fn set_guard_closed_permissive(&mut self, closed: bool) {
        if let Some(safety) = self
            .safety
            .iter_mut()
            .find(|safety| safety.component_id == GUARD_SAFETY)
            && let Some(permissive) = safety
                .permissives
                .iter_mut()
                .find(|permissive| permissive.tag == GATE_POSITION)
        {
            permissive.satisfied = closed;
        }
    }
}

fn guarded_command(tag: &str) -> bool {
    tag.contains("mould") || tag.contains("robot") || tag.contains("handoff")
}

fn valid_speed(speed_percent: f64) -> bool {
    speed_percent.is_finite() && speed_percent > 0.0 && speed_percent <= 100.0
}

fn valid_pose(pose: HmiRobotPose) -> bool {
    [pose.x, pose.y, pose.z, pose.w, pose.p, pose.r]
        .into_iter()
        .all(f64::is_finite)
}

fn valid_jog_axis(coordinate_system: HmiRobotCoordinateSystem, axis: HmiRobotAxis) -> bool {
    matches!(
        (coordinate_system, axis),
        (
            HmiRobotCoordinateSystem::World,
            HmiRobotAxis::X
                | HmiRobotAxis::Y
                | HmiRobotAxis::Z
                | HmiRobotAxis::W
                | HmiRobotAxis::P
                | HmiRobotAxis::R
        ) | (
            HmiRobotCoordinateSystem::Joint,
            HmiRobotAxis::J1
                | HmiRobotAxis::J2
                | HmiRobotAxis::J3
                | HmiRobotAxis::J4
                | HmiRobotAxis::J5
                | HmiRobotAxis::J6
        )
    )
}
