use std::net::Ipv4Addr;

use crate::scenario::{ScenarioApplicationConfig, ScenarioReport, ScenarioRepository};
use crate::{ConfigError, ConfigRepository, ConnectionRepository};

use super::curl::parse_curl;
use super::http::{NavigationRequest, navigate};
use super::schema::{
    WorkstationAction, WorkstationActionKind, WorkstationActionReport, WorkstationActionStatus,
    WorkstationProfile, WorkstationSession, workstation_profile,
};
use super::shell::{runtime_inspection_report, split_command_line};
use super::support::{
    arp_report, clear_report, dns_answer, dns_cache_report, final_failure, find_dns_scenario,
    find_probe_scenario, find_service_scenario, flush_dns_report, ipconfig_report, local_report,
    network_report, network_status, parse_ping, ping_scenario, simulation_summary, usage_report,
};

pub fn run_workstation_action(
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    scenarios: &ScenarioRepository,
    workstation_id: &str,
    action: WorkstationAction,
) -> Result<WorkstationActionReport, ConfigError> {
    let mut session = WorkstationSession::default();
    run_workstation_action_with_session(
        appliances,
        connections,
        scenarios,
        workstation_id,
        action,
        &mut session,
    )
}

pub fn run_workstation_action_with_session(
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    scenarios: &ScenarioRepository,
    workstation_id: &str,
    action: WorkstationAction,
    session: &mut WorkstationSession,
) -> Result<WorkstationActionReport, ConfigError> {
    let profile = workstation_profile(appliances, scenarios, workstation_id)?;
    let mut report = match action {
        WorkstationAction::Terminal { command } => execute_terminal(
            appliances,
            connections,
            scenarios,
            &profile,
            command.trim(),
            session,
        ),
        WorkstationAction::Browser { url } => navigate(
            appliances,
            connections,
            scenarios,
            &profile,
            NavigationRequest::browser(&url),
            session,
        ),
        WorkstationAction::Inspect { appliance, command } => {
            runtime_inspection_report(&profile, session, &appliance, &command)
        }
    }?;
    report.network_state = session.network_state()?;
    Ok(report)
}

fn execute_terminal(
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    scenarios: &ScenarioRepository,
    profile: &WorkstationProfile,
    command: &str,
    session: &mut WorkstationSession,
) -> Result<WorkstationActionReport, ConfigError> {
    if command.len() > 512 {
        return Err(ConfigError::new("terminal command exceeds 512 bytes"));
    }
    let arguments = match split_command_line(command) {
        Ok(arguments) => arguments,
        Err(message) => return Ok(usage_report(profile, &message)),
    };
    let Some(program) = arguments.first() else {
        return Ok(local_report(profile, "No command", Vec::new()));
    };
    let arguments = &arguments[1..];
    match program.to_ascii_lowercase().as_str() {
        "help" | "?" => Ok(local_report(
            profile,
            "Terminal commands",
            vec![
                "help                  Show supported commands".into(),
                "hostname              Show this endpoint hostname".into(),
                "ipconfig              Show configured interfaces and resolvers".into(),
                "ipconfig /displaydns  Show cached DNS records".into(),
                "ipconfig /flushdns    Clear cached DNS records".into(),
                "arp -a                Show the retained endpoint ARP table".into(),
                "nslookup <name>       Run a configured DNS exchange".into(),
                "ping [-n COUNT] <host-or-ip>".into(),
                "curl [-I] [-X METHOD] [-d DATA] <https-url>".into(),
                "ssh <host>            Attempt public SSH through perimeter policy".into(),
                "clear                 Clear terminal output".into(),
            ],
        )),
        "hostname" => Ok(local_report(
            profile,
            "Hostname",
            vec![profile.hostname.clone()],
        )),
        "ipconfig" => match arguments {
            [] => Ok(ipconfig_report(profile)),
            [option] if option.eq_ignore_ascii_case("/displaydns") => {
                Ok(dns_cache_report(profile, session))
            }
            [option] if option.eq_ignore_ascii_case("/flushdns") => {
                Ok(flush_dns_report(profile, session))
            }
            _ => Ok(usage_report(
                profile,
                "Usage: ipconfig [/displaydns | /flushdns]",
            )),
        },
        "nslookup" => {
            let Some(name) = arguments.first() else {
                return Ok(usage_report(profile, "Usage: nslookup <name>"));
            };
            execute_dns(appliances, connections, scenarios, profile, name, session)
        }
        "arp" if matches!(arguments, [option] if option.eq_ignore_ascii_case("-a")) => {
            arp_report(profile, session)
        }
        "arp" => Ok(usage_report(profile, "Usage: arp -a")),
        "ping" => {
            let (count, target) = match parse_ping(arguments) {
                Ok(request) => request,
                Err(message) => return Ok(usage_report(profile, &message)),
            };
            execute_ping(
                appliances,
                connections,
                scenarios,
                profile,
                target,
                count,
                session,
            )
        }
        "curl" => {
            let request = match parse_curl(arguments) {
                Ok(request) => request,
                Err(message) => return Ok(usage_report(profile, &message)),
            };
            navigate(
                appliances,
                connections,
                scenarios,
                profile,
                NavigationRequest {
                    url: request.url,
                    method: request.method,
                    body: request.body,
                    action: WorkstationActionKind::Terminal,
                },
                session,
            )
        }
        "ssh" => {
            let Some(target) = arguments.first() else {
                return Ok(usage_report(profile, "Usage: ssh <host>"));
            };
            execute_ssh(appliances, connections, scenarios, profile, target, session)
        }
        "clear" => Ok(clear_report(profile)),
        _ => Ok(network_report(
            profile,
            WorkstationActionKind::Terminal,
            WorkstationActionStatus::Unsupported,
            "Unsupported command".into(),
            vec![format!(
                "'{program}' is not implemented. Run 'help' for available commands."
            )],
            None,
            Vec::new(),
        )),
    }
}

fn execute_ping(
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    scenarios: &ScenarioRepository,
    profile: &WorkstationProfile,
    target: &str,
    count: u16,
    session: &mut WorkstationSession,
) -> Result<WorkstationActionReport, ConfigError> {
    let mut simulations = Vec::new();
    let resolution = resolve_host(appliances, connections, scenarios, profile, target, session)?;
    if let Some(dns) = resolution.simulation {
        simulations.push(dns);
    }
    let Some(destination) = resolution.address else {
        return Ok(network_report(
            profile,
            WorkstationActionKind::Terminal,
            WorkstationActionStatus::Failed,
            format!("Ping: {target}"),
            vec![format!("Ping request could not resolve host {target}.")],
            None,
            simulations,
        ));
    };
    let Some(template) = find_probe_scenario(scenarios, &profile.id, &destination) else {
        return Ok(network_report(
            profile,
            WorkstationActionKind::Terminal,
            WorkstationActionStatus::Failed,
            format!("Ping: {target}"),
            vec![
                format!("Pinging {target} [{destination}]"),
                "No configured route template covers this destination.".into(),
            ],
            None,
            simulations,
        ));
    };

    let mut output = Vec::new();
    if resolution.source == "client-cache" {
        output.push(format!("DNS cache: {target} -> {destination}"));
    }
    output.push(format!(
        "Pinging {target} [{destination}] with 32 bytes of data:"
    ));
    let mut reply_times = Vec::new();
    let identifier = u16::try_from(
        profile
            .id
            .bytes()
            .fold(0_u32, |sum, byte| sum.wrapping_add(u32::from(byte))),
    )
    .unwrap_or(u16::MAX);
    for sequence in 1..=count {
        let scenario = ping_scenario(
            &template.config,
            profile,
            &destination,
            identifier,
            sequence,
        );
        let report = session.run_scenario(
            appliances,
            connections,
            scenarios,
            &profile.id,
            &scenario,
            None,
        )?;
        if report.expectation_met {
            output.push(format!(
                "Reply from {destination}: bytes=32 time={}us TTL=64",
                report.duration_us
            ));
            reply_times.push(report.duration_us);
        } else {
            output.push(format!("Request timed out. {}", final_failure(&report)));
        }
        simulations.push(report);
    }

    let sent = usize::from(count);
    let received = reply_times.len();
    let lost = sent.saturating_sub(received);
    output.push(String::new());
    output.push(format!("Ping statistics for {destination}:"));
    output.push(format!(
        "    Packets: Sent = {sent}, Received = {received}, Lost = {lost} ({}% loss)",
        lost.saturating_mul(100) / sent
    ));
    if let (Some(minimum), Some(maximum)) = (
        reply_times.iter().min().copied(),
        reply_times.iter().max().copied(),
    ) {
        let average = reply_times.iter().sum::<u64>() / received as u64;
        output.push("Approximate round trip times in microseconds:".into());
        output.push(format!(
            "    Minimum = {minimum}us, Maximum = {maximum}us, Average = {average}us"
        ));
    }
    let status = if received > 0 {
        WorkstationActionStatus::Succeeded
    } else {
        simulations
            .last()
            .map_or(WorkstationActionStatus::Failed, network_status)
    };
    Ok(network_report(
        profile,
        WorkstationActionKind::Terminal,
        status,
        format!("Ping: {target}"),
        output,
        None,
        simulations,
    ))
}

fn execute_dns(
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    scenarios: &ScenarioRepository,
    profile: &WorkstationProfile,
    name: &str,
    session: &mut WorkstationSession,
) -> Result<WorkstationActionReport, ConfigError> {
    let (simulation, answer) =
        run_dns_query(appliances, connections, scenarios, profile, name, session)?;
    let status = network_status(&simulation);
    let mut output = vec![format!(
        "Server:  {}",
        profile
            .dns_servers
            .first()
            .map_or("not configured", String::as_str)
    )];
    if let Some(address) = &answer {
        output.push(format!("Name:    {name}"));
        output.push(format!("Address: {address}"));
    } else {
        output.push(format!("*** No modeled answer for {name}"));
    }
    output.push(simulation_summary(&simulation));
    Ok(network_report(
        profile,
        WorkstationActionKind::Terminal,
        status,
        format!("DNS lookup: {name}"),
        output,
        None,
        vec![simulation],
    ))
}

pub(super) struct HostResolution {
    pub address: Option<String>,
    pub simulation: Option<ScenarioReport>,
    pub source: &'static str,
}

pub(super) fn resolve_host(
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    scenarios: &ScenarioRepository,
    profile: &WorkstationProfile,
    host: &str,
    session: &mut WorkstationSession,
) -> Result<HostResolution, ConfigError> {
    if host.parse::<Ipv4Addr>().is_ok() {
        return Ok(HostResolution {
            address: Some(host.into()),
            simulation: None,
            source: "literal-address",
        });
    }
    if let Some(address) = session.cached_dns_address(host) {
        return Ok(HostResolution {
            address: Some(address),
            simulation: None,
            source: "client-cache",
        });
    }
    let (simulation, address) =
        run_dns_query(appliances, connections, scenarios, profile, host, session)?;
    if let Some(address) = &address {
        session.remember_dns(host, address);
    }
    Ok(HostResolution {
        address,
        simulation: Some(simulation),
        source: "dns-query",
    })
}

fn execute_ssh(
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    scenarios: &ScenarioRepository,
    profile: &WorkstationProfile,
    target: &str,
    session: &mut WorkstationSession,
) -> Result<WorkstationActionReport, ConfigError> {
    let host = target
        .rsplit('@')
        .next()
        .unwrap_or(target)
        .to_ascii_lowercase();
    let mut simulations = Vec::new();
    let resolution = resolve_host(appliances, connections, scenarios, profile, &host, session)?;
    if let Some(dns) = resolution.simulation {
        simulations.push(dns);
    }
    let Some(destination) = resolution.address else {
        return Ok(network_report(
            profile,
            WorkstationActionKind::Terminal,
            WorkstationActionStatus::Failed,
            format!("SSH: {host}"),
            vec![format!("ssh: Could not resolve hostname {host}")],
            None,
            simulations,
        ));
    };
    let scenario = find_service_scenario(scenarios, &profile.id, "ssh").ok_or_else(|| {
        ConfigError::new(format!(
            "no configured SSH scenario originates from {}",
            profile.id
        ))
    })?;
    let mut packet = scenario.config.packet.clone();
    packet.destination_ip = destination.clone();
    let simulation = session.run_scenario(
        appliances,
        connections,
        scenarios,
        &profile.id,
        &scenario.config,
        Some(packet),
    )?;
    let status = network_status(&simulation);
    let mut output = Vec::new();
    if resolution.source == "client-cache" {
        output.push(format!("DNS cache: {host} -> {destination}"));
    }
    output.extend([
        format!("Connecting to {host} on TCP/22"),
        final_failure(&simulation),
        simulation_summary(&simulation),
    ]);
    simulations.push(simulation);
    Ok(network_report(
        profile,
        WorkstationActionKind::Terminal,
        status,
        format!("SSH: {host}"),
        output,
        None,
        simulations,
    ))
}

pub(super) fn run_dns_query(
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    scenarios: &ScenarioRepository,
    profile: &WorkstationProfile,
    name: &str,
    session: &mut WorkstationSession,
) -> Result<(ScenarioReport, Option<String>), ConfigError> {
    let scenario = find_dns_scenario(scenarios, &profile.id).ok_or_else(|| {
        ConfigError::new(format!(
            "no configured DNS scenario originates from {}",
            profile.id
        ))
    })?;
    if !profile
        .dns_servers
        .iter()
        .any(|server| server == &scenario.config.packet.destination_ip)
    {
        return Err(ConfigError::new(format!(
            "{} does not configure scenario DNS server {}",
            profile.id, scenario.config.packet.destination_ip
        )));
    }
    let mut packet = scenario.config.packet.clone();
    packet.application = ScenarioApplicationConfig::DnsQuery {
        name: name.to_ascii_lowercase(),
    };
    let report = session.run_scenario(
        appliances,
        connections,
        scenarios,
        &profile.id,
        &scenario.config,
        Some(packet),
    )?;
    let answer = report
        .expectation_met
        .then(|| dns_answer(appliances, &scenario.config.participants, name))
        .flatten();
    Ok((report, answer))
}
