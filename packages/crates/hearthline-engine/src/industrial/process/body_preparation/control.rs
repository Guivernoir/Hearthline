use super::{BodyPreparationTick, BodyPreparationTrip, SlipPhase};

/// IEC-owned slip sequence state consumed by the Rust physical model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BodyPreparationControlState {
    pub phase: SlipPhase,
    pub running: bool,
    pub scan_count: u64,
    pub batch_count: u64,
}

/// Inputs for one controlled Body Preparation physical slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BodyPreparationPhysicsInputs {
    pub elapsed_ms: u64,
    pub control: BodyPreparationControlState,
    pub automatic_enabled: bool,
}

/// Rust-owned process feedback returned to IEC sequence execution.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BodyPreparationPhysicsFeedback {
    pub trip: Option<BodyPreparationTrip>,
    pub phase_complete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BodyPreparationControlledTick {
    pub process: BodyPreparationTick,
    pub physics: BodyPreparationPhysicsFeedback,
}
