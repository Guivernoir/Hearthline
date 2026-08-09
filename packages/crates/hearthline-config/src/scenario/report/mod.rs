use hearthline_engine::{Effect, TraceEntry};
use hearthline_model::{
    ApplicationData, ArpOperation, HttpDocument, NetworkPayload, TransportProtocol,
};
use serde::Serialize;

use crate::runtime::service_name;

use super::{
    ScenarioConfig, ScenarioConnectionState, ScenarioExpectation, ScenarioExpectedOutcome,
    ScenarioFirewallHaState, ScenarioFirstHopState, ScenarioLinkAggregationState,
    ScenarioPacketConfig, ScenarioSecurityEvent, ScenarioSpanningTreeState,
};

mod resilience;
pub(crate) use resilience::{
    ScenarioContinuityEvidence, ScenarioHaIsolationEvidence, ScenarioLocalAutonomyEvidence,
};
pub use resilience::{
    ScenarioContinuityReport, ScenarioHaIsolationReport, ScenarioLocalAutonomyReport,
};

pub const SCENARIO_REPORT_SCHEMA_VERSION: &str = "0.15.0";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScenarioStatus {
    Passed,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScenarioExpectationMode {
    Baseline,
    Recovery,
    Continuity,
    Isolation,
    Autonomy,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScenarioReport {
    pub schema_version: &'static str,
    pub scenario_id: String,
    pub scenario_label: String,
    pub status: ScenarioStatus,
    pub expectation_mode: ScenarioExpectationMode,
    pub expectation_met: bool,
    pub duration_us: u64,
    pub appliance_count: usize,
    pub link_count: usize,
    pub packet: ScenarioPacketConfig,
    pub connection_states: Vec<ScenarioConnectionState>,
    pub first_hop_states: Vec<ScenarioFirstHopState>,
    pub firewall_ha_states: Vec<ScenarioFirewallHaState>,
    pub link_aggregation_states: Vec<ScenarioLinkAggregationState>,
    pub spanning_tree_states: Vec<ScenarioSpanningTreeState>,
    pub expectation: ScenarioExpectation,
    pub statistics: ScenarioStatistics,
    pub http_response: Option<ScenarioHttpResponse>,
    pub security: Option<ScenarioSecurityEvent>,
    pub continuity: Option<ScenarioContinuityReport>,
    pub ha_isolation: Option<ScenarioHaIsolationReport>,
    pub local_autonomy: Option<ScenarioLocalAutonomyReport>,
    pub trace: Vec<ScenarioTraceEntry>,
}

pub(super) struct ScenarioExecutionEvidence<'a> {
    pub active_expectation: (ScenarioExpectationMode, &'a ScenarioExpectation),
    pub packet: ScenarioPacketConfig,
    pub appliance_count: usize,
    pub link_count: usize,
    pub connection_states: Vec<ScenarioConnectionState>,
    pub first_hop_states: Vec<ScenarioFirstHopState>,
    pub firewall_ha_states: Vec<ScenarioFirewallHaState>,
    pub link_aggregation_states: Vec<ScenarioLinkAggregationState>,
    pub spanning_tree_states: Vec<ScenarioSpanningTreeState>,
    pub continuity: Option<ScenarioContinuityEvidence>,
    pub ha_isolation: Option<ScenarioHaIsolationEvidence>,
    pub local_autonomy: Option<ScenarioLocalAutonomyEvidence>,
    pub trace: &'a [TraceEntry],
}

impl ScenarioReport {
    pub(super) fn from_trace(
        scenario: &ScenarioConfig,
        evidence: ScenarioExecutionEvidence<'_>,
    ) -> Self {
        let ScenarioExecutionEvidence {
            active_expectation: (expectation_mode, expectation),
            packet,
            appliance_count,
            link_count,
            connection_states,
            first_hop_states,
            firewall_ha_states,
            link_aggregation_states,
            spanning_tree_states,
            continuity,
            ha_isolation,
            local_autonomy,
            trace,
        } = evidence;
        let network_expectation_met = trace.iter().any(|entry| {
            if entry.component.as_str() != expectation.component {
                return false;
            }
            match (&expectation.outcome, &entry.effect) {
                (ScenarioExpectedOutcome::Delivered, Effect::Deliver { service, .. }) => {
                    expectation
                        .service
                        .as_deref()
                        .is_some_and(|expected| service_name(*service) == expected)
                }
                (
                    ScenarioExpectedOutcome::Forwarded,
                    Effect::ApplicationForward {
                        service, target, ..
                    },
                ) => {
                    expectation
                        .service
                        .as_deref()
                        .is_some_and(|expected| service_name(*service) == expected)
                        && expectation
                            .target
                            .as_deref()
                            .is_some_and(|expected| target.as_str() == expected)
                }
                (ScenarioExpectedOutcome::Dropped, Effect::Drop(reason)) => expectation
                    .reason_contains
                    .as_deref()
                    .is_none_or(|expected| reason.to_string().contains(expected)),
                _ => false,
            }
        });
        let expectation_met = network_expectation_met
            && local_autonomy
                .as_ref()
                .is_none_or(|autonomy| autonomy.autonomy_expectation_met);
        let statistics = ScenarioStatistics::from_trace(trace);
        let http_response = trace.iter().rev().find_map(|entry| {
            let Effect::Transmit { frame, .. } = &entry.effect else {
                return None;
            };
            let NetworkPayload::Ipv4(packet) = &frame.payload else {
                return None;
            };
            let ApplicationData::HttpResponse { status, document } = &packet.application else {
                return None;
            };
            Some(ScenarioHttpResponse::new(*status, document.as_ref()))
        });
        let security = scenario.security.clone().map(|config| {
            ScenarioSecurityEvent::from_trace(
                scenario.id.clone(),
                config,
                &packet,
                network_expectation_met,
                trace,
            )
        });
        Self {
            schema_version: SCENARIO_REPORT_SCHEMA_VERSION,
            scenario_id: scenario.id.clone(),
            scenario_label: scenario.label.clone(),
            status: if expectation_met {
                ScenarioStatus::Passed
            } else {
                ScenarioStatus::Failed
            },
            expectation_mode,
            expectation_met,
            duration_us: trace.last().map_or(0, |entry| entry.time_us),
            appliance_count,
            link_count,
            packet,
            connection_states,
            first_hop_states,
            firewall_ha_states,
            link_aggregation_states,
            spanning_tree_states,
            expectation: expectation.clone(),
            statistics,
            http_response,
            security,
            continuity: continuity.map(|continuity| ScenarioContinuityReport {
                failed_appliance: continuity.failed_appliance,
                promoted_appliance: continuity.promoted_appliance,
                failure_at_us: continuity.failure_at_us,
                last_heartbeat_us: continuity.last_heartbeat_us,
                promotion_at_us: continuity.promotion_at_us,
                interruption_us: continuity
                    .promotion_at_us
                    .saturating_sub(continuity.failure_at_us),
                synchronized_sessions: continuity.synchronized_sessions,
                sessions_after_continuation: continuity.sessions_after_continuation,
                replicated_updates: continuity.replicated_updates,
                sync_operational_at_failure: continuity.sync_operational_at_failure,
                faults: continuity.faults,
                continuation_expectation_met: expectation_met,
            }),
            ha_isolation: ha_isolation.map(|isolation| ScenarioHaIsolationReport {
                active_appliance: isolation.active_appliance,
                standby_appliance: isolation.standby_appliance,
                isolation_at_us: isolation.isolation_at_us,
                last_heartbeat_us: isolation.last_heartbeat_us,
                evaluation_at_us: isolation.evaluation_at_us,
                promotion_inhibited_at_us: isolation.promotion_inhibited_at_us,
                active_members: isolation.active_members,
                standby_sessions: isolation.standby_sessions,
                sync_operational: isolation.sync_operational,
                peer_failure_confirmed: isolation.peer_failure_confirmed,
                continuation_expectation_met: expectation_met,
            }),
            local_autonomy: local_autonomy.map(|autonomy| ScenarioLocalAutonomyReport {
                hmi: autonomy.hmi,
                controller: autonomy.controller,
                remote_io: autonomy.remote_io,
                safety_interface: autonomy.safety_interface,
                actuator: autonomy.actuator,
                command_tag: autonomy.command_tag,
                command_value: autonomy.command_value,
                expected_actuator_state: autonomy.expected_actuator_state,
                actuator_state: autonomy.actuator_state,
                outage_connections: autonomy.outage_connections,
                local_path_connections: autonomy.local_path_connections,
                local_path_operational: autonomy.local_path_operational,
                safety_reset_applied: autonomy.safety_reset_applied,
                command_applied: autonomy.command_applied,
                northbound_expectation_met: network_expectation_met,
                autonomy_expectation_met: expectation_met,
                control_trace: autonomy.control_trace,
            }),
            trace: trace
                .iter()
                .enumerate()
                .map(|(sequence, entry)| ScenarioTraceEntry::new(sequence, entry))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ScenarioHttpResponse {
    pub status: u16,
    pub document: Option<ScenarioHttpDocument>,
}

impl ScenarioHttpResponse {
    fn new(status: u16, document: Option<&HttpDocument>) -> Self {
        Self {
            status,
            document: document.map(|document| ScenarioHttpDocument {
                title: document.title.to_string(),
                heading: document.heading.to_string(),
                body: document.body.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ScenarioHttpDocument {
    pub title: String,
    pub heading: String,
    pub body: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ScenarioStatistics {
    pub events: usize,
    pub transmissions: usize,
    pub media_transits: usize,
    pub deliveries: usize,
    pub drops: usize,
    pub observations: usize,
}

impl ScenarioStatistics {
    fn from_trace(trace: &[TraceEntry]) -> Self {
        let mut statistics = Self {
            events: trace.len(),
            ..Self::default()
        };
        for entry in trace {
            match entry.effect {
                Effect::Transmit { .. } => statistics.transmissions += 1,
                Effect::MediaTransit { .. } => statistics.media_transits += 1,
                Effect::Deliver { .. } => statistics.deliveries += 1,
                Effect::Drop(_) => statistics.drops += 1,
                Effect::Observe { .. } => statistics.observations += 1,
                Effect::ApplicationForward { .. } | Effect::Process(_) => {}
            }
        }
        statistics
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ScenarioTraceEntry {
    pub sequence: usize,
    pub time_us: u64,
    pub component: String,
    pub kind: ScenarioTraceKind,
    pub summary: String,
    pub egress: Option<String>,
    pub connection: Option<String>,
    pub peer: Option<String>,
    pub source_ip: Option<String>,
    pub destination_ip: Option<String>,
    pub protocol: Option<String>,
}

impl ScenarioTraceEntry {
    fn new(sequence: usize, entry: &TraceEntry) -> Self {
        let mut projected = Self {
            sequence,
            time_us: entry.time_us,
            component: entry.component.to_string(),
            kind: ScenarioTraceKind::Observation,
            summary: String::new(),
            egress: None,
            connection: None,
            peer: None,
            source_ip: None,
            destination_ip: None,
            protocol: None,
        };
        match &entry.effect {
            Effect::Transmit {
                egress,
                next_hop,
                frame,
                delay_ms,
            } => {
                projected.kind = ScenarioTraceKind::Transmission;
                projected.egress = Some(egress.to_string());
                match &frame.payload {
                    NetworkPayload::Arp(packet) => {
                        let operation = match packet.operation {
                            ArpOperation::Request => "request",
                            ArpOperation::Reply => "reply",
                        };
                        projected.protocol = Some("arp".into());
                        projected.source_ip = Some(packet.sender_ip.to_string());
                        projected.destination_ip = Some(packet.target_ip.to_string());
                        projected.summary = format!(
                            "ARP {operation} {} -> {} on {egress}",
                            packet.sender_ip, packet.target_ip
                        );
                    }
                    NetworkPayload::Ipv4(packet) => {
                        let protocol = protocol_name(packet.transport.protocol());
                        projected.protocol = Some(protocol.into());
                        projected.source_ip = Some(packet.source.to_string());
                        projected.destination_ip = Some(packet.destination.to_string());
                        projected.summary = format!(
                            "{} {} -> {} on {}{}{}",
                            protocol.to_ascii_uppercase(),
                            endpoint(packet.source, packet.transport.source_port()),
                            endpoint(packet.destination, packet.transport.destination_port()),
                            egress,
                            next_hop.map_or_else(String::new, |hop| format!(" via {hop}")),
                            if *delay_ms == 0 {
                                String::new()
                            } else {
                                format!(" after {delay_ms} ms")
                            }
                        );
                    }
                    NetworkPayload::FirewallHa(message) => {
                        projected.protocol = Some("firewall-ha".into());
                        projected.summary = match message {
                            hearthline_model::FirewallHaMessage::Heartbeat {
                                domain,
                                sequence,
                                ..
                            } => {
                                format!("Firewall HA heartbeat {sequence} for {domain} on {egress}")
                            }
                            hearthline_model::FirewallHaMessage::SessionUpsert {
                                domain,
                                generation,
                                flow,
                                ..
                            } => format!(
                                "Firewall HA session update {generation} for {domain}: {} -> {} on {egress}",
                                endpoint(flow.source, flow.source_port),
                                endpoint(flow.destination, flow.destination_port)
                            ),
                        };
                    }
                }
            }
            Effect::Deliver { service, detail } => {
                projected.kind = ScenarioTraceKind::Delivery;
                projected.protocol = Some(service_name(*service).into());
                projected.summary = detail.to_string();
            }
            Effect::ApplicationForward {
                service,
                target,
                detail,
            } => {
                projected.kind = ScenarioTraceKind::Application;
                projected.protocol = Some(service_name(*service).into());
                projected.peer = Some(target.to_string());
                projected.summary = detail.to_string();
            }
            Effect::MediaTransit {
                connection,
                destination_component,
                destination_port,
                wire_bytes,
                queue_delay_us,
                serialization_us,
                propagation_us,
                arrival_us,
            } => {
                projected.kind = ScenarioTraceKind::Media;
                projected.connection = Some(connection.to_string());
                projected.peer = Some(destination_component.to_string());
                projected.summary = format!(
                    "{wire_bytes} B over {connection} to {destination_component}:{destination_port}; \
                     queue {queue_delay_us} us, serialization {serialization_us} us, \
                     propagation {propagation_us} us, arrival {arrival_us} us"
                );
            }
            Effect::Drop(reason) => {
                projected.kind = ScenarioTraceKind::Drop;
                projected.summary = reason.to_string();
            }
            Effect::Observe { detail } => {
                projected.summary = detail.to_string();
            }
            Effect::Process(effect) => {
                projected.kind = ScenarioTraceKind::Process;
                projected.summary = format!("{effect:?}");
            }
        }
        projected
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScenarioTraceKind {
    Transmission,
    Delivery,
    Application,
    Media,
    Drop,
    Observation,
    Process,
}

fn protocol_name(protocol: TransportProtocol) -> &'static str {
    match protocol {
        TransportProtocol::Icmp => "icmp",
        TransportProtocol::Tcp => "tcp",
        TransportProtocol::Udp => "udp",
        TransportProtocol::Other(_) => "other",
    }
}

fn endpoint(address: core::net::Ipv4Addr, port: Option<u16>) -> String {
    port.map_or_else(|| address.to_string(), |port| format!("{address}:{port}"))
}
