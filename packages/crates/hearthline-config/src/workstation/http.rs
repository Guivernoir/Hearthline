use url::Url;

use crate::scenario::{
    ScenarioApplicationConfig, ScenarioHttpMethod, ScenarioReport, ScenarioRepository,
    ScenarioTraceKind,
};
use crate::{ConfigError, ConfigRepository, ConnectionRepository};

use super::executor::resolve_host;
use super::schema::{
    BrowserNavigation, WorkstationActionKind, WorkstationActionReport, WorkstationActionStatus,
    WorkstationProfile, WorkstationSession,
};
use super::support::{
    final_failure, find_http_scenario, network_report, network_status, simulation_summary,
};

pub(super) struct NavigationRequest<'a> {
    pub url: &'a str,
    pub method: ScenarioHttpMethod,
    pub body: Option<&'a str>,
    pub action: WorkstationActionKind,
}

impl<'a> NavigationRequest<'a> {
    pub(super) fn browser(url: &'a str) -> Self {
        Self {
            url,
            method: ScenarioHttpMethod::Get,
            body: None,
            action: WorkstationActionKind::Browser,
        }
    }
}

struct ResolvedRequest {
    action: WorkstationActionKind,
    url: Url,
    host: String,
    path: String,
    method: ScenarioHttpMethod,
    body: Option<String>,
    resolved_address: String,
    resolution_source: &'static str,
}

pub(super) fn navigate(
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    scenarios: &ScenarioRepository,
    profile: &WorkstationProfile,
    request: NavigationRequest<'_>,
    session: &mut WorkstationSession,
) -> Result<WorkstationActionReport, ConfigError> {
    if request.url.len() > 2_048 {
        return Err(ConfigError::new("browser URL exceeds 2048 bytes"));
    }
    let normalized = if request.url.contains("://") {
        request.url.to_owned()
    } else {
        format!("https://{}", request.url)
    };
    let url = Url::parse(&normalized)
        .map_err(|error| ConfigError::new(format!("invalid browser URL: {error}")))?;
    if url.scheme() != "https" {
        return Ok(unavailable(
            profile,
            request.action,
            "Only configured HTTPS navigation is currently executable",
        ));
    }
    if url.port().is_some_and(|port| port != 443) {
        return Ok(unavailable(
            profile,
            request.action,
            "Only the configured HTTPS port 443 is currently executable",
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(ConfigError::new("browser URL credentials are not accepted"));
    }
    let host = url
        .host_str()
        .ok_or_else(|| ConfigError::new("browser URL requires a host"))?
        .to_ascii_lowercase();
    let path = request_path(&url);
    let mut simulations = Vec::new();
    let resolution = resolve_host(appliances, connections, scenarios, profile, &host, session)?;
    if let Some(dns) = resolution.simulation {
        simulations.push(dns);
    }
    let Some(resolved_address) = resolution.address else {
        return Ok(name_resolution_failure(
            profile,
            request,
            url,
            host,
            path,
            simulations,
        ));
    };
    run_https(
        appliances,
        connections,
        scenarios,
        profile,
        ResolvedRequest {
            action: request.action,
            url,
            host,
            path,
            method: request.method,
            body: request.body.map(str::to_owned),
            resolved_address,
            resolution_source: resolution.source,
        },
        simulations,
        session,
    )
}

fn run_https(
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    scenarios: &ScenarioRepository,
    profile: &WorkstationProfile,
    request: ResolvedRequest,
    mut simulations: Vec<ScenarioReport>,
    session: &mut WorkstationSession,
) -> Result<WorkstationActionReport, ConfigError> {
    let scenario = find_http_scenario(
        scenarios,
        &profile.id,
        request.method,
        &request.path,
        request.body.as_deref(),
    )
    .ok_or_else(|| {
        ConfigError::new(format!(
            "no configured HTTPS scenario originates from {}",
            profile.id
        ))
    })?;
    let mut packet = scenario.config.packet.clone();
    packet.destination_ip = request.resolved_address.clone();
    packet.application = ScenarioApplicationConfig::HttpRequest {
        method: request.method,
        host: request.host.clone(),
        path: request.path.clone(),
        body_bytes: request.body.as_ref().map_or(0, String::len),
        body: request.body.clone(),
    };
    let simulation = session.run_scenario(
        appliances,
        connections,
        scenarios,
        &profile.id,
        &scenario.config,
        Some(packet),
    )?;
    let status = network_status(&simulation);
    let handoff = simulation
        .trace
        .iter()
        .rev()
        .find(|entry| matches!(entry.kind, ScenarioTraceKind::Application));
    let forwarded_to = handoff.and_then(|entry| entry.peer.clone());
    let gateway = handoff.map(|entry| entry.component.clone());
    let response = simulation.http_response.clone();
    let outcome = if response
        .as_ref()
        .is_some_and(|response| response.status < 400)
        && simulation.expectation_met
    {
        "responded"
    } else if simulation.statistics.drops > 0 {
        "denied"
    } else {
        "failed"
    };
    let mut output = vec![format!("{} {}", request.method.as_str(), request.url)];
    if request.resolution_source == "client-cache" {
        output.push(format!(
            "DNS cache: {} -> {}",
            request.host, request.resolved_address
        ));
    }
    output.push(match (&response, &forwarded_to) {
        (Some(response), Some(target)) => {
            format!(
                "HTTP {} returned by {target} through the modeled HTTPS path",
                response.status
            )
        }
        (Some(response), None) => {
            format!(
                "HTTP {} returned without a recorded upstream handoff",
                response.status
            )
        }
        (_, Some(target)) => format!("HTTPS request forwarded to {target} without a response"),
        (None, _) => final_failure(&simulation),
    });
    if let Some(body) = &request.body {
        output.push(format!("Request body: {} bytes", body.len()));
    }
    if let Some(event) = &simulation.security {
        output.push(format!(
            "{} security event: {} {} at {}; evidence prepared for {}",
            event.severity, event.control, event.technique, event.detector, event.defender
        ));
    }
    output.push(simulation_summary(&simulation));
    simulations.push(simulation);
    Ok(network_report(
        profile,
        request.action,
        status,
        format!("HTTPS {}: {}", request.method.as_str(), request.host),
        output,
        Some(BrowserNavigation {
            url: request.url.to_string(),
            method: request.method.as_str().into(),
            request_body_bytes: request.body.as_ref().map_or(0, String::len),
            host: request.host,
            path: request.path,
            resolved_address: Some(request.resolved_address),
            resolution_source: request.resolution_source,
            gateway,
            forwarded_to,
            response,
            outcome,
        }),
        simulations,
    ))
}

fn name_resolution_failure(
    profile: &WorkstationProfile,
    request: NavigationRequest<'_>,
    url: Url,
    host: String,
    path: String,
    simulations: Vec<ScenarioReport>,
) -> WorkstationActionReport {
    network_report(
        profile,
        request.action,
        WorkstationActionStatus::Failed,
        format!("Cannot resolve {host}"),
        vec![format!("DNS did not return a modeled address for {host}")],
        Some(BrowserNavigation {
            url: url.to_string(),
            method: request.method.as_str().into(),
            request_body_bytes: request.body.map_or(0, str::len),
            host,
            path,
            resolved_address: None,
            resolution_source: "dns-query",
            gateway: None,
            forwarded_to: None,
            response: None,
            outcome: "name-resolution-failed",
        }),
        simulations,
    )
}

fn unavailable(
    profile: &WorkstationProfile,
    action: WorkstationActionKind,
    message: &str,
) -> WorkstationActionReport {
    network_report(
        profile,
        action,
        WorkstationActionStatus::Unsupported,
        "Action unavailable".into(),
        vec![message.into()],
        None,
        Vec::new(),
    )
}

fn request_path(url: &Url) -> String {
    let mut path = url.path().to_owned();
    if let Some(query) = url.query() {
        path.push('?');
        path.push_str(query);
    }
    path
}
