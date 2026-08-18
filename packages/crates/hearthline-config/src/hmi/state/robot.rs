use std::collections::BTreeMap;

use hearthline_engine::{
    RobotCartesianAxis, RobotCellArbiter, RobotCellStage, RobotInstruction, RobotJoints,
    RobotMotionError, RobotMotionKind, RobotMotionRuntime, RobotPose, RobotProgramRuntime,
    RobotWorkspace,
};

use super::super::robot::{
    ParsedRobotProgram, parse, routine_map, validate_automatic_routines,
    validate_routines_for_handoffs,
};
use super::super::{
    HmiRobotArchitecture, HmiRobotCellState, HmiRobotFrame, HmiRobotHandoff, HmiRobotMotionState,
    HmiRobotPayload, HmiRobotPose, HmiRobotProgramLine, HmiRobotProgramState, HmiRobotState,
    HmiRobotTaughtPosition, HmiRobotTool, HmiRobotWorkspace,
};
use crate::{ConfigError, RobotHandoffConfig, RobotMotionProfileConfig, RobotPoseConfig};

mod automatic;
mod cell_equipment;

pub(in crate::hmi) use cell_equipment::{GuardedCellRuntime, HandoffStationRuntime};

#[derive(Clone, Debug)]
pub(in crate::hmi) struct RobotRuntime {
    motion: RobotMotionRuntime,
    program: RobotProgramRuntime,
    routines: BTreeMap<String, hearthline_engine::RobotProgram>,
    program_name: String,
    program_path: String,
    program_revision: String,
    source: String,
    source_lines: Vec<String>,
    taught_positions: Vec<HmiRobotTaughtPosition>,
    default_speed_percent: f64,
    motion_enabled: bool,
    automatic_command: String,
    architecture: HmiRobotArchitecture,
    frames: Vec<HmiRobotFrame>,
    payloads: Vec<HmiRobotPayload>,
    tools: Vec<HmiRobotTool>,
    handoffs: Vec<HmiRobotHandoff>,
    handoff_config: Vec<RobotHandoffConfig>,
    active_user_frame: String,
    active_tool: String,
    active_payload: String,
    cell: RobotCellArbiter,
    completed_moulds: Vec<String>,
    automatic_gripper_closed: bool,
    automatic_fault: Option<RobotAutomaticFault>,
}

#[derive(Clone, Debug)]
pub(in crate::hmi) struct RobotAutomaticFault {
    pub(in crate::hmi) code: &'static str,
    pub(in crate::hmi) mould: String,
    pub(in crate::hmi) message: String,
}

impl RobotRuntime {
    pub(in crate::hmi) fn from_profile(
        profile: &RobotMotionProfileConfig,
        source: String,
        revision: String,
    ) -> Result<Self, ConfigError> {
        let workspace = workspace(&profile.workspace);
        let home = pose(profile.home);
        let motion = RobotMotionRuntime::new(
            workspace,
            home,
            profile.max_linear_speed_mm_s,
            profile.max_joint_speed_deg_s,
        )
        .map_err(|error| ConfigError::new(format!("invalid robot motion profile: {error:?}")))?;
        let parsed = parse(&source, home)?;
        let routines = routine_map(&parsed.routines)?;
        validate_automatic_routines(profile, &routines)?;
        let taught_positions = profile
            .taught_positions
            .iter()
            .map(|position| HmiRobotTaughtPosition {
                id: position.id.clone(),
                label: position.label.clone(),
                pose: hmi_pose(pose(position.pose)),
            })
            .collect();
        Ok(Self {
            motion,
            program: RobotProgramRuntime::new(parsed.program.clone()),
            routines,
            program_name: parsed.name,
            program_path: profile.program_ref.clone(),
            program_revision: revision,
            source,
            source_lines: parsed.source_lines,
            taught_positions,
            default_speed_percent: profile.default_speed_percent,
            motion_enabled: false,
            automatic_command: "home".into(),
            architecture: HmiRobotArchitecture {
                controller: profile.architecture.cell_controller.clone(),
                manipulator: profile.architecture.manipulator.clone(),
                pendant: profile.architecture.pendant.clone(),
                safety_interface: profile.architecture.safety_interface.clone(),
                cell_controller: profile.architecture.cell_controller.clone(),
                servo_axes: profile.architecture.servo_axes,
                motion_group: profile.architecture.motion_group.clone(),
                interpolation_cycle_ms: profile.architecture.interpolation_cycle_ms,
            },
            frames: profile
                .frames
                .iter()
                .map(|frame| HmiRobotFrame {
                    id: frame.id.clone(),
                    label: frame.label.clone(),
                    parent: frame.parent.clone(),
                    pose: hmi_pose(pose(frame.pose)),
                })
                .collect(),
            payloads: profile
                .payloads
                .iter()
                .map(|payload| HmiRobotPayload {
                    id: payload.id.clone(),
                    label: payload.label.clone(),
                    mass_kg: payload.mass_kg,
                    center_of_mass_mm: payload.center_of_mass_mm,
                })
                .collect(),
            tools: profile
                .tools
                .iter()
                .map(|tool| HmiRobotTool {
                    id: tool.id.clone(),
                    label: tool.label.clone(),
                    tcp: hmi_pose(pose(tool.tcp)),
                    payload: tool.payload.clone(),
                })
                .collect(),
            handoffs: profile
                .handoffs
                .iter()
                .map(|handoff| HmiRobotHandoff {
                    mould: handoff.mould.clone(),
                    program: handoff.program.clone(),
                    user_frame: handoff.user_frame.clone(),
                    approach_position: handoff.approach_position.clone(),
                    pickup_position: handoff.pickup_position.clone(),
                    handoff_position: handoff.handoff_position.clone(),
                    retreat_position: handoff.retreat_position.clone(),
                    pickup_tolerance_mm: handoff.pickup_tolerance_mm,
                    handoff_tolerance_mm: handoff.handoff_tolerance_mm,
                    orientation_tolerance_deg: handoff.orientation_tolerance_deg,
                })
                .collect(),
            handoff_config: profile.handoffs.clone(),
            active_user_frame: profile.active_user_frame.clone(),
            active_tool: profile.active_tool.clone(),
            active_payload: profile.active_payload.clone(),
            cell: RobotCellArbiter::default(),
            completed_moulds: Vec::new(),
            automatic_gripper_closed: false,
            automatic_fault: None,
        })
    }

    pub(in crate::hmi) fn tick(&mut self, elapsed_ms: u64) -> Result<(), RobotMotionError> {
        if self.program.running() || self.program.paused() {
            self.program.tick(&mut self.motion, elapsed_ms)
        } else {
            self.motion.tick(elapsed_ms);
            Ok(())
        }
    }

    pub(in crate::hmi) fn set_motion_enabled(&mut self, enabled: bool) {
        self.motion_enabled = enabled;
        if !enabled {
            self.motion.stop();
            self.program.pause();
        }
    }

    pub(in crate::hmi) fn apply_manual_state(&mut self, state: &str) {
        self.automatic_command = state.into();
        match state {
            "stopped" => self.motion.stop(),
            "home" => {
                let _ = self.motion.command_pose(
                    self.motion.home(),
                    RobotMotionKind::Joint,
                    self.default_speed_percent,
                );
            }
            _ => {}
        }
    }

    pub(in crate::hmi) fn command_pose(
        &mut self,
        target: RobotPose,
        speed_percent: f64,
    ) -> Result<(), RobotMotionError> {
        self.motion
            .command_pose(target, RobotMotionKind::Linear, speed_percent)?;
        self.automatic_command = "manual-positioning".into();
        Ok(())
    }

    pub(in crate::hmi) fn command_taught_position(
        &mut self,
        id: &str,
        speed_percent: f64,
    ) -> Result<bool, RobotMotionError> {
        let Some(target) = self.taught_pose(id) else {
            return Ok(false);
        };
        self.command_pose(target, speed_percent)?;
        Ok(true)
    }

    pub(in crate::hmi) fn jog_cartesian(
        &mut self,
        axis: RobotCartesianAxis,
        increment: f64,
        speed_percent: f64,
    ) -> Result<(), RobotMotionError> {
        self.motion.jog_cartesian(axis, increment, speed_percent)?;
        self.automatic_command = "jogging".into();
        Ok(())
    }

    pub(in crate::hmi) fn jog_joint(
        &mut self,
        axis: usize,
        increment: f64,
        speed_percent: f64,
    ) -> Result<(), RobotMotionError> {
        self.motion.jog_joint(axis, increment, speed_percent)?;
        self.automatic_command = "joint-jogging".into();
        Ok(())
    }

    pub(in crate::hmi) fn teach(&mut self, id: &str, label: &str) {
        let pose = hmi_pose(self.motion.pose());
        if let Some(existing) = self
            .taught_positions
            .iter_mut()
            .find(|position| position.id == id)
        {
            existing.label = label.into();
            existing.pose = pose;
        } else {
            self.taught_positions.push(HmiRobotTaughtPosition {
                id: id.into(),
                label: label.into(),
                pose,
            });
        }
    }

    pub(in crate::hmi) fn load_program(
        &mut self,
        name: String,
        source: String,
        revision: String,
    ) -> Result<(), ConfigError> {
        let ParsedRobotProgram {
            name: parsed_name,
            program,
            routines,
            source_lines,
        } = parse(&source, self.motion.home())?;
        let routines = routine_map(&routines)?;
        validate_routines_for_handoffs(&self.handoff_config, &routines)?;
        self.program.replace(program);
        self.routines = routines;
        self.program_name = if name.trim().is_empty() {
            parsed_name
        } else {
            name
        };
        self.program_path = "session-upload.g".into();
        self.program_revision = revision;
        self.source = source;
        self.source_lines = source_lines;
        Ok(())
    }

    pub(in crate::hmi) fn start_program(&mut self) -> bool {
        let started = self.program.start();
        if started {
            self.automatic_command = "program-execution".into();
        }
        started
    }

    pub(in crate::hmi) fn step_program(&mut self) -> bool {
        let started = self.program.step();
        if started {
            self.automatic_command = "program-step".into();
        }
        started
    }

    pub(in crate::hmi) fn pause_program(&mut self) {
        self.program.pause();
    }

    pub(in crate::hmi) fn reset_program(&mut self) {
        self.program.reset(&mut self.motion);
        self.cell.clear_fault();
        self.automatic_fault = None;
        self.automatic_command = "program-reset".into();
    }

    pub(in crate::hmi) const fn motion_enabled(&self) -> bool {
        self.motion_enabled
    }

    pub(in crate::hmi) fn normalized_position_mm(&self) -> f64 {
        let progress = self.motion.progress();
        match self.automatic_command.as_str() {
            "home" | "stopped" | "program-reset" => 0.0,
            "approaching" => 900.0 * progress,
            "gripping" => 900.0 + 300.0 * progress,
            "delivering" => 1_200.0 + 1_000.0 * progress,
            "releasing" => 2_200.0,
            "returning" => 2_200.0 + 800.0 * progress,
            _ => {
                let workspace = self.motion.workspace();
                let span = workspace.maximum.x - workspace.minimum.x;
                (self.motion.pose().x - workspace.minimum.x) / span * 3_000.0
            }
        }
        .clamp(0.0, 3_000.0)
    }

    pub(in crate::hmi) fn snapshot(&self) -> HmiRobotState {
        let active_line = self.program.active_source_line();
        let executable = self.program.lines();
        HmiRobotState {
            coordinate_system: "world",
            motion_enabled: self.motion_enabled,
            pose: hmi_pose(self.motion.pose()),
            joints: self.motion.joints().axes,
            gripper_closed: self.program.gripper_closed()
                || self.automatic_gripper_closed
                || self.automatic_command == "gripping"
                || self.automatic_command == "delivering",
            automatic_command: self.automatic_command.clone(),
            controller_state: if self.cell.stage() == RobotCellStage::Faulted
                || self.automatic_fault.is_some()
            {
                "faulted"
            } else if self.cell.active().is_some() || self.motion.active() {
                "executing"
            } else {
                "ready"
            },
            active_user_frame: self.active_user_frame.clone(),
            active_tool: self.active_tool.clone(),
            active_payload: self.active_payload.clone(),
            architecture: self.architecture.clone(),
            frames: self.frames.clone(),
            payloads: self.payloads.clone(),
            tools: self.tools.clone(),
            handoffs: self.handoffs.clone(),
            cell: HmiRobotCellState {
                active_mould: self.cell.active().map(str::to_string),
                queued_moulds: self.cell.queue().map(str::to_string).collect(),
                stage: self.cell.stage().as_str(),
                completed_handoffs: self.cell.completed(),
                active_program: self
                    .cell
                    .active()
                    .and_then(|mould| self.handoff(mould))
                    .map(|handoff| handoff.program.clone()),
                fault_code: self
                    .automatic_fault
                    .as_ref()
                    .map(|fault| fault.code.to_string()),
                fault_message: self
                    .automatic_fault
                    .as_ref()
                    .map(|fault| fault.message.clone()),
            },
            motion: HmiRobotMotionState {
                active: self.motion.active(),
                kind: self.motion.motion_kind().as_str(),
                progress_percent: self.motion.progress() * 100.0,
                elapsed_ms: self.motion.elapsed_ms(),
                duration_ms: self.motion.duration_ms(),
                speed_percent: self.motion.speed_percent(),
                target_pose: hmi_pose(self.motion.target_pose()),
                target_joints: self.motion.target_joints().axes,
            },
            program: HmiRobotProgramState {
                name: self.program_name.clone(),
                source_path: self.program_path.clone(),
                revision: self.program_revision.clone(),
                running: self.program.running(),
                paused: self.program.paused(),
                active_line,
                cycle_count: self.program.cycle_count(),
                source: self.source.clone(),
                lines: self
                    .source_lines
                    .iter()
                    .enumerate()
                    .map(|(index, source)| {
                        let number = u16::try_from(index + 1).unwrap_or(u16::MAX);
                        HmiRobotProgramLine {
                            number,
                            source: source.clone(),
                            operation: executable
                                .iter()
                                .find(|line| line.source_line == number)
                                .map(|line| operation(line.instruction)),
                            active: active_line == Some(number),
                        }
                    })
                    .collect(),
            },
            taught_positions: self.taught_positions.clone(),
            workspace: hmi_workspace(self.motion.workspace()),
        }
    }

    fn taught_pose(&self, id: &str) -> Option<RobotPose> {
        self.taught_positions
            .iter()
            .find(|position| position.id == id)
            .map(|position| robot_pose(position.pose))
    }

    fn handoff(&self, mould: &str) -> Option<&RobotHandoffConfig> {
        self.handoff_config
            .iter()
            .find(|handoff| handoff.mould == mould)
    }
}

fn operation(instruction: RobotInstruction) -> String {
    match instruction {
        RobotInstruction::Move { kind, .. } => format!("{} motion", kind.as_str()),
        RobotInstruction::Dwell { .. } => "dwell".into(),
        RobotInstruction::Gripper { closed: true } => "close gripper".into(),
        RobotInstruction::Gripper { closed: false } => "open gripper".into(),
        RobotInstruction::End => "end program".into(),
    }
}

pub(in crate::hmi) fn pose(config: RobotPoseConfig) -> RobotPose {
    RobotPose::new(config.x, config.y, config.z, config.w, config.p, config.r)
}

pub(in crate::hmi) fn robot_pose(value: HmiRobotPose) -> RobotPose {
    RobotPose::new(value.x, value.y, value.z, value.w, value.p, value.r)
}

fn hmi_pose(value: RobotPose) -> HmiRobotPose {
    HmiRobotPose {
        x: value.x,
        y: value.y,
        z: value.z,
        w: value.w,
        p: value.p,
        r: value.r,
    }
}

fn workspace(config: &crate::RobotWorkspaceConfig) -> RobotWorkspace {
    RobotWorkspace {
        minimum: pose(config.minimum),
        maximum: pose(config.maximum),
        joint_minimum: RobotJoints::new(config.joint_minimum),
        joint_maximum: RobotJoints::new(config.joint_maximum),
    }
}

fn hmi_workspace(value: RobotWorkspace) -> HmiRobotWorkspace {
    HmiRobotWorkspace {
        minimum: hmi_pose(value.minimum),
        maximum: hmi_pose(value.maximum),
        joint_minimum: value.joint_minimum.axes,
        joint_maximum: value.joint_maximum.axes,
    }
}
