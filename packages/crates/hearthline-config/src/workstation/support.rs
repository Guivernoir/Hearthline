use crate::appliance::BehaviorConfig;
use crate::scenario::{
    LoadedScenario, ScenarioApplicationConfig, ScenarioConfig, ScenarioExpectation,
    ScenarioExpectedOutcome, ScenarioHttpMethod, ScenarioPacketConfig, ScenarioReport,
    ScenarioRepository, ScenarioTraceKind, ScenarioTransportConfig, is_interactive_scenario,
};
use crate::{ConfigRepository, WorkstationProfile, WorkstationSession};

use super::{
    BrowserNavigation, WORKSTATION_SCHEMA_VERSION, WorkstationActionKind, WorkstationActionReport,
    WorkstationActionStatus,
};

pub(super) fn find_dns_scenario<'a>(
    scenarios: &'a ScenarioRepository,
    source: &str,
) -> Option<&'a LoadedScenario> {
    scenarios.scenarios().find(|scenario| {
        is_interactive_scenario(&scenario.config, source)
            && matches!(
                scenario.config.packet.application,
                ScenarioApplicationConfig::DnsQuery { .. }
            )
    })
}

pub(super) fn find_http_scenario<'a>(
    scenarios: &'a ScenarioRepository,
    source: &str,
    method: ScenarioHttpMethod,
    path: &str,
    body: Option<&str>,
) -> Option<&'a LoadedScenario> {
    let matching_security_exercise = scenarios.scenarios().find(|scenario| {
        is_interactive_scenario(&scenario.config, source)
            && scenario.config.security.is_some()
            && matches!(
                &scenario.config.packet.application,
                ScenarioApplicationConfig::HttpRequest {
                    method: configured_method,
                    path: configured_path,
                    body: configured_body,
                    ..
                } if *configured_method == method
                    && configured_path == path
                    && configured_body.as_deref() == body
            )
    });
    matching_security_exercise.or_else(|| {
        scenarios.scenarios().find(|scenario| {
            is_interactive_scenario(&scenario.config, source)
                && scenario.config.security.is_none()
                && matches!(
                    scenario.config.packet.application,
                    ScenarioApplicationConfig::HttpRequest { .. }
                )
        })
    })
}

pub(super) fn find_service_scenario<'a>(
    scenarios: &'a ScenarioRepository,
    source: &str,
    expected_service: &str,
) -> Option<&'a LoadedScenario> {
    scenarios.scenarios().find(|scenario| {
        is_interactive_scenario(&scenario.config, source)
            && matches!(
                &scenario.config.packet.application,
                ScenarioApplicationConfig::Service { service }
                    if service.eq_ignore_ascii_case(expected_service)
            )
    })
}

pub(super) fn find_probe_scenario<'a>(
    scenarios: &'a ScenarioRepository,
    source: &str,
    destination: &str,
) -> Option<&'a LoadedScenario> {
    scenarios
        .scenarios()
        .filter(|scenario| {
            is_interactive_scenario(&scenario.config, source)
                && scenario.config.packet.destination_ip == destination
                && scenario.config.security.is_none()
        })
        .min_by_key(|scenario| scenario.config.participants.len())
}

pub(super) fn parse_ping(arguments: &[String]) -> Result<(u16, &str), String> {
    match arguments {
        [target] => Ok((4, target)),
        [flag, count, target] if flag.eq_ignore_ascii_case("-n") => {
            let count = count
                .parse::<u16>()
                .map_err(|_| "ping count must be a number between 1 and 4".to_owned())?;
            if !(1..=4).contains(&count) {
                return Err("ping count must be between 1 and 4".into());
            }
            Ok((count, target))
        }
        _ => Err("Usage: ping [-n COUNT] <host-or-ip>".into()),
    }
}

pub(super) fn ping_scenario(
    template: &ScenarioConfig,
    profile: &WorkstationProfile,
    destination: &str,
    identifier: u16,
    sequence: u16,
) -> ScenarioConfig {
    let mut scenario = template.clone();
    scenario.id = format!("interactive-{}-ping", profile.id);
    scenario.label = format!("Interactive ping from {}", profile.hostname);
    scenario.summary = format!(
        "Interactive ICMP echo probe from {} to {destination}.",
        profile.hostname
    );
    scenario.category = "interactive-diagnostic".into();
    scenario.packet = ScenarioPacketConfig {
        source_ip: template.packet.source_ip.clone(),
        destination_ip: destination.into(),
        ttl: 64,
        wire_length_bytes: 64,
        transport: ScenarioTransportConfig::IcmpEcho {
            identifier,
            sequence,
        },
        application: ScenarioApplicationConfig::None,
    };
    scenario.expectation = ScenarioExpectation {
        component: profile.id.clone(),
        outcome: ScenarioExpectedOutcome::Delivered,
        service: Some("icmp-echo".into()),
        target: None,
        reason_contains: None,
    };
    scenario.security = None;
    scenario
}

pub(super) fn dns_answer(
    appliances: &ConfigRepository,
    participants: &[String],
    name: &str,
) -> Option<String> {
    participants.iter().find_map(|id| {
        let appliance = appliances.get(id)?;
        let BehaviorConfig::ServiceHost { dns_records, .. } = &appliance.config.behavior else {
            return None;
        };
        dns_records
            .iter()
            .find(|record| record.name.eq_ignore_ascii_case(name))
            .map(|record| record.address.clone())
    })
}

pub(super) fn ipconfig_report(profile: &WorkstationProfile) -> WorkstationActionReport {
    let mut output = Vec::new();
    for interface in &profile.interfaces {
        output.push(format!("{} ({})", interface.id, interface.hardware));
        output.push(format!(
            "  State . . . . . . : {}/{}",
            interface.administrative_state, interface.operational_state
        ));
        output.push(format!(
            "  IPv4 address . . . : {}",
            interface
                .addresses
                .first()
                .map_or("not assigned", String::as_str)
        ));
        output.push(format!(
            "  MAC address  . . . : {}",
            interface.mac_address.as_deref().unwrap_or("not assigned")
        ));
    }
    output.push(format!(
        "Default gateway . . . : {}",
        profile
            .default_gateway
            .as_deref()
            .unwrap_or("not configured")
    ));
    output.push(format!(
        "DNS servers . . . . . : {}",
        if profile.dns_servers.is_empty() {
            "not configured".into()
        } else {
            profile.dns_servers.join(", ")
        }
    ));
    local_report(profile, "IP configuration", output)
}

pub(super) fn dns_cache_report(
    profile: &WorkstationProfile,
    session: &mut WorkstationSession,
) -> WorkstationActionReport {
    let entries = session.dns_entries();
    if entries.is_empty() {
        return local_report(
            profile,
            "DNS resolver cache",
            vec!["The DNS resolver cache is empty.".into()],
        );
    }
    let mut output = Vec::new();
    for entry in entries {
        output.push(format!("Record name . . . : {}", entry.name));
        output.push(format!("Address . . . . . : {}", entry.address));
        output.push(format!(
            "Remaining TTL . . : {} seconds",
            entry.remaining_ttl_ms.div_ceil(1_000)
        ));
        output.push(String::new());
    }
    local_report(profile, "DNS resolver cache", output)
}

pub(super) fn flush_dns_report(
    profile: &WorkstationProfile,
    session: &mut WorkstationSession,
) -> WorkstationActionReport {
    let removed = session.flush_dns();
    local_report(
        profile,
        "DNS resolver cache flushed",
        vec![format!(
            "Successfully flushed {removed} cached DNS record(s)."
        )],
    )
}

pub(super) fn arp_report(
    profile: &WorkstationProfile,
    session: &WorkstationSession,
) -> Result<WorkstationActionReport, crate::ConfigError> {
    let state = session.network_state()?;
    if state.arp_entries.is_empty() {
        return Ok(local_report(
            profile,
            "ARP table",
            vec!["No retained ARP entries.".into()],
        ));
    }
    let mut output = vec!["Interface          Internet Address      Physical Address".into()];
    for entry in state.arp_entries {
        output.push(format!(
            "{:<18} {:<21} {}",
            entry.interface, entry.address, entry.mac_address
        ));
    }
    Ok(local_report(profile, "ARP table", output))
}

pub(super) fn local_report(
    profile: &WorkstationProfile,
    title: impl Into<String>,
    output: Vec<String>,
) -> WorkstationActionReport {
    network_report(
        profile,
        WorkstationActionKind::Terminal,
        WorkstationActionStatus::Completed,
        title.into(),
        output,
        None,
        Vec::new(),
    )
}

pub(super) fn usage_report(profile: &WorkstationProfile, message: &str) -> WorkstationActionReport {
    network_report(
        profile,
        WorkstationActionKind::Terminal,
        WorkstationActionStatus::Unsupported,
        "Action unavailable".into(),
        vec![message.into()],
        None,
        Vec::new(),
    )
}

pub(super) fn clear_report(profile: &WorkstationProfile) -> WorkstationActionReport {
    let mut report = local_report(profile, "Terminal cleared", Vec::new());
    report.clear_output = true;
    report
}

pub(super) fn network_report(
    profile: &WorkstationProfile,
    action: WorkstationActionKind,
    status: WorkstationActionStatus,
    title: String,
    output: Vec<String>,
    browser: Option<BrowserNavigation>,
    simulations: Vec<ScenarioReport>,
) -> WorkstationActionReport {
    WorkstationActionReport {
        schema_version: WORKSTATION_SCHEMA_VERSION,
        workstation_id: profile.id.clone(),
        action,
        status,
        title,
        output,
        clear_output: false,
        browser,
        simulations,
        network_state: Default::default(),
    }
}

pub(super) fn network_status(report: &ScenarioReport) -> WorkstationActionStatus {
    if report.statistics.drops > 0 {
        WorkstationActionStatus::Denied
    } else if report.expectation_met {
        WorkstationActionStatus::Succeeded
    } else {
        WorkstationActionStatus::Failed
    }
}

pub(super) fn simulation_summary(report: &ScenarioReport) -> String {
    format!(
        "{}: {} links, {} trace events, {} us",
        if report.expectation_met {
            "Scenario expectation met"
        } else {
            "Scenario expectation not met"
        },
        report.link_count,
        report.statistics.events,
        report.duration_us
    )
}

pub(super) fn final_failure(report: &ScenarioReport) -> String {
    report
        .trace
        .iter()
        .rev()
        .find(|entry| matches!(entry.kind, ScenarioTraceKind::Drop))
        .map_or_else(
            || "No modeled application-forward result was produced".into(),
            |entry| format!("Blocked at {}: {}", entry.component, entry.summary),
        )
}
