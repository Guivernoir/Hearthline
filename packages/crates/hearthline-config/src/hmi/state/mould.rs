use hearthline_engine::{
    FormingFault, FormingMeasurements, FormingOutputs, FormingPhase, FormingProcess,
    FormingSetpoints, FormingTrip, SequenceInputs,
};

use crate::hmi::actions::process::ConfiguredControlProgram;
use crate::hmi::{
    HmiMouldControlCabinet, HmiMouldProcessState, HmiMouldUtilityCabinet, HmiMouldUtilityCircuit,
    HmiProcessPhase,
};
use crate::{MouldControlCabinetConfig, MouldUtilityCabinetConfig};

use crate::hmi::schema::FORMING_PHASES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MouldStopRequest {
    AfterPhase,
    AfterCycle,
}

impl MouldStopRequest {
    const fn as_str(self) -> &'static str {
        match self {
            Self::AfterPhase => "after-phase",
            Self::AfterCycle => "after-cycle",
        }
    }
}

#[derive(Clone, Debug)]
pub(in crate::hmi) struct MouldProcessRuntime {
    target: String,
    label: String,
    process: FormingProcess,
    program: ConfiguredControlProgram,
    production_enabled: bool,
    paused: bool,
    stop_request: Option<MouldStopRequest>,
    control_cabinet_id: Option<String>,
    control_cabinet: Option<MouldControlCabinetConfig>,
    utility_cabinet_id: Option<String>,
    utility_cabinet: Option<MouldUtilityCabinetConfig>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct MouldTick {
    pub(super) phase_changes: u64,
    pub(super) trip: Option<FormingTrip>,
}

impl MouldProcessRuntime {
    pub(in crate::hmi) fn new(
        target: String,
        label: String,
        measurements: FormingMeasurements,
        program: ConfiguredControlProgram,
        setpoints: FormingSetpoints,
        control_cabinet: Option<(String, MouldControlCabinetConfig)>,
        utility_cabinet: Option<(String, MouldUtilityCabinetConfig)>,
    ) -> Self {
        Self {
            target,
            label,
            process: FormingProcess::new(measurements).with_setpoints(setpoints),
            program,
            production_enabled: false,
            paused: false,
            stop_request: None,
            control_cabinet_id: control_cabinet.as_ref().map(|(id, _)| id.clone()),
            control_cabinet: control_cabinet.map(|(_, cabinet)| cabinet),
            utility_cabinet_id: utility_cabinet.as_ref().map(|(id, _)| id.clone()),
            utility_cabinet: utility_cabinet.map(|(_, cabinet)| cabinet),
        }
    }

    pub(in crate::hmi) fn start(&mut self, safety_ready: bool) -> Result<(), &'static str> {
        if self.process.fault().is_some() || self.process.phase() == FormingPhase::Faulted {
            return Err("Clear the active fault and reset the safety circuit first.");
        }
        if !safety_ready {
            return Err("The mould requires healthy, reset safety permissives.");
        }
        if self.process.running() {
            return Err("The mould is already producing.");
        }
        self.production_enabled = true;
        self.stop_request = None;
        if self.paused {
            self.process
                .start_controlled(safety_ready, self.program.phase())
                .map_err(|_| "The paused mould could not resume.")?;
            self.paused = false;
            return Ok(());
        }
        self.program.execute_scan(SequenceInputs {
            start_request: true,
            safety_ready,
            ..SequenceInputs::default()
        });
        self.process
            .start_controlled(safety_ready, self.program.phase())
            .map_err(|_| "The mould could not start.")
    }

    pub(in crate::hmi) fn stop_after_phase(&mut self) -> Result<(), &'static str> {
        if self.paused {
            return Err("The mould is already paused at a phase boundary.");
        }
        if !self.process.running() {
            return Err("The mould is not producing.");
        }
        self.production_enabled = false;
        self.stop_request = Some(MouldStopRequest::AfterPhase);
        Ok(())
    }

    pub(in crate::hmi) fn end_after_cycle(
        &mut self,
        safety_ready: bool,
    ) -> Result<(), &'static str> {
        if !self.process.running() && !self.paused {
            self.production_enabled = false;
            self.stop_request = None;
            return Err("The mould is already stopped.");
        }
        self.production_enabled = false;
        self.stop_request = Some(MouldStopRequest::AfterCycle);
        if self.paused {
            if !safety_ready {
                return Err("The paused mould cannot finish while safety is not ready.");
            }
            self.process
                .start_controlled(safety_ready, self.program.phase())
                .map_err(|_| "The paused mould could not resume to finish its cycle.")?;
            self.paused = false;
        }
        Ok(())
    }

    pub(super) fn tick(
        &mut self,
        elapsed_ms: u64,
        safety_ready: bool,
        robot_pickup_permitted: bool,
        robot_delivery_permitted: bool,
    ) -> MouldTick {
        let mut result = MouldTick::default();
        if self.paused || (!self.process.running() && !self.production_enabled) {
            return result;
        }
        let mut remaining = elapsed_ms;
        while remaining > 0 && !self.paused {
            let slice = remaining.min(self.program.runtime().time_to_next_scan_ms());
            let before_phase = self.program.phase();
            let plant = self.process.tick_controlled(slice);
            remaining -= slice;
            if let Some(trip) = plant.trip {
                self.program.force_fault();
                self.production_enabled = false;
                self.stop_request = None;
                result.trip = Some(trip);
                break;
            }
            let timer_override = match before_phase {
                FormingPhase::RobotPickup if robot_pickup_permitted => Some(0),
                FormingPhase::RobotDelivery if robot_delivery_permitted => Some(0),
                FormingPhase::RobotPickup | FormingPhase::RobotDelivery => Some(u64::MAX),
                phase => Some(self.process.setpoints().phase_duration_ms(phase)),
            };
            self.program.elapse_with_timer_override(
                slice,
                SequenceInputs {
                    start_request: self.production_enabled,
                    safety_ready,
                    trip_active: self.program.runtime().running() && !safety_ready,
                    ..SequenceInputs::default()
                },
                timer_override,
            );
            let phase = self.program.phase();
            self.process.synchronize_control_state(
                phase,
                self.program.runtime().running(),
                self.program.runtime().scan_count(),
                self.program.runtime().cycle_count(),
            );
            if phase != before_phase {
                result.phase_changes = result.phase_changes.saturating_add(1);
                if self.stop_request == Some(MouldStopRequest::AfterPhase) {
                    self.stop_request = None;
                    if phase == FormingPhase::Idle {
                        self.paused = false;
                    } else {
                        self.process.pause_controlled(
                            phase,
                            self.program.runtime().scan_count(),
                            self.program.runtime().cycle_count(),
                        );
                        self.paused = true;
                    }
                    break;
                }
                if phase == FormingPhase::Idle
                    && self.stop_request == Some(MouldStopRequest::AfterCycle)
                {
                    self.stop_request = None;
                    self.paused = false;
                    break;
                }
            }
        }
        result
    }

    pub(in crate::hmi) fn set_fault(&mut self, fault: Option<FormingFault>) {
        self.process.set_fault(fault);
    }

    pub(in crate::hmi) fn reset_after_trip(&mut self, safety_ready: bool) -> bool {
        if !safety_ready
            || self.process.fault().is_some()
            || self.process.phase() != FormingPhase::Faulted
        {
            return false;
        }
        self.program.execute_scan(SequenceInputs {
            reset_request: true,
            safety_ready,
            ..SequenceInputs::default()
        });
        let reset = self.process.reset_after_trip(safety_ready);
        if reset {
            self.production_enabled = false;
            self.paused = false;
            self.stop_request = None;
        }
        reset
    }

    pub(in crate::hmi) fn trip_safety(&mut self) -> bool {
        if !self.process.running() {
            return false;
        }
        self.program.force_fault();
        self.production_enabled = false;
        self.paused = false;
        self.stop_request = None;
        self.process.synchronize_control_state(
            FormingPhase::Faulted,
            false,
            self.program.runtime().scan_count(),
            self.program.runtime().cycle_count(),
        );
        true
    }

    pub(super) fn snapshot(&self) -> HmiMouldProcessState {
        HmiMouldProcessState {
            target: self.target.clone(),
            label: self.label.clone(),
            phase: self.process.phase().as_str(),
            operating_state: self.operating_state(),
            running: self.process.running(),
            production_enabled: self.production_enabled,
            paused: self.paused,
            stop_request: self.stop_request.map(MouldStopRequest::as_str),
            phase_elapsed_ms: self.process.phase_elapsed_ms(),
            scan_count: self.process.scan_count(),
            cycle_count: self.process.cycle_count(),
            fault: self.process.fault().map(|fault| fault.as_str()),
            target_duration_ms: self
                .process
                .setpoints()
                .phase_duration_ms(self.process.phase()),
            casting_pressure_bar: self.process.setpoints().pressure_bar,
            setpoints_bound: true,
            control_cabinet: self
                .control_cabinet
                .as_ref()
                .map(|cabinet| HmiMouldControlCabinet {
                    remote_io: self.control_cabinet_id.clone().unwrap_or_default(),
                    enclosure_rating: cabinet.enclosure_rating.clone(),
                    control_voltage_vdc: cabinet.control_voltage_vdc,
                    safety_relay: cabinet.safety_relay.clone(),
                    modules: cabinet.modules.clone(),
                }),
            utility_cabinet: self.utility_cabinet.as_ref().map(|cabinet| {
                let active_state = utility_state(self.process.outputs());
                HmiMouldUtilityCabinet {
                    actuator: self.utility_cabinet_id.clone().unwrap_or_default(),
                    enclosure_rating: cabinet.enclosure_rating.clone(),
                    control_voltage_vdc: cabinet.control_voltage_vdc,
                    isolation_state: cabinet.isolation_state.clone(),
                    active_state: active_state.into(),
                    circuits: cabinet
                        .circuits
                        .iter()
                        .map(|circuit| HmiMouldUtilityCircuit {
                            id: circuit.id.clone(),
                            label: circuit.label.clone(),
                            medium: circuit.medium.to_string(),
                            source: circuit.source.clone(),
                            nominal_pressure: circuit.nominal_pressure,
                            state: if circuit
                                .command_states
                                .iter()
                                .any(|state| state == active_state)
                            {
                                active_state.into()
                            } else {
                                cabinet.isolation_state.clone()
                            },
                        })
                        .collect(),
                }
            }),
            phases: &FORMING_PHASES,
        }
    }

    pub(in crate::hmi) fn apply_parameter(&mut self, parameter_id: &str, value: f64) -> bool {
        let mut setpoints = self.process.setpoints();
        if parameter_id.ends_with("-fill-ms") {
            setpoints.fill_ms = value.round() as u64;
        } else if parameter_id.ends_with("-pressure-bar") {
            setpoints.pressure_bar = value;
        } else if parameter_id.ends_with("-dwell-ms") {
            setpoints.dwell_ms = value.round() as u64;
        } else if parameter_id.ends_with("-drain-ms") {
            setpoints.drain_ms = value.round() as u64;
        } else if parameter_id.ends_with("-pickup-delay-ms") {
            setpoints.pickup_delay_ms = value.round() as u64;
        } else if parameter_id.ends_with("-wash-ms") {
            setpoints.wash_ms = value.round() as u64;
        } else if parameter_id.ends_with("-vacuum-ms") {
            setpoints.vacuum_ms = value.round() as u64;
        } else {
            return false;
        }
        self.process.set_setpoints(setpoints);
        true
    }

    pub(super) const fn process(&self) -> &FormingProcess {
        &self.process
    }

    pub(super) const fn program(&self) -> &ConfiguredControlProgram {
        &self.program
    }

    pub(super) const fn measurements(&self) -> &FormingMeasurements {
        self.process.measurements()
    }

    pub(super) const fn outputs(&self) -> FormingOutputs {
        self.process.outputs()
    }

    pub(in crate::hmi) const fn running(&self) -> bool {
        self.process.running()
    }

    pub(super) const fn paused(&self) -> bool {
        self.paused
    }

    pub(super) const fn phase(&self) -> FormingPhase {
        self.process.phase()
    }

    pub(super) fn target(&self) -> &str {
        &self.target
    }

    fn operating_state(&self) -> &'static str {
        if self.process.phase() == FormingPhase::Faulted {
            "faulted"
        } else if self.paused {
            "paused"
        } else if self.stop_request == Some(MouldStopRequest::AfterPhase) {
            "stopping-after-phase"
        } else if self.stop_request == Some(MouldStopRequest::AfterCycle) {
            "ending-after-cycle"
        } else if self.process.running() {
            "producing"
        } else {
            "stopped"
        }
    }
}

fn utility_state(outputs: FormingOutputs) -> &'static str {
    match (outputs.slip, outputs.water, outputs.air, outputs.vacuum) {
        ("filling", _, _, _) => "slip-fill",
        ("draining", _, _, _) => "drain-under-pressure",
        (_, _, "pressurizing", _) => "casting-pressure",
        (_, "release-wet", _, _) => "release-water-both",
        (_, _, "release-assist", _) => "release-air-both",
        (_, "mould-wash", _, _) => "wash-water-both",
        (_, _, "cleaning-purge", _) => "cleaning-air-both",
        (_, _, _, "vacuum-drying") => "vacuum-dry",
        _ => "isolated",
    }
}

pub(super) fn aggregate_phase(states: &[HmiMouldProcessState]) -> &'static str {
    let Some(first) = states.first() else {
        return "idle";
    };
    if states.iter().all(|state| state.phase == first.phase) {
        first.phase
    } else {
        "mixed"
    }
}

pub(super) const fn phases() -> &'static [HmiProcessPhase] {
    &FORMING_PHASES
}
