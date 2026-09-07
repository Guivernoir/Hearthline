use std::collections::BTreeMap;

use hearthline_engine::{
    RobotCartesianAxis, RobotCartesianIncrement, RobotCellArbiter, RobotCellStage,
    RobotInstruction, RobotMotionError, RobotMotionKind, RobotMotionRuntime, RobotPose,
    RobotProgramRuntime,
};
use hearthline_model::{Angle, FixedValue, Percentage, Position, fixed};

use super::super::robot::{
    ParsedRobotProgram, parse, routine_map, validate_automatic_routines,
    validate_routine_assignments,
};
use super::super::{
    HmiRobotArchitecture, HmiRobotCellState, HmiRobotFrame, HmiRobotHandoff, HmiRobotMotionState,
    HmiRobotPayload, HmiRobotProgramLine, HmiRobotProgramState, HmiRobotState,
    HmiRobotTaughtPosition, HmiRobotTool,
};
use crate::{ConfigError, RobotMotionProfileConfig};

mod automatic;
mod cell_equipment;
mod quantities;

pub(in crate::hmi) use cell_equipment::{GuardedCellRuntime, HandoffStationRuntime};
pub(in crate::hmi) use quantities::{angle, percentage, pose, position, robot_pose};
use quantities::{
    angle_to_degrees, angular_speed, hmi_pose, hmi_workspace, joint_degrees, linear_speed,
    percentage_to_f64, position_to_mm, workspace,
};

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
    taught_positions: Vec<RobotTaughtPositionRuntime>,
    default_speed_percent: Percentage,
    motion_enabled: bool,
    automatic_command: String,
    architecture: HmiRobotArchitecture,
    frames: Vec<RobotFrameRuntime>,
    payloads: Vec<RobotPayloadRuntime>,
    tools: Vec<RobotToolRuntime>,
    handoffs: Vec<RobotHandoffRuntime>,
    active_user_frame: String,
    active_tool: String,
    active_payload: String,
    cell: RobotCellArbiter,
    completed_moulds: Vec<String>,
    automatic_gripper_closed: bool,
    automatic_fault: Option<RobotAutomaticFault>,
}

#[derive(Clone, Debug)]
struct RobotTaughtPositionRuntime {
    id: String,
    label: String,
    pose: RobotPose,
}

#[derive(Clone, Debug)]
struct RobotFrameRuntime {
    id: String,
    label: String,
    parent: Option<String>,
    pose: RobotPose,
}

#[derive(Clone, Debug)]
struct RobotPayloadRuntime {
    id: String,
    label: String,
    mass_kg: FixedValue,
    center_of_mass: [Position; 3],
}

#[derive(Clone, Debug)]
struct RobotToolRuntime {
    id: String,
    label: String,
    tcp: RobotPose,
    payload: String,
}

#[derive(Clone, Debug)]
struct RobotHandoffRuntime {
    mould: String,
    program: String,
    user_frame: String,
    approach_position: String,
    pickup_position: String,
    handoff_position: String,
    retreat_position: String,
    pickup_tolerance: Position,
    handoff_tolerance: Position,
    orientation_tolerance: Angle,
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
            linear_speed(profile.max_linear_speed_mm_s)?,
            angular_speed(profile.max_joint_speed_deg_s)?,
        )
        .map_err(|error| ConfigError::new(format!("invalid robot motion profile: {error:?}")))?;
        let parsed = parse(&source, home)?;
        let routines = routine_map(&parsed.routines)?;
        validate_automatic_routines(profile, &routines)?;
        let taught_positions = profile
            .taught_positions
            .iter()
            .map(|position| RobotTaughtPositionRuntime {
                id: position.id.clone(),
                label: position.label.clone(),
                pose: pose(position.pose),
            })
            .collect();
        let payloads = profile
            .payloads
            .iter()
            .map(|payload| {
                Ok(RobotPayloadRuntime {
                    id: payload.id.clone(),
                    label: payload.label.clone(),
                    mass_kg: super::super::quantities::quantize(payload.mass_kg)?,
                    center_of_mass: payload
                        .center_of_mass_mm
                        .map(|value| position(value).map_err(robot_config_error))
                        .into_iter()
                        .collect::<Result<Vec<_>, _>>()?
                        .try_into()
                        .expect("three center-of-mass coordinates"),
                })
            })
            .collect::<Result<Vec<_>, ConfigError>>()?;
        let handoffs = profile
            .handoffs
            .iter()
            .map(|handoff| {
                Ok(RobotHandoffRuntime {
                    mould: handoff.mould.clone(),
                    program: handoff.program.clone(),
                    user_frame: handoff.user_frame.clone(),
                    approach_position: handoff.approach_position.clone(),
                    pickup_position: handoff.pickup_position.clone(),
                    handoff_position: handoff.handoff_position.clone(),
                    retreat_position: handoff.retreat_position.clone(),
                    pickup_tolerance: position(handoff.pickup_tolerance_mm)
                        .map_err(robot_config_error)?,
                    handoff_tolerance: position(handoff.handoff_tolerance_mm)
                        .map_err(robot_config_error)?,
                    orientation_tolerance: angle(handoff.orientation_tolerance_deg)
                        .map_err(robot_config_error)?,
                })
            })
            .collect::<Result<Vec<_>, ConfigError>>()?;
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
            default_speed_percent: percentage(profile.default_speed_percent)
                .map_err(robot_config_error)?,
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
                .map(|frame| RobotFrameRuntime {
                    id: frame.id.clone(),
                    label: frame.label.clone(),
                    parent: frame.parent.clone(),
                    pose: pose(frame.pose),
                })
                .collect(),
            payloads,
            tools: profile
                .tools
                .iter()
                .map(|tool| RobotToolRuntime {
                    id: tool.id.clone(),
                    label: tool.label.clone(),
                    tcp: pose(tool.tcp),
                    payload: tool.payload.clone(),
                })
                .collect(),
            handoffs,
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
        speed_percent: Percentage,
    ) -> Result<(), RobotMotionError> {
        self.motion
            .command_pose(target, RobotMotionKind::Linear, speed_percent)?;
        self.automatic_command = "manual-positioning".into();
        Ok(())
    }

    pub(in crate::hmi) fn command_taught_position(
        &mut self,
        id: &str,
        speed_percent: Percentage,
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
        increment: RobotCartesianIncrement,
        speed_percent: Percentage,
    ) -> Result<(), RobotMotionError> {
        self.motion.jog_cartesian(axis, increment, speed_percent)?;
        self.automatic_command = "jogging".into();
        Ok(())
    }

    pub(in crate::hmi) fn jog_joint(
        &mut self,
        axis: usize,
        increment: Angle,
        speed_percent: Percentage,
    ) -> Result<(), RobotMotionError> {
        self.motion.jog_joint(axis, increment, speed_percent)?;
        self.automatic_command = "joint-jogging".into();
        Ok(())
    }

    pub(in crate::hmi) fn teach(&mut self, id: &str, label: &str) {
        let pose = self.motion.pose();
        if let Some(existing) = self
            .taught_positions
            .iter_mut()
            .find(|position| position.id == id)
        {
            existing.label = label.into();
            existing.pose = pose;
        } else {
            self.taught_positions.push(RobotTaughtPositionRuntime {
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
        validate_routine_assignments(
            self.handoffs
                .iter()
                .map(|handoff| (handoff.mould.as_str(), handoff.program.as_str())),
            &routines,
        )?;
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

    pub(in crate::hmi) fn normalized_position_mm(&self) -> FixedValue {
        let progress = FixedValue::from_ratio(self.motion.progress().raw(), 10_000)
            .unwrap_or(FixedValue::ZERO);
        match self.automatic_command.as_str() {
            "home" | "stopped" | "program-reset" => FixedValue::ZERO,
            "approaching" => fixed!(900.0) * progress,
            "gripping" => fixed!(900.0) + fixed!(300.0) * progress,
            "delivering" => fixed!(1200.0) + fixed!(1000.0) * progress,
            "releasing" => fixed!(2200.0),
            "returning" => fixed!(2200.0) + fixed!(800.0) * progress,
            _ => {
                let workspace = self.motion.workspace();
                let span = workspace.maximum.x.raw() - workspace.minimum.x.raw();
                let offset = self.motion.pose().x.raw() - workspace.minimum.x.raw();
                if span == 0 {
                    FixedValue::ZERO
                } else {
                    FixedValue::from_ratio(offset, span).unwrap_or(FixedValue::ZERO)
                        * fixed!(3000.0)
                }
            }
        }
        .clamp(FixedValue::ZERO, fixed!(3000.0))
    }

    pub(in crate::hmi) fn snapshot(&self) -> HmiRobotState {
        let active_line = self.program.active_source_line();
        let executable = self.program.lines();
        HmiRobotState {
            coordinate_system: "world",
            motion_enabled: self.motion_enabled,
            pose: hmi_pose(self.motion.pose()),
            joints: joint_degrees(self.motion.joints()),
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
            frames: self
                .frames
                .iter()
                .map(RobotFrameRuntime::snapshot)
                .collect(),
            payloads: self
                .payloads
                .iter()
                .map(RobotPayloadRuntime::snapshot)
                .collect(),
            tools: self.tools.iter().map(RobotToolRuntime::snapshot).collect(),
            handoffs: self
                .handoffs
                .iter()
                .map(RobotHandoffRuntime::snapshot)
                .collect(),
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
                progress_percent: percentage_to_f64(self.motion.progress()),
                elapsed_ms: self.motion.elapsed_ms(),
                duration_ms: self.motion.duration_ms(),
                speed_percent: percentage_to_f64(self.motion.speed_percent()),
                target_pose: hmi_pose(self.motion.target_pose()),
                target_joints: joint_degrees(self.motion.target_joints()),
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
            taught_positions: self
                .taught_positions
                .iter()
                .map(RobotTaughtPositionRuntime::snapshot)
                .collect(),
            workspace: hmi_workspace(self.motion.workspace()),
        }
    }

    fn taught_pose(&self, id: &str) -> Option<RobotPose> {
        self.taught_positions
            .iter()
            .find(|position| position.id == id)
            .map(|position| position.pose)
    }

    fn handoff(&self, mould: &str) -> Option<&RobotHandoffRuntime> {
        self.handoffs.iter().find(|handoff| handoff.mould == mould)
    }
}

impl RobotTaughtPositionRuntime {
    fn snapshot(&self) -> HmiRobotTaughtPosition {
        HmiRobotTaughtPosition {
            id: self.id.clone(),
            label: self.label.clone(),
            pose: hmi_pose(self.pose),
        }
    }
}

impl RobotFrameRuntime {
    fn snapshot(&self) -> HmiRobotFrame {
        HmiRobotFrame {
            id: self.id.clone(),
            label: self.label.clone(),
            parent: self.parent.clone(),
            pose: hmi_pose(self.pose),
        }
    }
}

impl RobotPayloadRuntime {
    fn snapshot(&self) -> HmiRobotPayload {
        HmiRobotPayload {
            id: self.id.clone(),
            label: self.label.clone(),
            mass_kg: super::super::quantities::project(self.mass_kg),
            center_of_mass_mm: self.center_of_mass.map(position_to_mm),
        }
    }
}

impl RobotToolRuntime {
    fn snapshot(&self) -> HmiRobotTool {
        HmiRobotTool {
            id: self.id.clone(),
            label: self.label.clone(),
            tcp: hmi_pose(self.tcp),
            payload: self.payload.clone(),
        }
    }
}

impl RobotHandoffRuntime {
    fn snapshot(&self) -> HmiRobotHandoff {
        HmiRobotHandoff {
            mould: self.mould.clone(),
            program: self.program.clone(),
            user_frame: self.user_frame.clone(),
            approach_position: self.approach_position.clone(),
            pickup_position: self.pickup_position.clone(),
            handoff_position: self.handoff_position.clone(),
            retreat_position: self.retreat_position.clone(),
            pickup_tolerance_mm: position_to_mm(self.pickup_tolerance),
            handoff_tolerance_mm: position_to_mm(self.handoff_tolerance),
            orientation_tolerance_deg: angle_to_degrees(self.orientation_tolerance),
        }
    }
}

fn robot_config_error(error: RobotMotionError) -> ConfigError {
    ConfigError::new(format!("invalid robot fixed-point quantity: {error:?}"))
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
