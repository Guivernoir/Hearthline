use std::collections::BTreeMap;

use hearthline_engine::{FormingProcess, IoDirection};
use hearthline_model::ComponentKind;

use crate::{BehaviorConfig, ConfigError, ConfigRepository};

use super::actions::process::ConfiguredControlProgram;
use super::schema::FORMING_PHASES;
use super::{
    HMI_SCHEMA_VERSION, HmiActionReport, HmiActionStatus, HmiActuator, HmiAlarm, HmiAlarmSeverity,
    HmiAuditEntry, HmiControlProgramDocument, HmiControlProgramState, HmiProcessState, HmiSafety,
    HmiSignal, HmiSnapshot, HmiTraceEntry,
};

const MAX_AUDIT_ENTRIES: usize = 32;

mod process;

#[derive(Clone, Debug)]
pub struct HmiSession {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) environment: String,
    pub(super) zone: String,
    pub(super) role: String,
    pub(super) kind: ComponentKind,
    pub(super) controller: ControllerRuntime,
    pub(super) remote_io: RemoteIoRuntime,
    pub(super) permissions: Vec<String>,
    pub(super) ports: Vec<String>,
    pub(super) command_tags: Vec<String>,
    pub(super) signals: Vec<HmiSignal>,
    pub(super) actuators: Vec<HmiActuator>,
    pub(super) safety: Vec<HmiSafety>,
    pub(super) alarms: Vec<HmiAlarm>,
    pub(super) audit: Vec<HmiAuditEntry>,
    pub(super) sequence: u64,
    pub(super) process: Option<FormingProcess>,
}

#[derive(Clone, Debug)]
pub(super) struct ControllerRuntime {
    pub(super) id: String,
    pub(super) ports: Vec<String>,
    pub(super) scan_interval_ms: u64,
    pub(super) program: Option<ConfiguredControlProgram>,
}

#[derive(Clone, Debug)]
pub(super) struct RemoteIoRuntime {
    pub(super) id: String,
    pub(super) ports: Vec<String>,
    pub(super) channels: Vec<(String, IoDirection)>,
}

impl HmiSession {
    pub fn control_program(&self) -> Option<HmiControlProgramDocument> {
        self.controller
            .program
            .as_ref()
            .map(|program| HmiControlProgramDocument {
                schema_version: HMI_SCHEMA_VERSION,
                controller: self.controller.id.clone(),
                language: "structured-text",
                program: program.program_name().into(),
                task: program.task_name().into(),
                source_path: program.source_path().into(),
                binding_path: program.binding_path().into(),
                revision: program.revision().into(),
                source: program.source().into(),
                binding_yaml: program.binding_yaml().into(),
            })
    }

    pub fn snapshot(&self) -> HmiSnapshot {
        HmiSnapshot {
            schema_version: HMI_SCHEMA_VERSION,
            id: self.id.clone(),
            label: self.label.clone(),
            environment: self.environment.clone(),
            zone: self.zone.clone(),
            role: self.role.clone(),
            interface_kind: self.kind.to_string(),
            controller: self.controller.id.clone(),
            remote_io: self.remote_io.id.clone(),
            permissions: self.permissions.clone(),
            sequence: self.sequence,
            control_program: self.controller.program.as_ref().map(|program| {
                HmiControlProgramState {
                    language: "structured-text",
                    program: program.program_name().into(),
                    task: program.task_name().into(),
                    source_path: program.source_path().into(),
                    binding_path: program.binding_path().into(),
                    revision: program.revision().into(),
                    current_step: program.runtime().current_step(),
                    scan_interval_ms: program.runtime().program().scan_interval_ms,
                    watchdog_ms: program.watchdog_ms(),
                }
            }),
            process: self.process.as_ref().map(|process| HmiProcessState {
                model: "ceramic-slip-pressure-casting-cell",
                phase: process.phase().as_str(),
                running: process.running(),
                phase_elapsed_ms: process.phase_elapsed_ms(),
                scan_count: process.scan_count(),
                cycle_count: process.cycle_count(),
                fault: process.fault().map(|fault| fault.as_str()),
                phases: &FORMING_PHASES,
            }),
            signals: self.signals.clone(),
            actuators: self.actuators.clone(),
            safety: self.safety.clone(),
            alarms: self.alarms.clone(),
            audit: self.audit.clone(),
        }
    }

    fn sync_shared_from(&mut self, shared: &Self) {
        self.sequence = shared.sequence;
        self.controller
            .program
            .clone_from(&shared.controller.program);
        self.process.clone_from(&shared.process);
        self.alarms.clone_from(&shared.alarms);
        for signal in &mut self.signals {
            if let Some(source) = shared
                .signals
                .iter()
                .find(|candidate| candidate.tag == signal.tag)
            {
                signal.value = source.value;
                signal.quality_good = source.quality_good;
                signal.timestamp_ms = source.timestamp_ms;
            }
        }
        for actuator in &mut self.actuators {
            if let Some(source) = shared
                .actuators
                .iter()
                .find(|candidate| candidate.command_tag == actuator.command_tag)
            {
                actuator.current_state.clone_from(&source.current_state);
            }
        }
        for safety in &mut self.safety {
            if let Some(source) = shared
                .safety
                .iter()
                .find(|candidate| candidate.component_id == safety.component_id)
            {
                safety.clone_from(source);
            }
        }
    }

    fn merge_shared_from(&mut self, source: &Self) {
        self.sequence = source.sequence;
        self.controller
            .program
            .clone_from(&source.controller.program);
        self.process.clone_from(&source.process);
        self.alarms.clone_from(&source.alarms);
        for signal in &source.signals {
            if let Some(target) = self
                .signals
                .iter_mut()
                .find(|candidate| candidate.tag == signal.tag)
            {
                target.clone_from(signal);
            }
        }
        for actuator in &source.actuators {
            if let Some(target) = self
                .actuators
                .iter_mut()
                .find(|candidate| candidate.command_tag == actuator.command_tag)
            {
                target.current_state.clone_from(&actuator.current_state);
            }
        }
        for safety in &source.safety {
            if let Some(target) = self
                .safety
                .iter_mut()
                .find(|candidate| candidate.component_id == safety.component_id)
            {
                target.clone_from(safety);
            }
        }
    }

    pub(super) fn has_permission(&self, permission: &str) -> bool {
        self.permissions
            .iter()
            .any(|candidate| candidate == permission)
    }

    pub(super) fn raise_alarm(
        &mut self,
        code: &str,
        source: &str,
        message: &str,
        severity: HmiAlarmSeverity,
    ) {
        if self
            .alarms
            .iter()
            .any(|alarm| alarm.active && alarm.code == code && alarm.source == source)
        {
            return;
        }
        self.alarms.push(HmiAlarm {
            id: format!("alarm-{}", self.sequence),
            code: code.into(),
            source: source.into(),
            message: message.into(),
            severity,
            active: true,
            acknowledged: false,
            sequence: self.sequence,
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn finish(
        &mut self,
        status: HmiActionStatus,
        message: String,
        action: &str,
        target: &str,
        result: &str,
        mut trace: Vec<HmiTraceEntry>,
    ) -> HmiActionReport {
        for (index, entry) in trace.iter_mut().enumerate() {
            entry.sequence = index;
        }
        self.record_audit(action, target, result);
        HmiActionReport {
            schema_version: HMI_SCHEMA_VERSION,
            status,
            message,
            trace,
            snapshot: self.snapshot(),
        }
    }

    fn record_audit(&mut self, action: &str, target: &str, result: &str) {
        self.audit.push(HmiAuditEntry {
            sequence: self.sequence,
            action: action.into(),
            target: target.into(),
            result: result.into(),
        });
        if self.audit.len() > MAX_AUDIT_ENTRIES {
            self.audit.remove(0);
        }
    }
}

#[derive(Debug, Default)]
pub struct HmiSessionStore {
    cells: BTreeMap<String, HmiSession>,
    sessions: BTreeMap<String, HmiSession>,
}

impl HmiSessionStore {
    pub fn control_program(
        &mut self,
        appliances: &ConfigRepository,
        id: &str,
    ) -> Result<Option<HmiControlProgramDocument>, ConfigError> {
        let cell = self.ensure(appliances, id)?;
        Ok(self
            .cells
            .get(&cell)
            .expect("HMI cell exists")
            .control_program())
    }

    pub fn profile(
        &mut self,
        appliances: &ConfigRepository,
        id: &str,
    ) -> Result<HmiSnapshot, ConfigError> {
        let cell = self.ensure(appliances, id)?;
        let shared = self.cells.get(&cell).expect("HMI cell exists").clone();
        let session = self.sessions.get_mut(id).expect("HMI session exists");
        session.sync_shared_from(&shared);
        Ok(session.snapshot())
    }

    pub fn execute(
        &mut self,
        appliances: &ConfigRepository,
        id: &str,
        action: super::HmiAction,
    ) -> Result<HmiActionReport, ConfigError> {
        let cell = self.ensure(appliances, id)?;
        let shared = self.cells.get(&cell).expect("HMI cell exists").clone();
        let session = self.sessions.get_mut(id).expect("HMI session exists");
        session.sync_shared_from(&shared);
        let report = session.execute(action);
        let updated = session.clone();
        self.cells
            .get_mut(&cell)
            .expect("HMI cell exists")
            .merge_shared_from(&updated);
        Ok(report)
    }

    pub fn tick(&mut self, elapsed_ms: u64) {
        for cell in self.cells.values_mut() {
            cell.tick(elapsed_ms);
        }
    }

    pub fn record_telemetry_publication(
        &mut self,
        appliances: &ConfigRepository,
        id: &str,
        delivered: bool,
    ) -> Result<(), ConfigError> {
        let cell = self.ensure(appliances, id)?;
        let shared = self.cells.get(&cell).expect("HMI cell exists").clone();
        let session = self.sessions.get_mut(id).expect("HMI session exists");
        session.sync_shared_from(&shared);
        session.sequence = session.sequence.saturating_add(1);
        session.record_audit(
            "publish-telemetry",
            "operations-analytics-01",
            if delivered { "delivered" } else { "failed" },
        );
        Ok(())
    }

    pub fn clear(&mut self) {
        self.cells.clear();
        self.sessions.clear();
    }

    fn ensure(&mut self, appliances: &ConfigRepository, id: &str) -> Result<String, ConfigError> {
        if !self.sessions.contains_key(id) {
            self.sessions
                .insert(id.into(), HmiSession::from_repository(appliances, id)?);
        }
        let controller = self
            .sessions
            .get(id)
            .expect("HMI session exists")
            .controller
            .id
            .clone();
        if !self.cells.contains_key(&controller) {
            let mut canonical = self.sessions.get(id).expect("HMI session exists").clone();
            let mut score = canonical.signals.len() + canonical.actuators.len();
            for candidate in appliances.appliances() {
                let BehaviorConfig::OperatorInterface {
                    controller: assigned,
                    ..
                } = &candidate.config.behavior
                else {
                    continue;
                };
                if assigned != &controller
                    || !candidate.config.tags.iter().any(|tag| tag == "interactive")
                {
                    continue;
                }
                let possible = HmiSession::from_repository(appliances, &candidate.config.id)?;
                let possible_score = possible.signals.len() + possible.actuators.len();
                if possible_score > score {
                    canonical = possible;
                    score = possible_score;
                }
            }
            canonical.tick(0);
            self.cells.insert(controller.clone(), canonical);
        }
        Ok(controller)
    }
}
