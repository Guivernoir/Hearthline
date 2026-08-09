use hearthline_engine::{FormingFault, FormingPhase, FormingStartError, SequenceInputs};

use super::super::state::HmiSession;
use super::super::support::trace_entry;
use super::super::{HmiActionReport, HmiActionStatus, HmiProcessFault};

mod binding;
mod compiler;
mod control;
mod parser;

pub(crate) use control::{ConfiguredControlProgram, load_control_program};

impl HmiSession {
    pub(super) fn start_process(&mut self) -> HmiActionReport {
        let mut trace = Vec::new();
        if !self.has_permission("run-sequence") {
            return self.finish(
                HmiActionStatus::Denied,
                "Operator interface is not authorized to start the automatic sequence.".into(),
                "start-process",
                "forming-cycle",
                "denied",
                trace,
            );
        }
        let safety_ready = self.safety.iter().all(|safety| {
            !safety.trip_latched
                && safety
                    .permissives
                    .iter()
                    .all(|permissive| permissive.satisfied)
        });
        let Some(process) = &mut self.process else {
            return self.finish(
                HmiActionStatus::Denied,
                "This controller has no executable process model.".into(),
                "start-process",
                "forming-cycle",
                "denied",
                trace,
            );
        };
        let start = if let Some(program) = &mut self.controller.program {
            if process.running() {
                Err(FormingStartError::AlreadyRunning)
            } else if !safety_ready || process.phase() == FormingPhase::Faulted {
                Err(FormingStartError::SafetyNotReady)
            } else if process.fault().is_some() {
                Err(FormingStartError::FaultActive)
            } else {
                program.execute_scan(SequenceInputs {
                    start_request: true,
                    safety_ready,
                    ..SequenceInputs::default()
                });
                process.start_controlled(safety_ready, program.phase())
            }
        } else {
            process.start(safety_ready)
        };
        if let Err(reason) = start {
            let message = match reason {
                FormingStartError::AlreadyRunning => "The forming cycle is already running.",
                FormingStartError::SafetyNotReady => {
                    "The forming cycle requires healthy, reset machine permissives."
                }
                FormingStartError::FaultActive => {
                    "Clear the active simulated process fault before starting."
                }
            };
            return self.finish(
                HmiActionStatus::Denied,
                message.into(),
                "start-process",
                "forming-cycle",
                "denied",
                trace,
            );
        }
        trace.push(trace_entry(
            &self.id,
            "operator request",
            "authorized automatic-cycle start".into(),
        ));
        trace.push(trace_entry(
            &self.controller.id,
            "sequence control",
            "entered mould-filling phase on the next PLC scan".into(),
        ));
        self.tick(0);
        self.finish(
            HmiActionStatus::Applied,
            "Forming cycle started.".into(),
            "start-process",
            "forming-cycle",
            "applied",
            trace,
        )
    }

    pub(super) fn reset_process(&mut self) -> HmiActionReport {
        if !self.has_permission("run-sequence") {
            return self.finish(
                HmiActionStatus::Denied,
                "Operator interface is not authorized to reset the process sequence.".into(),
                "reset-process",
                "forming-cycle",
                "denied",
                Vec::new(),
            );
        }
        let safety_ready = self.safety.iter().all(|safety| !safety.trip_latched);
        let Some(process) = &mut self.process else {
            return self.finish(
                HmiActionStatus::Denied,
                "This controller has no executable process model.".into(),
                "reset-process",
                "forming-cycle",
                "denied",
                Vec::new(),
            );
        };
        let reset = if let Some(program) = &mut self.controller.program {
            if !safety_ready
                || process.fault().is_some()
                || process.phase() != FormingPhase::Faulted
            {
                false
            } else {
                program.execute_scan(SequenceInputs {
                    reset_request: true,
                    safety_ready,
                    ..SequenceInputs::default()
                });
                process.reset_after_trip(safety_ready)
            }
        } else {
            process.reset_after_trip(safety_ready)
        };
        if !reset {
            return self.finish(
                HmiActionStatus::Denied,
                "Process reset requires a cleared fault, a stopped faulted sequence, and healthy safety state."
                    .into(),
                "reset-process",
                "forming-cycle",
                "denied",
                Vec::new(),
            );
        }
        self.clear_process_alarms();
        self.tick(0);
        self.finish(
            HmiActionStatus::Applied,
            "Forming process trip reset.".into(),
            "reset-process",
            "forming-cycle",
            "applied",
            vec![trace_entry(
                &self.controller.id,
                "sequence reset",
                "faulted sequence returned to idle".into(),
            )],
        )
    }

    pub(super) fn set_process_fault(
        &mut self,
        fault: HmiProcessFault,
        active: bool,
    ) -> HmiActionReport {
        if !self.has_permission("inject-faults") {
            return self.finish(
                HmiActionStatus::Denied,
                "Fault injection is restricted to the simulator supervisory interface.".into(),
                "set-process-fault",
                "forming-cycle",
                "denied",
                Vec::new(),
            );
        }
        let modeled = match fault {
            HmiProcessFault::SlipSupplyLoss => FormingFault::SlipSupplyLoss,
            HmiProcessFault::CompressedAirLoss => FormingFault::CompressedAirLoss,
            HmiProcessFault::MouldOverpressure => FormingFault::MouldOverpressure,
            HmiProcessFault::VacuumLoss => FormingFault::VacuumLoss,
            HmiProcessFault::RobotPickupFailure => FormingFault::RobotPickupFailure,
        };
        let target = modeled.as_str();
        let Some(process) = &mut self.process else {
            return self.finish(
                HmiActionStatus::Denied,
                "This controller has no executable process model.".into(),
                "set-process-fault",
                target,
                "denied",
                Vec::new(),
            );
        };
        process.set_fault(active.then_some(modeled));
        self.tick(0);
        self.finish(
            HmiActionStatus::Applied,
            format!(
                "Process disturbance {target} {}.",
                if active { "enabled" } else { "cleared" }
            ),
            "set-process-fault",
            target,
            "applied",
            vec![trace_entry(
                &self.id,
                "simulation harness",
                format!("{target} active={active}"),
            )],
        )
    }

    pub(super) fn clear_process_alarms(&mut self) {
        for alarm in &mut self.alarms {
            if alarm.code.starts_with("FORMING-") {
                alarm.active = false;
            }
        }
    }
}
