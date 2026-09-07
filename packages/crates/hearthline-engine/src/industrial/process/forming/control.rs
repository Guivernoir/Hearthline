use super::{FormingPhase, FormingTrip};

/// IEC-owned sequence state consumed by the Rust process model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormingControlState {
    pub phase: FormingPhase,
    pub running: bool,
    pub scan_count: u64,
    pub cycle_count: u64,
}

/// Physical and interlock conditions evaluated during one process slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormingPhysicsInputs {
    pub elapsed_ms: u64,
    pub control: FormingControlState,
    pub robot_pickup_permitted: bool,
    pub robot_delivery_permitted: bool,
}

/// Rust-owned process feedback returned to IEC sequence execution.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FormingPhysicsFeedback {
    pub trip: Option<FormingTrip>,
    pub phase_complete: bool,
}
