use crate::appliance::BehaviorConfig;
use crate::scenario::{
    LoadedScenario, ScenarioApplicationConfig, ScenarioHttpMethod, ScenarioReport,
    ScenarioRepository, ScenarioTraceKind,
};
use crate::{ConfigRepository, WorkstationProfile};

use super::{
    BrowserNavigation, WORKSTATION_SCHEMA_VERSION, WorkstationActionKind, WorkstationActionReport,
    WorkstationActionStatus,
};

pub(super) fn find_dns_scenario<'a>(
    scenarios: &'a ScenarioRepository,
    source: &str,
) -> Option<&'a LoadedScenario> {
    scenarios.scenarios().find(|scenario| {
        scenario.config.source == source
            && scenario.config.connection_overrides.is_empty()
            && scenario.config.first_hop_overrides.is_empty()
            && scenario.config.recovery.is_none()
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
        scenario.config.source == source
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
            scenario.config.source == source
                && scenario.config.security.is_none()
                && scenario.config.connection_overrides.is_empty()
                && scenario.config.first_hop_overrides.is_empty()
                && scenario.config.recovery.is_none()
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
        scenario.config.source == source
            && scenario.config.connection_overrides.is_empty()
            && scenario.config.first_hop_overrides.is_empty()
            && scenario.config.recovery.is_none()
            && matches!(
                &scenario.config.packet.application,
                ScenarioApplicationConfig::Service { service }
                    if service.eq_ignore_ascii_case(expected_service)
            )
    })
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
