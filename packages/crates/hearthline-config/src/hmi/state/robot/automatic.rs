use hearthline_engine::{RobotCellRequestStatus, RobotCellStage, RobotPose, RobotProgramRuntime};

use super::{RobotAutomaticFault, RobotRuntime};

impl RobotRuntime {
    pub(in crate::hmi) fn guarded_motion_active(&self) -> bool {
        self.motion.active() || self.program.running() || self.cell.active().is_some()
    }

    pub(in crate::hmi) fn trip_guard(&mut self) {
        self.motion.stop();
        self.program.pause();
        self.motion_enabled = false;
        self.cell.set_stage(RobotCellStage::Faulted);
        self.automatic_command = "safety-hold".into();
        self.automatic_fault = Some(RobotAutomaticFault {
            code: "CELL-GUARD-MOTION-INHIBITED",
            mould: self.cell.active().unwrap_or("robot-cell").to_string(),
            message: "Robot motion inhibited by the open guarded-cell access gate.".into(),
        });
    }

    pub(in crate::hmi) fn clear_guard_trip(&mut self) {
        if !self
            .automatic_fault
            .as_ref()
            .is_some_and(|fault| fault.code == "CELL-GUARD-MOTION-INHIBITED")
        {
            return;
        }
        self.program.reset(&mut self.motion);
        self.cell.clear_fault();
        self.completed_moulds.clear();
        self.automatic_gripper_closed = false;
        self.automatic_fault = None;
        self.automatic_command = "safety-reset".into();
    }

    pub(in crate::hmi) fn request_automatic_handoff(&mut self, mould: &str) {
        if self.cell.request(mould) == RobotCellRequestStatus::Granted {
            let _ = self.start_active_handoff();
        }
    }

    pub(in crate::hmi) fn tick_automatic(
        &mut self,
        elapsed_ms: u64,
    ) -> Option<RobotAutomaticFault> {
        if self.cell.active().is_none() || self.cell.stage() == RobotCellStage::Faulted {
            return None;
        }
        if !self.program.running() {
            return self.start_active_handoff().err();
        }

        let was_running = self.program.running();
        let gripper_before = self.program.gripper_closed();
        if let Err(error) = self.program.tick(&mut self.motion, elapsed_ms) {
            return Some(self.fault(
                "ROBOT-MOTION-FAULT",
                format!("Automatic motion failed: {error:?}."),
            ));
        }
        let gripper_after = self.program.gripper_closed();

        if !gripper_before && gripper_after {
            if let Err(fault) = self.validate_active_station_pose(true) {
                return Some(fault);
            }
            self.automatic_gripper_closed = true;
            self.automatic_command = "operator-handoff".into();
            self.cell.set_stage(RobotCellStage::Handoff);
        } else if gripper_before && !gripper_after {
            if let Err(fault) = self.validate_active_station_pose(false) {
                return Some(fault);
            }
            self.automatic_gripper_closed = false;
            self.automatic_command = "handoff-retreat".into();
            self.cell.set_stage(RobotCellStage::Retreat);
        }

        if was_running && !self.program.running() {
            if self.program.gripper_closed() {
                return Some(self.fault(
                    "ROBOT-PROGRAM-SEQUENCE",
                    "Automatic routine ended while the gripper was closed.".into(),
                ));
            }
            self.cell.set_stage(RobotCellStage::Return);
            if let Some(completed) = self.cell.complete_active() {
                self.completed_moulds.push(completed.to_string());
            }
            if self.cell.active().is_some() {
                if let Err(fault) = self.start_active_handoff() {
                    return Some(fault);
                }
            } else {
                self.automatic_command = "home".into();
                self.active_user_frame = "world".into();
            }
        }
        None
    }

    pub(in crate::hmi) fn pickup_ready(&self, mould: &str) -> bool {
        self.cell.active() == Some(mould)
            && matches!(
                self.cell.stage(),
                RobotCellStage::Handoff | RobotCellStage::Retreat | RobotCellStage::Return
            )
    }

    pub(in crate::hmi) fn delivery_ready(&self, mould: &str) -> bool {
        self.completed_moulds.iter().any(|target| target == mould)
    }

    pub(in crate::hmi) fn clear_delivery(&mut self, mould: &str) {
        self.completed_moulds.retain(|target| target != mould);
    }

    pub(in crate::hmi) fn automatic_command(&self) -> &str {
        &self.automatic_command
    }

    fn start_active_handoff(&mut self) -> Result<(), RobotAutomaticFault> {
        let Some(active) = self.cell.active().map(str::to_string) else {
            return Ok(());
        };
        let Some(handoff) = self.handoff(&active).cloned() else {
            return Err(self.fault(
                "ROBOT-HANDOFF-UNRESOLVED",
                format!("No handoff definition exists for {active}."),
            ));
        };
        let Some(program) = self.routines.get(&handoff.program).cloned() else {
            return Err(self.fault(
                "ROBOT-PROGRAM-MISSING",
                format!(
                    "Routine {} assigned to {active} is not loaded.",
                    handoff.program
                ),
            ));
        };
        self.program = RobotProgramRuntime::new(program);
        self.program_name.clone_from(&handoff.program);
        self.active_user_frame.clone_from(&handoff.user_frame);
        self.automatic_fault = None;
        self.automatic_gripper_closed = false;
        self.automatic_command = "approaching".into();
        self.cell.set_stage(RobotCellStage::Approach);
        if !self.program.start() {
            return Err(self.fault(
                "ROBOT-PROGRAM-EMPTY",
                format!(
                    "Routine {} contains no executable instructions.",
                    handoff.program
                ),
            ));
        }
        Ok(())
    }

    fn validate_active_station_pose(&mut self, pickup: bool) -> Result<(), RobotAutomaticFault> {
        let active = self.cell.active().unwrap_or("unknown").to_string();
        let Some(handoff) = self.handoff(&active).cloned() else {
            return Err(self.fault(
                "ROBOT-HANDOFF-UNRESOLVED",
                format!("No handoff definition exists for {active}."),
            ));
        };
        let position_id = if pickup {
            &handoff.pickup_position
        } else {
            &handoff.handoff_position
        };
        let tolerance_mm = if pickup {
            handoff.pickup_tolerance_mm
        } else {
            handoff.handoff_tolerance_mm
        };
        let Some(expected) = self.taught_pose(position_id) else {
            return Err(self.fault(
                "ROBOT-STATION-GEOMETRY-MISSING",
                format!("Reference position {position_id} is not configured."),
            ));
        };
        let actual = self.motion.pose();
        let translation_error = translation_error_mm(actual, expected);
        let orientation_error = orientation_error_deg(actual, expected);
        if translation_error <= tolerance_mm
            && orientation_error <= handoff.orientation_tolerance_deg
        {
            return Ok(());
        }
        let operation = if pickup { "pickup" } else { "handoff" };
        Err(self.fault(
            if pickup {
                "ROBOT-PICKUP-POSITION-MISMATCH"
            } else {
                "ROBOT-HANDOFF-POSITION-MISMATCH"
            },
            format!(
                "Routine {} reached the wrong {operation} pose for {active}: {:.1} mm and {:.1} deg error exceed {:.1} mm and {:.1} deg.",
                handoff.program,
                translation_error,
                orientation_error,
                tolerance_mm,
                handoff.orientation_tolerance_deg
            ),
        ))
    }

    fn fault(&mut self, code: &'static str, message: String) -> RobotAutomaticFault {
        self.motion.stop();
        self.program.pause();
        self.cell.set_stage(RobotCellStage::Faulted);
        self.automatic_command = "program-fault".into();
        let fault = RobotAutomaticFault {
            code,
            mould: self.cell.active().unwrap_or("robot-cell").to_string(),
            message,
        };
        self.automatic_fault = Some(fault.clone());
        fault
    }
}

fn translation_error_mm(actual: RobotPose, expected: RobotPose) -> f64 {
    ((actual.x - expected.x).powi(2)
        + (actual.y - expected.y).powi(2)
        + (actual.z - expected.z).powi(2))
    .sqrt()
}

fn orientation_error_deg(actual: RobotPose, expected: RobotPose) -> f64 {
    (actual.w - expected.w)
        .abs()
        .max((actual.p - expected.p).abs())
        .max((actual.r - expected.r).abs())
}
