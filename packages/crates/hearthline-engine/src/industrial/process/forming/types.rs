#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormingPhase {
    Idle,
    Filling,
    Pressurizing,
    PressureDwell,
    Depressurizing,
    Draining,
    ReleaseWater,
    ReleaseAir,
    OpeningMould,
    RobotPickup,
    RobotDelivery,
    MouldWash,
    AirPurge,
    VacuumDry,
    ClosingMould,
    Faulted,
}

impl FormingPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Filling => "mould-filling",
            Self::Pressurizing => "air-pressurizing",
            Self::PressureDwell => "pressure-dwell",
            Self::Depressurizing => "depressurizing",
            Self::Draining => "excess-slip-drain",
            Self::ReleaseWater => "release-water",
            Self::ReleaseAir => "release-air",
            Self::OpeningMould => "mould-opening",
            Self::RobotPickup => "robot-pickup",
            Self::RobotDelivery => "operator-delivery",
            Self::MouldWash => "mould-wash",
            Self::AirPurge => "cleaning-air-purge",
            Self::VacuumDry => "vacuum-dry",
            Self::ClosingMould => "mould-closing",
            Self::Faulted => "faulted",
        }
    }

    pub(super) const fn duration_ms(self) -> u64 {
        match self {
            Self::Idle | Self::Faulted => 0,
            Self::Filling => 1_500,
            Self::Pressurizing => 750,
            Self::PressureDwell => 2_500,
            Self::Depressurizing => 500,
            Self::Draining => 1_000,
            Self::ReleaseWater | Self::ReleaseAir => 400,
            Self::OpeningMould | Self::ClosingMould | Self::AirPurge => 750,
            Self::RobotPickup | Self::MouldWash => 1_000,
            Self::RobotDelivery => 1_200,
            Self::VacuumDry => 1_500,
        }
    }

    pub(super) const fn next(self) -> Self {
        match self {
            Self::Idle | Self::Faulted => self,
            Self::Filling => Self::Pressurizing,
            Self::Pressurizing => Self::PressureDwell,
            Self::PressureDwell => Self::Depressurizing,
            Self::Depressurizing => Self::Draining,
            Self::Draining => Self::ReleaseWater,
            Self::ReleaseWater => Self::ReleaseAir,
            Self::ReleaseAir => Self::OpeningMould,
            Self::OpeningMould => Self::RobotPickup,
            Self::RobotPickup => Self::RobotDelivery,
            Self::RobotDelivery => Self::MouldWash,
            Self::MouldWash => Self::AirPurge,
            Self::AirPurge => Self::VacuumDry,
            Self::VacuumDry => Self::ClosingMould,
            Self::ClosingMould => Self::Idle,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FormingSetpoints {
    pub fill_ms: u64,
    pub pressure_bar: f64,
    pub dwell_ms: u64,
    pub drain_ms: u64,
    pub pickup_delay_ms: u64,
    pub wash_ms: u64,
    pub vacuum_ms: u64,
}

impl Default for FormingSetpoints {
    fn default() -> Self {
        Self {
            fill_ms: 1_500,
            pressure_bar: 6.0,
            dwell_ms: 2_500,
            drain_ms: 1_000,
            pickup_delay_ms: 400,
            wash_ms: 1_000,
            vacuum_ms: 1_500,
        }
    }
}

impl FormingSetpoints {
    pub const fn phase_duration_ms(self, phase: FormingPhase) -> u64 {
        match phase {
            FormingPhase::Filling => self.fill_ms,
            FormingPhase::PressureDwell => self.dwell_ms,
            FormingPhase::Draining => self.drain_ms,
            FormingPhase::ReleaseAir => self.pickup_delay_ms,
            FormingPhase::MouldWash => self.wash_ms,
            FormingPhase::VacuumDry => self.vacuum_ms,
            _ => phase.duration_ms(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormingFault {
    SlipSupplyLoss,
    CompressedAirLoss,
    MouldOverpressure,
    VacuumLoss,
    RobotPickupFailure,
}

impl FormingFault {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SlipSupplyLoss => "slip-supply-loss",
            Self::CompressedAirLoss => "compressed-air-loss",
            Self::MouldOverpressure => "mould-overpressure",
            Self::VacuumLoss => "vacuum-loss",
            Self::RobotPickupFailure => "robot-pickup-failure",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormingTrip {
    SlipFlowNotEstablished,
    CompressedAirLow,
    MouldOverpressure,
    VacuumNotEstablished,
    RobotPickupFailed,
}

impl FormingTrip {
    pub const fn code(self) -> &'static str {
        match self {
            Self::SlipFlowNotEstablished => "FORMING-SLIP-FLOW-NOT-ESTABLISHED",
            Self::CompressedAirLow => "FORMING-COMPRESSED-AIR-LOW",
            Self::MouldOverpressure => "FORMING-MOULD-OVERPRESSURE",
            Self::VacuumNotEstablished => "FORMING-VACUUM-NOT-ESTABLISHED",
            Self::RobotPickupFailed => "FORMING-ROBOT-PICKUP-FAILED",
        }
    }

    pub const fn message(self) -> &'static str {
        match self {
            Self::SlipFlowNotEstablished => {
                "The PLC aborted mould filling because ceramic-slip flow was not established."
            }
            Self::CompressedAirLow => {
                "The PLC aborted the sequence after compressed-air pressure fell below the operating limit."
            }
            Self::MouldOverpressure => {
                "The mould-pressure high-high limit tripped the casting sequence."
            }
            Self::VacuumNotEstablished => {
                "The PLC aborted cleaning because mould-drying vacuum was not established in time."
            }
            Self::RobotPickupFailed => {
                "The robot did not confirm the piece during the configured pickup window."
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormingStartError {
    AlreadyRunning,
    SafetyNotReady,
    FaultActive,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FormingMeasurements {
    pub slip_tank_level_percent: f64,
    pub slip_density_g_cm3: f64,
    pub slip_viscosity_mpa_s: f64,
    pub slip_temperature_c: f64,
    pub slip_feed_flow_l_min: f64,
    pub slip_feed_pressure_bar: f64,
    pub mould_pressure_bar: f64,
    pub mould_temperature_c: f64,
    pub fill_head_position_mm: f64,
    pub mould_position_mm: f64,
    pub water_flow_l_min: f64,
    pub excess_slip_drain_flow_l_min: f64,
    pub mould_moisture_percent: f64,
    pub compressed_air_pressure_bar: f64,
    pub vacuum_pressure_kpa: f64,
    pub robot_position_mm: f64,
    pub piece_gripped: bool,
    pub piece_moisture_percent: f64,
    pub predicted_drying_shrinkage_percent: f64,
    pub drying_energy_factor: f64,
    pub green_strength_index: f64,
    pub fired_defect_risk_percent: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormingOutputs {
    pub slip: &'static str,
    pub mould: &'static str,
    pub water: &'static str,
    pub air: &'static str,
    pub vacuum: &'static str,
    pub robot: &'static str,
}

impl FormingOutputs {
    pub(super) const fn safe() -> Self {
        Self {
            slip: "isolated",
            mould: "stopped",
            water: "isolated",
            air: "isolated",
            vacuum: "stopped",
            robot: "stopped",
        }
    }

    pub(super) const fn idle() -> Self {
        Self {
            slip: "recirculating",
            mould: "closed",
            water: "isolated",
            air: "isolated",
            vacuum: "stopped",
            robot: "home",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormingTick {
    pub phase_changed: bool,
    pub trip: Option<FormingTrip>,
}
