use hearthline_config::{ScenarioPacketConfig, ScenarioSecurityConfig, SecuritySeverity};
use hearthline_engine::{Effect, TraceEntry};
use serde::Serialize;

pub const SECURITY_EVENT_SCHEMA_VERSION: &str = "0.1.0";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecurityDisposition {
    Prevented,
    ControlFailed,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScenarioSecurityEvent {
    pub schema_version: &'static str,
    pub scenario_id: String,
    pub tactic: String,
    pub technique: String,
    pub severity: SecuritySeverity,
    pub detector: String,
    pub defender: String,
    pub control: String,
    pub disposition: SecurityDisposition,
    pub source_ip: String,
    pub destination_ip: String,
    pub observed_at_us: u64,
    pub evidence: String,
}

impl ScenarioSecurityEvent {
    pub(super) fn from_trace(
        scenario_id: String,
        config: ScenarioSecurityConfig,
        packet: &ScenarioPacketConfig,
        expectation_met: bool,
        trace: &[TraceEntry],
    ) -> Self {
        let detection = trace.iter().rev().find(|entry| {
            entry.component.as_str() == config.detector && matches!(entry.effect, Effect::Drop(_))
        });
        let disposition = if expectation_met && detection.is_some() {
            SecurityDisposition::Prevented
        } else {
            SecurityDisposition::ControlFailed
        };
        let evidence = detection.map_or_else(
            || "No configured detector drop was present in the simulation trace".into(),
            |entry| match &entry.effect {
                Effect::Drop(reason) => reason.to_string(),
                _ => unreachable!("detection search only accepts drop effects"),
            },
        );
        Self {
            schema_version: SECURITY_EVENT_SCHEMA_VERSION,
            scenario_id,
            tactic: config.tactic,
            technique: config.technique,
            severity: config.severity,
            detector: config.detector,
            defender: config.defender,
            control: config.control,
            disposition,
            source_ip: packet.source_ip.clone(),
            destination_ip: packet.destination_ip.clone(),
            observed_at_us: detection
                .or_else(|| trace.last())
                .map_or(0, |entry| entry.time_us),
            evidence,
        }
    }
}
