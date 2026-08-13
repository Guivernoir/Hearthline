mod dynamics;

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

    const fn duration_ms(self) -> u64 {
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

    const fn next(self) -> Self {
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
    const fn safe() -> Self {
        Self {
            slip: "isolated",
            mould: "stopped",
            water: "isolated",
            air: "isolated",
            vacuum: "stopped",
            robot: "stopped",
        }
    }

    const fn idle() -> Self {
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

#[derive(Clone, Debug)]
pub struct FormingProcess {
    phase: FormingPhase,
    phase_elapsed_ms: u64,
    scan_elapsed_ms: u64,
    scan_count: u64,
    cycle_count: u64,
    running: bool,
    fault: Option<FormingFault>,
    measurements: FormingMeasurements,
    outputs: FormingOutputs,
    tank_level_at_cycle_start: f64,
    setpoints: FormingSetpoints,
}

impl FormingProcess {
    pub const SCAN_INTERVAL_MS: u64 = 20;

    pub fn new(measurements: FormingMeasurements) -> Self {
        Self {
            phase: FormingPhase::Idle,
            phase_elapsed_ms: 0,
            scan_elapsed_ms: 0,
            scan_count: 0,
            cycle_count: 0,
            running: false,
            fault: None,
            measurements,
            outputs: FormingOutputs::idle(),
            tank_level_at_cycle_start: measurements.slip_tank_level_percent,
            setpoints: FormingSetpoints::default(),
        }
    }

    pub fn with_setpoints(mut self, setpoints: FormingSetpoints) -> Self {
        self.setpoints = setpoints;
        self
    }

    pub fn set_setpoints(&mut self, setpoints: FormingSetpoints) {
        self.setpoints = setpoints;
        self.apply_measurements();
    }

    pub const fn setpoints(&self) -> FormingSetpoints {
        self.setpoints
    }

    pub const fn phase(&self) -> FormingPhase {
        self.phase
    }

    pub const fn phase_elapsed_ms(&self) -> u64 {
        self.phase_elapsed_ms
    }

    pub const fn scan_count(&self) -> u64 {
        self.scan_count
    }

    pub const fn cycle_count(&self) -> u64 {
        self.cycle_count
    }

    pub const fn running(&self) -> bool {
        self.running
    }

    pub const fn fault(&self) -> Option<FormingFault> {
        self.fault
    }

    pub const fn measurements(&self) -> &FormingMeasurements {
        &self.measurements
    }

    pub const fn outputs(&self) -> FormingOutputs {
        self.outputs
    }

    pub fn start(&mut self, safety_ready: bool) -> Result<(), FormingStartError> {
        self.prepare_start(safety_ready, FormingPhase::Filling)?;
        self.apply_phase_outputs();
        Ok(())
    }

    pub fn start_controlled(
        &mut self,
        safety_ready: bool,
        phase: FormingPhase,
    ) -> Result<(), FormingStartError> {
        self.prepare_start(safety_ready, phase)?;
        self.apply_phase_outputs();
        Ok(())
    }

    fn prepare_start(
        &mut self,
        safety_ready: bool,
        phase: FormingPhase,
    ) -> Result<(), FormingStartError> {
        if self.running {
            return Err(FormingStartError::AlreadyRunning);
        }
        if !safety_ready || self.phase == FormingPhase::Faulted {
            return Err(FormingStartError::SafetyNotReady);
        }
        if self.fault.is_some() {
            return Err(FormingStartError::FaultActive);
        }
        self.running = true;
        self.phase = phase;
        self.phase_elapsed_ms = 0;
        self.tank_level_at_cycle_start = self.measurements.slip_tank_level_percent;
        self.measurements.piece_gripped = false;
        Ok(())
    }

    pub fn synchronize_control_state(
        &mut self,
        phase: FormingPhase,
        running: bool,
        scan_count: u64,
        cycle_count: u64,
    ) {
        let starting_cycle = !self.running
            && running
            && self.phase == FormingPhase::Idle
            && phase == FormingPhase::Filling;
        if self.phase != phase {
            self.phase = phase;
            self.phase_elapsed_ms = 0;
        }
        if starting_cycle {
            self.tank_level_at_cycle_start = self.measurements.slip_tank_level_percent;
            self.measurements.piece_gripped = false;
        }
        self.running = running;
        self.scan_count = scan_count;
        self.cycle_count = cycle_count;
        if phase == FormingPhase::Idle {
            self.measurements.piece_gripped = false;
        }
        self.apply_phase_outputs();
        self.apply_measurements();
    }

    pub fn pause_controlled(&mut self, phase: FormingPhase, scan_count: u64, cycle_count: u64) {
        self.phase = phase;
        self.phase_elapsed_ms = 0;
        self.scan_count = scan_count;
        self.cycle_count = cycle_count;
        self.running = false;
        self.outputs = FormingOutputs::safe();
        self.measurements.slip_feed_flow_l_min = 0.0;
        self.measurements.water_flow_l_min = 0.0;
        self.measurements.excess_slip_drain_flow_l_min = 0.0;
        self.measurements.vacuum_pressure_kpa = 0.0;
    }

    pub fn set_fault(&mut self, fault: Option<FormingFault>) {
        self.fault = fault;
        if fault.is_none() && !self.running {
            self.measurements.slip_feed_pressure_bar = 2.5;
            self.measurements.compressed_air_pressure_bar = 6.0;
            self.measurements.vacuum_pressure_kpa = 0.0;
        }
    }

    pub fn reset_after_trip(&mut self, safety_ready: bool) -> bool {
        if !safety_ready || self.fault.is_some() || self.phase != FormingPhase::Faulted {
            return false;
        }
        self.phase = FormingPhase::Idle;
        self.phase_elapsed_ms = 0;
        self.running = false;
        self.outputs = FormingOutputs::idle();
        self.measurements.mould_pressure_bar = 0.0;
        self.measurements.water_flow_l_min = 0.0;
        self.measurements.excess_slip_drain_flow_l_min = 0.0;
        self.measurements.vacuum_pressure_kpa = 0.0;
        true
    }

    pub fn tick(&mut self, elapsed_ms: u64) -> FormingTick {
        self.scan_elapsed_ms = self.scan_elapsed_ms.saturating_add(elapsed_ms);
        self.scan_count = self
            .scan_count
            .saturating_add(self.scan_elapsed_ms / Self::SCAN_INTERVAL_MS);
        self.scan_elapsed_ms %= Self::SCAN_INTERVAL_MS;
        if !self.running {
            return FormingTick {
                phase_changed: false,
                trip: None,
            };
        }

        self.phase_elapsed_ms = self.phase_elapsed_ms.saturating_add(elapsed_ms);
        self.apply_measurements();
        if let Some(trip) = self.evaluate_fault() {
            self.trip();
            return FormingTick {
                phase_changed: true,
                trip: Some(trip),
            };
        }

        let mut changed = false;
        while self.running && self.phase_elapsed_ms >= self.setpoints.phase_duration_ms(self.phase)
        {
            self.phase_elapsed_ms -= self.setpoints.phase_duration_ms(self.phase);
            self.phase = self.phase.next();
            changed = true;
            if self.phase == FormingPhase::Idle {
                self.running = false;
                self.cycle_count = self.cycle_count.saturating_add(1);
                self.measurements.piece_gripped = false;
            }
            self.apply_phase_outputs();
            self.apply_measurements();
        }
        FormingTick {
            phase_changed: changed,
            trip: None,
        }
    }

    pub fn tick_controlled(&mut self, elapsed_ms: u64) -> FormingTick {
        if !self.running {
            return FormingTick {
                phase_changed: false,
                trip: None,
            };
        }
        self.phase_elapsed_ms = self.phase_elapsed_ms.saturating_add(elapsed_ms);
        self.apply_measurements();
        if let Some(trip) = self.evaluate_fault() {
            self.trip();
            return FormingTick {
                phase_changed: true,
                trip: Some(trip),
            };
        }
        FormingTick {
            phase_changed: false,
            trip: None,
        }
    }
}
