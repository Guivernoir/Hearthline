//! Wall-clock pacing does not alter numerical timestep or authoritative state.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackRate {
    One,
    Two,
    Five,
    Ten,
}

impl PlaybackRate {
    pub const fn multiplier(self) -> u64 {
        match self {
            Self::One => 1,
            Self::Two => 2,
            Self::Five => 5,
            Self::Ten => 10,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacingError {
    ZeroStep,
    Overflow,
    NoStepDue,
}

/// A bounded wall-time accumulator. A slow solver retains pending work;
/// pausing stops wall-time accrual without losing already elapsed simulation
/// time. The host must call commit_step only after a successful physical step.
pub struct PhysicsPacer {
    rate: PlaybackRate,
    paused: bool,
    step_us: u64,
    credit_us: u64,
    manual_steps: u64,
    completed_us: u64,
    elapsed_wall_us: u64,
}

impl PhysicsPacer {
    pub fn new(step_us: u64) -> Result<Self, PacingError> {
        if step_us == 0 {
            return Err(PacingError::ZeroStep);
        }
        Ok(Self {
            rate: PlaybackRate::One,
            paused: false,
            step_us,
            credit_us: 0,
            manual_steps: 0,
            completed_us: 0,
            elapsed_wall_us: 0,
        })
    }
    pub fn set_rate(&mut self, rate: PlaybackRate) {
        self.rate = rate;
    }
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }
    pub fn accrue(&mut self, elapsed_wall_us: u64) -> Result<(), PacingError> {
        if self.paused {
            return Ok(());
        }
        let credit = elapsed_wall_us
            .checked_mul(self.rate.multiplier())
            .and_then(|delta| self.credit_us.checked_add(delta))
            .ok_or(PacingError::Overflow)?;
        let wall = self
            .elapsed_wall_us
            .checked_add(elapsed_wall_us)
            .ok_or(PacingError::Overflow)?;
        self.credit_us = credit;
        self.elapsed_wall_us = wall;
        Ok(())
    }
    pub fn request_single_step(&mut self) -> Result<(), PacingError> {
        if !self.paused {
            return Err(PacingError::NoStepDue);
        }
        self.manual_steps = self
            .manual_steps
            .checked_add(1)
            .ok_or(PacingError::Overflow)?;
        Ok(())
    }
    pub fn step_due(&self) -> bool {
        if self.paused {
            self.manual_steps > 0
        } else {
            self.credit_us >= self.step_us
        }
    }
    pub fn commit_step(&mut self) -> Result<(), PacingError> {
        if !self.step_due() {
            return Err(PacingError::NoStepDue);
        }
        let completed = self
            .completed_us
            .checked_add(self.step_us)
            .ok_or(PacingError::Overflow)?;
        if self.paused {
            self.manual_steps -= 1;
        } else {
            self.credit_us -= self.step_us;
        }
        self.completed_us = completed;
        Ok(())
    }
    pub const fn completed_us(&self) -> u64 {
        self.completed_us
    }
    pub const fn elapsed_wall_us(&self) -> u64 {
        self.elapsed_wall_us
    }
    pub const fn pending_credit_us(&self) -> u64 {
        self.credit_us
    }
}
