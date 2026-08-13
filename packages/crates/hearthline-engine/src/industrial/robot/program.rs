use heapless::Vec as FixedList;

use super::{RobotMotionError, RobotMotionKind, RobotMotionRuntime, RobotPose};

pub const ROBOT_PROGRAM_CAPACITY: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RobotInstruction {
    Move {
        target: RobotPose,
        kind: RobotMotionKind,
        speed_percent: f64,
    },
    Dwell {
        duration_ms: u64,
    },
    Gripper {
        closed: bool,
    },
    End,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RobotProgramLine {
    pub source_line: u16,
    pub instruction: RobotInstruction,
}

#[derive(Clone, Debug, Default)]
pub struct RobotProgram {
    lines: FixedList<RobotProgramLine, ROBOT_PROGRAM_CAPACITY>,
}

impl RobotProgram {
    pub fn push(&mut self, line: RobotProgramLine) -> Result<(), RobotProgramLine> {
        self.lines.push(line)
    }

    pub fn lines(&self) -> &[RobotProgramLine] {
        &self.lines
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct RobotProgramRuntime {
    program: RobotProgram,
    active_index: usize,
    running: bool,
    paused: bool,
    single_step: bool,
    instruction_active: bool,
    dwell_remaining_ms: u64,
    cycle_count: u64,
    gripper_closed: bool,
}

impl RobotProgramRuntime {
    pub fn new(program: RobotProgram) -> Self {
        Self {
            program,
            active_index: 0,
            running: false,
            paused: false,
            single_step: false,
            instruction_active: false,
            dwell_remaining_ms: 0,
            cycle_count: 0,
            gripper_closed: false,
        }
    }

    pub fn replace(&mut self, program: RobotProgram) {
        *self = Self::new(program);
    }

    pub fn start(&mut self) -> bool {
        if self.program.is_empty() {
            return false;
        }
        if self.active_index >= self.program.lines().len() {
            self.active_index = 0;
        }
        self.running = true;
        self.paused = false;
        self.single_step = false;
        true
    }

    pub fn step(&mut self) -> bool {
        if self.program.is_empty() {
            return false;
        }
        self.running = true;
        self.paused = false;
        self.single_step = true;
        true
    }

    pub fn pause(&mut self) {
        self.running = false;
        self.paused = true;
    }

    pub fn reset(&mut self, motion: &mut RobotMotionRuntime) {
        motion.stop();
        self.active_index = 0;
        self.running = false;
        self.paused = false;
        self.single_step = false;
        self.instruction_active = false;
        self.dwell_remaining_ms = 0;
    }

    pub fn tick(
        &mut self,
        motion: &mut RobotMotionRuntime,
        elapsed_ms: u64,
    ) -> Result<(), RobotMotionError> {
        if self.instruction_active {
            if motion.active() {
                if motion.tick(elapsed_ms) {
                    self.complete_instruction();
                }
                return Ok(());
            }
            if self.dwell_remaining_ms > 0 {
                self.dwell_remaining_ms = self.dwell_remaining_ms.saturating_sub(elapsed_ms);
                if self.dwell_remaining_ms == 0 {
                    self.complete_instruction();
                }
                return Ok(());
            }
        }
        if !self.running {
            motion.tick(elapsed_ms);
            return Ok(());
        }

        loop {
            let Some(line) = self.program.lines().get(self.active_index).copied() else {
                self.finish_cycle();
                return Ok(());
            };
            match line.instruction {
                RobotInstruction::Move {
                    target,
                    kind,
                    speed_percent,
                } => {
                    motion.command_pose(target, kind, speed_percent)?;
                    if motion.active() {
                        self.instruction_active = true;
                        return Ok(());
                    }
                    self.complete_instruction();
                }
                RobotInstruction::Dwell { duration_ms } => {
                    self.dwell_remaining_ms = duration_ms;
                    self.instruction_active = duration_ms > 0;
                    if self.instruction_active {
                        return Ok(());
                    }
                    self.complete_instruction();
                }
                RobotInstruction::Gripper { closed } => {
                    self.gripper_closed = closed;
                    self.complete_instruction();
                }
                RobotInstruction::End => {
                    self.finish_cycle();
                    return Ok(());
                }
            }
            if !self.running {
                return Ok(());
            }
        }
    }

    pub fn lines(&self) -> &[RobotProgramLine] {
        self.program.lines()
    }

    pub fn active_source_line(&self) -> Option<u16> {
        self.program
            .lines()
            .get(self.active_index)
            .map(|line| line.source_line)
    }

    pub const fn running(&self) -> bool {
        self.running
    }

    pub const fn paused(&self) -> bool {
        self.paused
    }

    pub const fn cycle_count(&self) -> u64 {
        self.cycle_count
    }

    pub const fn gripper_closed(&self) -> bool {
        self.gripper_closed
    }

    fn complete_instruction(&mut self) {
        self.instruction_active = false;
        self.dwell_remaining_ms = 0;
        self.active_index = self.active_index.saturating_add(1);
        if self.single_step {
            self.running = false;
            self.paused = true;
            self.single_step = false;
        }
    }

    fn finish_cycle(&mut self) {
        self.active_index = 0;
        self.running = false;
        self.paused = false;
        self.single_step = false;
        self.instruction_active = false;
        self.dwell_remaining_ms = 0;
        self.cycle_count = self.cycle_count.saturating_add(1);
    }
}
