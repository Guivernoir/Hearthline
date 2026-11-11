//! Wall-clock pacing does not alter numerical timestep or authoritative state.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackRate {
    One,
    Two,
    Five,
    Ten,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackMode {
    Paced(PlaybackRate),
    Headless,
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
    InvalidBoundary,
    WrongCadence,
    Overflow,
    NoStepDue,
}

/// A bounded wall-time accumulator. A slow solver retains pending work;
/// pausing stops wall-time accrual without losing already elapsed simulation
/// time. The host must call commit_step only after a successful physical step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicsPacer {
    mode: PlaybackMode,
    paused: bool,
    cadence: Cadence,
    credit_us: u64,
    manual_steps: u64,
    completed_us: u64,
    elapsed_wall_us: u64,
    automatic_completed_us: u64,
    automatic_events: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Cadence {
    Fixed(u64),
    Events(Option<u64>),
}

impl PhysicsPacer {
    pub fn new(step_us: u64) -> Result<Self, PacingError> {
        Self::at_boundary(step_us, 0)
    }
    /// Wall-time statistics restart; accepted simulation time does not.
    pub fn at_boundary(step_us: u64, completed_us: u64) -> Result<Self, PacingError> {
        if step_us == 0 {
            return Err(PacingError::ZeroStep);
        }
        if !completed_us.is_multiple_of(step_us) {
            return Err(PacingError::InvalidBoundary);
        }
        Ok(Self {
            mode: PlaybackMode::Paced(PlaybackRate::One),
            paused: false,
            cadence: Cadence::Fixed(step_us),
            credit_us: 0,
            manual_steps: 0,
            completed_us,
            elapsed_wall_us: 0,
            automatic_completed_us: 0,
            automatic_events: 0,
        })
    }
    /// Equal-time events are valid. Their execution consumes no additional
    /// simulated duration, but still requires a manual step while paused.
    pub fn at_event_boundary(completed_us: u64, next_us: Option<u64>) -> Result<Self, PacingError> {
        let mut clock = Self::at_boundary(1, completed_us)?;
        clock.cadence = Cadence::Events(None);
        clock.set_next_event(next_us)?;
        Ok(clock)
    }
    pub fn set_next_event(&mut self, next_us: Option<u64>) -> Result<(), PacingError> {
        if !matches!(self.cadence, Cadence::Events(_)) {
            return Err(PacingError::WrongCadence);
        }
        if next_us.is_some_and(|next| next < self.completed_us) {
            return Err(PacingError::InvalidBoundary);
        }
        self.cadence = Cadence::Events(next_us);
        Ok(())
    }
    fn next_duration_us(&self) -> Option<u64> {
        match self.cadence {
            Cadence::Fixed(step) => Some(step),
            Cadence::Events(next) => next.map(|next| next - self.completed_us),
        }
    }
    pub fn set_rate(&mut self, rate: PlaybackRate) {
        self.mode = PlaybackMode::Paced(rate);
    }
    pub fn set_headless(&mut self) {
        self.mode = PlaybackMode::Headless;
        // Wall pacing debt is not physical state. Unpaced execution has none.
        self.credit_us = 0;
    }
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }
    pub fn accrue(&mut self, elapsed_wall_us: u64) -> Result<(), PacingError> {
        if self.paused {
            return Ok(());
        }
        let credit = match self.mode {
            PlaybackMode::Paced(rate) => elapsed_wall_us
                .checked_mul(rate.multiplier())
                .and_then(|delta| self.credit_us.checked_add(delta))
                .ok_or(PacingError::Overflow)?,
            PlaybackMode::Headless => 0,
        };
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
        let Some(step_us) = self.next_duration_us() else {
            return false;
        };
        if self.paused {
            self.manual_steps > 0
        } else {
            self.mode == PlaybackMode::Headless || self.credit_us >= step_us
        }
    }
    pub fn commit_step(&mut self) -> Result<(), PacingError> {
        if !self.step_due() {
            return Err(PacingError::NoStepDue);
        }
        let step_us = self.next_duration_us().ok_or(PacingError::NoStepDue)?;
        let completed = self
            .completed_us
            .checked_add(step_us)
            .ok_or(PacingError::Overflow)?;
        let automatic = if self.paused {
            self.automatic_completed_us
        } else {
            self.automatic_completed_us
                .checked_add(step_us)
                .ok_or(PacingError::Overflow)?
        };
        let automatic_events = self
            .automatic_events
            .checked_add(u64::from(!self.paused))
            .ok_or(PacingError::Overflow)?;
        if self.paused {
            self.manual_steps -= 1;
        } else if matches!(self.mode, PlaybackMode::Paced(_)) {
            self.credit_us -= step_us;
        }
        self.completed_us = completed;
        self.automatic_completed_us = automatic;
        self.automatic_events = automatic_events;
        if matches!(self.cadence, Cadence::Events(_)) {
            self.cadence = Cadence::Events(None);
        }
        Ok(())
    }
    pub const fn mode(&self) -> PlaybackMode {
        self.mode
    }
    pub const fn paused(&self) -> bool {
        self.paused
    }
    pub const fn automatic_completed_us(&self) -> u64 {
        self.automatic_completed_us
    }
    pub const fn automatic_events(&self) -> u64 {
        self.automatic_events
    }
    /// None means wait for a command; zero means process another physical step.
    pub fn wait_us(&self) -> Option<u64> {
        let step_us = self.next_duration_us()?;
        if self.step_due() {
            Some(0)
        } else if self.paused {
            None
        } else if let PlaybackMode::Paced(rate) = self.mode {
            Some((step_us - self.credit_us).div_ceil(rate.multiplier()))
        } else {
            Some(0)
        }
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
