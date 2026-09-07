use serde::Serialize;

use crate::HmiTraceEntry;

use super::super::ScenarioContinuityFault;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ScenarioContinuityReport {
    pub failed_appliance: String,
    pub promoted_appliance: String,
    pub failure_at_us: u64,
    pub last_heartbeat_us: u64,
    pub promotion_at_us: u64,
    pub interruption_us: u64,
    pub synchronized_sessions: usize,
    pub sessions_after_continuation: usize,
    pub replicated_updates: u64,
    pub sync_operational_at_failure: bool,
    pub faults: Vec<ScenarioContinuityFault>,
    pub continuation_expectation_met: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ScenarioHaIsolationReport {
    pub active_appliance: String,
    pub standby_appliance: String,
    pub isolation_at_us: u64,
    pub last_heartbeat_us: u64,
    pub evaluation_at_us: u64,
    pub promotion_inhibited_at_us: u64,
    pub active_members: usize,
    pub standby_sessions: usize,
    pub sync_operational: bool,
    pub peer_failure_confirmed: bool,
    pub continuation_expectation_met: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScenarioLocalAutonomyReport {
    pub hmi: String,
    pub controller: String,
    pub remote_io: String,
    pub safety_interface: String,
    pub actuator: String,
    pub command_tag: String,
    pub command_value: String,
    pub expected_actuator_state: String,
    pub actuator_state: String,
    pub outage_connections: Vec<String>,
    pub local_path_connections: Vec<String>,
    pub local_path_operational: bool,
    pub safety_reset_applied: bool,
    pub command_applied: bool,
    pub northbound_expectation_met: bool,
    pub autonomy_expectation_met: bool,
    pub control_trace: Vec<HmiTraceEntry>,
}

pub(crate) struct ScenarioContinuityEvidence {
    pub failed_appliance: String,
    pub promoted_appliance: String,
    pub failure_at_us: u64,
    pub last_heartbeat_us: u64,
    pub promotion_at_us: u64,
    pub synchronized_sessions: usize,
    pub sessions_after_continuation: usize,
    pub replicated_updates: u64,
    pub sync_operational_at_failure: bool,
    pub faults: Vec<ScenarioContinuityFault>,
}

pub(crate) struct ScenarioHaIsolationEvidence {
    pub active_appliance: String,
    pub standby_appliance: String,
    pub isolation_at_us: u64,
    pub last_heartbeat_us: u64,
    pub evaluation_at_us: u64,
    pub promotion_inhibited_at_us: u64,
    pub active_members: usize,
    pub standby_sessions: usize,
    pub sync_operational: bool,
    pub peer_failure_confirmed: bool,
}

pub(crate) struct ScenarioLocalAutonomyEvidence {
    pub hmi: String,
    pub controller: String,
    pub remote_io: String,
    pub safety_interface: String,
    pub actuator: String,
    pub command_tag: String,
    pub command_value: String,
    pub expected_actuator_state: String,
    pub actuator_state: String,
    pub outage_connections: Vec<String>,
    pub local_path_connections: Vec<String>,
    pub local_path_operational: bool,
    pub safety_reset_applied: bool,
    pub command_applied: bool,
    pub autonomy_expectation_met: bool,
    pub control_trace: Vec<HmiTraceEntry>,
}
