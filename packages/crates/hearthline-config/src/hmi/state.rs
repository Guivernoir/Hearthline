use hearthline_engine::IoDirection;

use super::{
    HMI_SCHEMA_VERSION, HmiActionReport, HmiActionStatus, HmiActuator, HmiAlarm, HmiAlarmSeverity,
    HmiAuditEntry, HmiSafety, HmiSignal, HmiSnapshot, HmiTraceEntry,
};

const MAX_AUDIT_ENTRIES: usize = 32;

#[derive(Clone, Debug)]
pub struct HmiSession {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) environment: String,
    pub(super) zone: String,
    pub(super) role: String,
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
}

#[derive(Clone, Debug)]
pub(super) struct ControllerRuntime {
    pub(super) id: String,
    pub(super) ports: Vec<String>,
    pub(super) scan_interval_ms: u64,
}

#[derive(Clone, Debug)]
pub(super) struct RemoteIoRuntime {
    pub(super) id: String,
    pub(super) ports: Vec<String>,
    pub(super) channels: Vec<(String, IoDirection)>,
}

impl HmiSession {
    pub fn snapshot(&self) -> HmiSnapshot {
        HmiSnapshot {
            schema_version: HMI_SCHEMA_VERSION,
            id: self.id.clone(),
            label: self.label.clone(),
            environment: self.environment.clone(),
            zone: self.zone.clone(),
            role: self.role.clone(),
            controller: self.controller.id.clone(),
            remote_io: self.remote_io.id.clone(),
            permissions: self.permissions.clone(),
            sequence: self.sequence,
            signals: self.signals.clone(),
            actuators: self.actuators.clone(),
            safety: self.safety.clone(),
            alarms: self.alarms.clone(),
            audit: self.audit.clone(),
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
        self.audit.push(HmiAuditEntry {
            sequence: self.sequence,
            action: action.into(),
            target: target.into(),
            result: result.into(),
        });
        if self.audit.len() > MAX_AUDIT_ENTRIES {
            self.audit.remove(0);
        }
        HmiActionReport {
            schema_version: HMI_SCHEMA_VERSION,
            status,
            message,
            trace,
            snapshot: self.snapshot(),
        }
    }
}
