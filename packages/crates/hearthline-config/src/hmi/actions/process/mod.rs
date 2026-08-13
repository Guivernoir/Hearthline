use hearthline_engine::FormingFault;

use super::super::builder::support::trace_entry;
use super::super::state::HmiSession;
use super::super::{HmiActionReport, HmiActionStatus, HmiControlMode, HmiProcessFault};

mod binding;
mod compiler;
mod control;
mod parser;

pub(crate) use control::{ConfiguredControlProgram, load_control_program};

impl HmiSession {
    pub(super) fn start_process(&mut self) -> HmiActionReport {
        self.finish(
            HmiActionStatus::Denied,
            "Cell-wide start is not available. Start each mould from its local HMI.".into(),
            "start-process",
            "forming-cell",
            "denied",
            Vec::new(),
        )
    }

    pub(super) fn start_mould(&mut self) -> HmiActionReport {
        let Some(target) = self.authorize_local_mould_production("start-mould") else {
            return self.mould_action_denied(
                "start-mould",
                "Only a mould-local HMI in auto can start its mould.",
            );
        };
        let safety_ready = self.mould_safety_ready(&target);
        let start = self
            .moulds
            .get_mut(&target)
            .expect("local mould runtime exists")
            .start(safety_ready);
        if let Err(message) = start {
            return self.mould_result(
                HmiActionStatus::Denied,
                "start-mould",
                &target,
                message,
                "denied",
            );
        }
        self.tick(0);
        self.mould_result(
            HmiActionStatus::Applied,
            "start-mould",
            &target,
            "Production enabled. The mould will repeat cycles until Stop or End is requested.",
            "applied",
        )
    }

    pub(super) fn stop_mould_after_phase(&mut self) -> HmiActionReport {
        let Some(target) = self.authorize_local_mould_production("stop-mould-after-phase") else {
            return self.mould_action_denied(
                "stop-mould-after-phase",
                "Only a mould-local HMI in auto can stop its mould.",
            );
        };
        let stop = self
            .moulds
            .get_mut(&target)
            .expect("local mould runtime exists")
            .stop_after_phase();
        if let Err(message) = stop {
            return self.mould_result(
                HmiActionStatus::Denied,
                "stop-mould-after-phase",
                &target,
                message,
                "denied",
            );
        }
        self.mould_result(
            HmiActionStatus::Applied,
            "stop-mould-after-phase",
            &target,
            "Stop requested. The current phase will finish before the mould pauses.",
            "applied",
        )
    }

    pub(super) fn end_mould_after_cycle(&mut self) -> HmiActionReport {
        let Some(target) = self.authorize_local_mould_production("end-mould-after-cycle") else {
            return self.mould_action_denied(
                "end-mould-after-cycle",
                "Only a mould-local HMI in auto can end its mould production.",
            );
        };
        let safety_ready = self.mould_safety_ready(&target);
        let end = self
            .moulds
            .get_mut(&target)
            .expect("local mould runtime exists")
            .end_after_cycle(safety_ready);
        if let Err(message) = end {
            return self.mould_result(
                HmiActionStatus::Denied,
                "end-mould-after-cycle",
                &target,
                message,
                "denied",
            );
        }
        self.mould_result(
            HmiActionStatus::Applied,
            "end-mould-after-cycle",
            &target,
            "End requested. The mould will complete its current cycle and then stop.",
            "applied",
        )
    }

    pub(super) fn reset_process(&mut self) -> HmiActionReport {
        if !self.has_permission("reset-safety") {
            return self.finish(
                HmiActionStatus::Denied,
                "This interface is not authorized to reset a process trip.".into(),
                "reset-process",
                "forming-cell",
                "denied",
                Vec::new(),
            );
        }
        let reset = self.reset_faulted_moulds();
        if reset == 0 {
            return self.finish(
                HmiActionStatus::Denied,
                "Reset requires a cleared fault and healthy mould safety state.".into(),
                "reset-process",
                "forming-cell",
                "denied",
                Vec::new(),
            );
        }
        self.clear_process_alarms();
        self.tick(0);
        self.finish(
            HmiActionStatus::Applied,
            format!("Reset {reset} faulted mould sequence(s)."),
            "reset-process",
            "forming-cell",
            "applied",
            vec![trace_entry(
                &self.controller.id,
                "sequence reset",
                format!("returned {reset} mould sequence(s) to idle"),
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
                "forming-cell",
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
        for mould in self.moulds.values_mut() {
            mould.set_fault(active.then_some(modeled));
        }
        self.tick(0);
        let target = modeled.as_str();
        self.finish(
            HmiActionStatus::Applied,
            format!(
                "Process disturbance {target} {} for all moulds.",
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

    fn authorize_local_mould_production(&self, permission: &str) -> Option<String> {
        let station = self.controller.stations.get(&self.id)?;
        (station.station_type == "mould-panel"
            && station.selected_mode == HmiControlMode::Auto
            && self.has_permission(permission))
        .then(|| station.target.clone())
    }

    fn mould_action_denied(&mut self, action: &str, message: &str) -> HmiActionReport {
        self.finish(
            HmiActionStatus::Denied,
            message.into(),
            action,
            "local-mould",
            "denied",
            Vec::new(),
        )
    }

    fn mould_result(
        &mut self,
        status: HmiActionStatus,
        action: &str,
        target: &str,
        message: &str,
        result: &str,
    ) -> HmiActionReport {
        self.finish(
            status,
            message.into(),
            action,
            target,
            result,
            vec![
                trace_entry(&self.id, "local operator request", message.into()),
                trace_entry(
                    &self.controller.id,
                    "mould sequence control",
                    format!("{action} accepted for {target}"),
                ),
            ],
        )
    }
}
