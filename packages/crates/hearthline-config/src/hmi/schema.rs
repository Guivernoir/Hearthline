use serde::{Deserialize, Serialize};

pub const HMI_SCHEMA_VERSION: &str = "0.4.0";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiSnapshot {
    pub schema_version: &'static str,
    pub id: String,
    pub label: String,
    pub environment: String,
    pub zone: String,
    pub role: String,
    pub interface_kind: String,
    pub controller: String,
    pub remote_io: String,
    pub permissions: Vec<String>,
    pub sequence: u64,
    pub control_program: Option<HmiControlProgramState>,
    pub process: Option<HmiProcessState>,
    pub signals: Vec<HmiSignal>,
    pub actuators: Vec<HmiActuator>,
    pub safety: Vec<HmiSafety>,
    pub alarms: Vec<HmiAlarm>,
    pub audit: Vec<HmiAuditEntry>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiControlProgramState {
    pub language: &'static str,
    pub program: String,
    pub task: String,
    pub source_path: String,
    pub binding_path: String,
    pub revision: String,
    pub current_step: i64,
    pub scan_interval_ms: u64,
    pub watchdog_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiControlProgramDocument {
    pub schema_version: &'static str,
    pub controller: String,
    pub language: &'static str,
    pub program: String,
    pub task: String,
    pub source_path: String,
    pub binding_path: String,
    pub revision: String,
    pub source: String,
    pub binding_yaml: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiProcessState {
    pub model: &'static str,
    pub phase: &'static str,
    pub running: bool,
    pub phase_elapsed_ms: u64,
    pub scan_count: u64,
    pub cycle_count: u64,
    pub fault: Option<&'static str>,
    pub phases: &'static [HmiProcessPhase],
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiProcessPhase {
    pub key: &'static str,
    pub label: &'static str,
}

pub(super) const FORMING_PHASES: [HmiProcessPhase; 15] = [
    HmiProcessPhase {
        key: "idle",
        label: "Ready",
    },
    HmiProcessPhase {
        key: "mould-filling",
        label: "Fill",
    },
    HmiProcessPhase {
        key: "air-pressurizing",
        label: "Apply air pressure",
    },
    HmiProcessPhase {
        key: "pressure-dwell",
        label: "Pressure hold",
    },
    HmiProcessPhase {
        key: "depressurizing",
        label: "Depressurize",
    },
    HmiProcessPhase {
        key: "excess-slip-drain",
        label: "Drain slip",
    },
    HmiProcessPhase {
        key: "release-water",
        label: "Release water",
    },
    HmiProcessPhase {
        key: "release-air",
        label: "Release air",
    },
    HmiProcessPhase {
        key: "mould-opening",
        label: "Open mould",
    },
    HmiProcessPhase {
        key: "robot-pickup",
        label: "Robot pickup",
    },
    HmiProcessPhase {
        key: "operator-delivery",
        label: "Operator handoff",
    },
    HmiProcessPhase {
        key: "mould-wash",
        label: "Wash",
    },
    HmiProcessPhase {
        key: "cleaning-air-purge",
        label: "Cleaning air",
    },
    HmiProcessPhase {
        key: "vacuum-dry",
        label: "Vacuum dry",
    },
    HmiProcessPhase {
        key: "mould-closing",
        label: "Close mould",
    },
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiSignal {
    pub component_id: String,
    pub label: String,
    pub tag: String,
    pub unit: String,
    pub minimum: f64,
    pub maximum: f64,
    pub value: f64,
    pub quality_good: bool,
    pub timestamp_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiActuator {
    pub component_id: String,
    pub label: String,
    pub command_tag: String,
    pub feedback_tag: Option<String>,
    pub safe_state: String,
    pub states: Vec<String>,
    pub current_state: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiSafety {
    pub component_id: String,
    pub label: String,
    pub permissives: Vec<HmiPermissive>,
    pub trip_latched: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiPermissive {
    pub tag: String,
    pub satisfied: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiAlarm {
    pub id: String,
    pub code: String,
    pub source: String,
    pub message: String,
    pub severity: HmiAlarmSeverity,
    pub active: bool,
    pub acknowledged: bool,
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiAlarmSeverity {
    Warning,
    Trip,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiAuditEntry {
    pub sequence: u64,
    pub action: String,
    pub target: String,
    pub result: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum HmiAction {
    Command {
        tag: String,
        value: String,
    },
    StartProcess,
    ResetProcess,
    SetProcessFault {
        fault: HmiProcessFault,
        active: bool,
    },
    ResetSafety {
        safety_id: String,
    },
    AcknowledgeAlarm {
        alarm_id: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiProcessFault {
    SlipSupplyLoss,
    CompressedAirLoss,
    MouldOverpressure,
    VacuumLoss,
    RobotPickupFailure,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiActionStatus {
    Applied,
    Completed,
    Denied,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiActionReport {
    pub schema_version: &'static str,
    pub status: HmiActionStatus,
    pub message: String,
    pub trace: Vec<HmiTraceEntry>,
    pub snapshot: HmiSnapshot,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiTraceEntry {
    pub sequence: usize,
    pub component: String,
    pub stage: String,
    pub detail: String,
}
