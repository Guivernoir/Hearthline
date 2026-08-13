use hearthline_engine::{
    Actuator, OperatorInterface, RemoteIo, SafetyInterface, SimulatedComponent, SimulationEvent,
    VirtualPlc,
};
use hearthline_model::{ProcessCommand, ProcessEvent, ProcessSignal, SignalValue, Text};

use super::builder::support::{
    component_id, forwards_command, ports, produces_output, produces_true_output,
    signal_value_text, trace_entry,
};
use super::state::HmiSession;
use super::{HmiAction, HmiActionReport, HmiActionStatus, HmiAlarmSeverity};

mod guard;
mod operator;
pub(super) mod process;
mod robot;

impl HmiSession {
    pub fn execute(&mut self, action: HmiAction) -> HmiActionReport {
        self.sequence = self.sequence.saturating_add(1);
        if let Some(report) = self.inhibit_open_guard_motion(&action) {
            return report;
        }
        match action {
            HmiAction::Command { tag, value } => self.execute_command(tag, value),
            HmiAction::StartProcess => self.start_process(),
            HmiAction::StartMould => self.start_mould(),
            HmiAction::StopMouldAfterPhase => self.stop_mould_after_phase(),
            HmiAction::EndMouldAfterCycle => self.end_mould_after_cycle(),
            HmiAction::ResetProcess => self.reset_process(),
            HmiAction::SetProcessFault { fault, active } => self.set_process_fault(fault, active),
            HmiAction::ResetSafety { safety_id } => self.reset_safety(safety_id),
            HmiAction::SetGuardDoor { open } => self.set_guard_door(open),
            HmiAction::AcknowledgeAlarm { alarm_id } => self.acknowledge_alarm(alarm_id),
            HmiAction::SetControlMode { mode, password } => self.set_control_mode(mode, password),
            HmiAction::SetParameter {
                parameter_id,
                value,
            } => self.set_parameter(parameter_id, value),
            HmiAction::SelectRecipe { recipe_id } => self.select_recipe(recipe_id),
            HmiAction::SetRobotMotionEnable { enabled } => self.set_robot_motion_enable(enabled),
            HmiAction::MoveRobot {
                target,
                speed_percent,
            } => self.move_robot(target, speed_percent),
            HmiAction::MoveRobotToPosition {
                position_id,
                speed_percent,
            } => self.move_robot_to_position(position_id, speed_percent),
            HmiAction::JogRobot {
                coordinate_system,
                axis,
                increment,
                speed_percent,
            } => self.jog_robot(coordinate_system, axis, increment, speed_percent),
            HmiAction::TeachRobotPosition { position_id, label } => {
                self.teach_robot_position(position_id, label)
            }
            HmiAction::RunRobotProgram => self.run_robot_program(false),
            HmiAction::PauseRobotProgram => self.pause_robot_program(),
            HmiAction::StepRobotProgram => self.run_robot_program(true),
            HmiAction::ResetRobotProgram => self.reset_robot_program(),
            HmiAction::LoadRobotProgram { name, source } => self.load_robot_program(name, source),
        }
    }

    fn execute_command(&mut self, tag: String, value: String) -> HmiActionReport {
        let mut trace = Vec::new();
        if let Err(message) = self.authorize_manual_command(&tag) {
            return self.finish(
                HmiActionStatus::Denied,
                message,
                "command",
                &tag,
                "denied",
                trace,
            );
        }
        if self
            .safety
            .iter()
            .any(|state| self.safety_in_scope(&state.component_id) && state.trip_latched)
        {
            let source = self.id.clone();
            self.raise_alarm(
                "HMI-COMMAND-INHIBITED",
                &source,
                "Operator command inhibited by a latched safety trip.",
                HmiAlarmSeverity::Trip,
            );
            return self.finish(
                HmiActionStatus::Denied,
                "Command denied: reset the healthy safety circuit first.".into(),
                "command",
                &tag,
                "denied",
                trace,
            );
        }
        let target = self.local_mould_target().map(str::to_owned);
        if target
            .as_deref()
            .is_some_and(|target| self.mould_running(target))
            || (target.is_none() && self.any_mould_running())
        {
            return self.finish(
                HmiActionStatus::Denied,
                "Manual output command denied while the PLC automatic sequence is running.".into(),
                "command",
                &tag,
                "denied",
                trace,
            );
        }

        let command = ProcessCommand {
            tag: Text::from(tag.as_str()),
            value: SignalValue::Text(Text::from(value.as_str())),
            source: Text::from(self.id.as_str()),
        };
        let mut operator = OperatorInterface::with_kind(
            component_id(&self.id),
            self.kind,
            ports(&self.ports),
            self.command_tags
                .iter()
                .map(|configured| Text::from(configured.as_str())),
        );
        if !forwards_command(
            operator.handle(SimulationEvent::Process(ProcessEvent::Command(
                command.clone(),
            ))),
        ) {
            let source = self.id.clone();
            self.raise_alarm(
                "HMI-COMMAND-DENIED",
                &source,
                "The requested command tag is not authorized by HMI configuration.",
                HmiAlarmSeverity::Warning,
            );
            return self.finish(
                HmiActionStatus::Denied,
                "Command tag is not authorized.".into(),
                "command",
                &tag,
                "denied",
                trace,
            );
        }
        trace.push(trace_entry(
            &self.id,
            "authorization",
            format!("authorized {tag}={value}"),
        ));

        let Some(actuator_index) = self
            .actuators
            .iter()
            .position(|actuator| actuator.command_tag == tag)
        else {
            return self.finish(
                HmiActionStatus::Denied,
                "No configured actuator owns this command tag.".into(),
                "command",
                &tag,
                "denied",
                trace,
            );
        };
        if !self.actuators[actuator_index].states.contains(&value) {
            return self.finish(
                HmiActionStatus::Denied,
                format!("State '{value}' is not configured for this actuator."),
                "command",
                &tag,
                "denied",
                trace,
            );
        }

        let mut controller = VirtualPlc::new(
            component_id(&self.controller.id),
            ports(&self.controller.ports),
            self.controller.scan_interval_ms,
            [],
        );
        if !forwards_command(
            controller.handle(SimulationEvent::Process(ProcessEvent::Command(
                command.clone(),
            ))),
        ) {
            return self.finish(
                HmiActionStatus::Denied,
                "Controller rejected the command.".into(),
                "command",
                &tag,
                "denied",
                trace,
            );
        }
        trace.push(trace_entry(
            &self.controller.id,
            "controller",
            "accepted operator command".into(),
        ));

        let Some(configured_remote_io) = self
            .remote_io
            .iter()
            .find(|remote_io| {
                remote_io
                    .channels
                    .iter()
                    .any(|(channel, _)| channel == &tag)
            })
            .cloned()
        else {
            return self.finish(
                HmiActionStatus::Denied,
                "No configured remote I/O station maps this output.".into(),
                "command",
                &tag,
                "denied",
                trace,
            );
        };
        let mut remote_io = RemoteIo::new(
            component_id(&configured_remote_io.id),
            ports(&configured_remote_io.ports),
            configured_remote_io
                .channels
                .iter()
                .map(|(channel, direction)| (Text::from(channel.as_str()), *direction)),
        );
        if !produces_output(
            remote_io.handle(SimulationEvent::Process(ProcessEvent::Command(
                command.clone(),
            ))),
        ) {
            return self.finish(
                HmiActionStatus::Denied,
                "Remote I/O rejected the output mapping.".into(),
                "command",
                &tag,
                "denied",
                trace,
            );
        }
        trace.push(trace_entry(
            &configured_remote_io.id,
            "output mapping",
            format!("mapped output channel {tag}"),
        ));

        let configured = &self.actuators[actuator_index];
        let mut actuator = Actuator::new(
            component_id(&configured.component_id),
            Text::from(configured.command_tag.as_str()),
            SignalValue::Text(Text::from(configured.current_state.as_str())),
            SignalValue::Text(Text::from(configured.safe_state.as_str())),
        );
        if !produces_output(
            actuator.handle(SimulationEvent::Process(ProcessEvent::Command(command))),
        ) {
            return self.finish(
                HmiActionStatus::Denied,
                "Actuator rejected the command.".into(),
                "command",
                &tag,
                "denied",
                trace,
            );
        }
        self.actuators[actuator_index].current_state = signal_value_text(actuator.value());
        if tag == "area-02-robot-01-command"
            && let Some(robot) = &mut self.robot
        {
            robot.apply_manual_state(&value);
        }
        trace.push(trace_entry(
            &self.actuators[actuator_index].component_id,
            "field output",
            format!("state changed to {value}"),
        ));
        self.finish(
            HmiActionStatus::Applied,
            format!("{tag} changed to {value}."),
            "command",
            &tag,
            "applied",
            trace,
        )
    }

    fn reset_safety(&mut self, safety_id: String) -> HmiActionReport {
        let mut trace = Vec::new();
        if !self.has_permission("reset-safety") {
            return self.finish(
                HmiActionStatus::Denied,
                "HMI configuration does not grant safety-reset permission.".into(),
                "reset-safety",
                &safety_id,
                "denied",
                trace,
            );
        }
        if !self.safety_in_scope(&safety_id) {
            return self.finish(
                HmiActionStatus::Denied,
                "Safety interface is outside this HMI's configured authority.".into(),
                "reset-safety",
                &safety_id,
                "denied",
                trace,
            );
        }
        let Some(index) = self
            .safety
            .iter()
            .position(|state| state.component_id == safety_id)
        else {
            return self.finish(
                HmiActionStatus::Denied,
                "Unknown safety interface.".into(),
                "reset-safety",
                &safety_id,
                "denied",
                trace,
            );
        };
        let configured = self.safety[index].clone();
        let mut safety = SafetyInterface::new(
            component_id(&configured.component_id),
            configured
                .permissives
                .iter()
                .map(|permissive| Text::from(permissive.tag.as_str())),
        );
        for permissive in &configured.permissives {
            safety.handle(SimulationEvent::Process(ProcessEvent::Signal(
                ProcessSignal {
                    tag: Text::from(permissive.tag.as_str()),
                    value: SignalValue::Bool(permissive.satisfied),
                    quality_good: true,
                    timestamp_ms: self.sequence,
                },
            )));
        }
        trace.push(trace_entry(
            &configured.component_id,
            "permissive evaluation",
            format!(
                "{} of {} permissives satisfied",
                configured
                    .permissives
                    .iter()
                    .filter(|permissive| permissive.satisfied)
                    .count(),
                configured.permissives.len()
            ),
        ));
        let effects = safety.handle(SimulationEvent::Process(ProcessEvent::Reset {
            authorized: true,
        }));
        if !produces_true_output(effects) {
            return self.finish(
                HmiActionStatus::Denied,
                "Safety reset denied because permissives are not satisfied.".into(),
                "reset-safety",
                &safety_id,
                "denied",
                trace,
            );
        }
        self.safety[index].trip_latched = false;
        for alarm in &mut self.alarms {
            if alarm.source == safety_id {
                alarm.active = false;
            }
        }
        trace.push(trace_entry(
            &configured.component_id,
            "safety reset",
            "latched trip cleared".into(),
        ));
        if safety_id == "area-02-cell-guard-safe-01" {
            self.clear_guard_motion_trip();
        }
        let safety_ready = self.safety.iter().all(|safety| {
            !safety.trip_latched
                && safety
                    .permissives
                    .iter()
                    .all(|permissive| permissive.satisfied)
        });
        let process_resets = if safety_ready {
            self.reset_faulted_moulds()
        } else {
            0
        };
        if process_resets > 0 {
            self.clear_process_alarms();
            trace.push(trace_entry(
                &self.controller.id,
                "sequence reset",
                format!("cleared {process_resets} faulted mould sequence(s)"),
            ));
        }
        self.finish(
            HmiActionStatus::Applied,
            "Safety reset accepted.".into(),
            "reset-safety",
            &safety_id,
            "applied",
            trace,
        )
    }

    fn acknowledge_alarm(&mut self, alarm_id: String) -> HmiActionReport {
        if !self.has_permission("acknowledge-alarms") {
            return self.finish(
                HmiActionStatus::Denied,
                "HMI configuration does not grant alarm acknowledgement.".into(),
                "acknowledge-alarm",
                &alarm_id,
                "denied",
                Vec::new(),
            );
        }
        let Some(alarm) = self.alarms.iter_mut().find(|alarm| alarm.id == alarm_id) else {
            return self.finish(
                HmiActionStatus::Denied,
                "Unknown alarm.".into(),
                "acknowledge-alarm",
                &alarm_id,
                "denied",
                Vec::new(),
            );
        };
        alarm.acknowledged = true;
        let source = alarm.source.clone();
        self.finish(
            HmiActionStatus::Completed,
            "Alarm acknowledged.".into(),
            "acknowledge-alarm",
            &alarm_id,
            "completed",
            vec![trace_entry(
                &source,
                "alarm acknowledgement",
                format!("acknowledged {alarm_id}"),
            )],
        )
    }
}
