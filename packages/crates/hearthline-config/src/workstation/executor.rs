use std::net::Ipv4Addr;

use crate::scenario::{
    ScenarioApplicationConfig, ScenarioReport, ScenarioRepository, run_scenario,
};
use crate::{ConfigError, ConfigRepository, ConnectionRepository};

use super::curl::parse_curl;
use super::http::{NavigationRequest, navigate};
use super::schema::{
    WORKSTATION_SCHEMA_VERSION, WorkstationAction, WorkstationActionKind, WorkstationActionReport,
    WorkstationActionStatus, WorkstationProfile, workstation_profile,
};
use super::shell::split_command_line;
use super::support::{
    dns_answer, final_failure, find_dns_scenario, find_service_scenario, ipconfig_report,
    local_report, network_report, network_status, simulation_summary, usage_report,
};

pub fn run_workstation_action(
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    scenarios: &ScenarioRepository,
    workstation_id: &str,
    action: WorkstationAction,
) -> Result<WorkstationActionReport, ConfigError> {
    let profile = workstation_profile(appliances, scenarios, workstation_id)?;
    match action {
        WorkstationAction::Terminal { command } => {
            execute_terminal(appliances, connections, scenarios, &profile, command.trim())
        }
        WorkstationAction::Browser { url } => navigate(
            appliances,
            connections,
            scenarios,
            &profile,
            NavigationRequest::browser(&url),
        ),
    }
}

fn execute_terminal(
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    scenarios: &ScenarioRepository,
    profile: &WorkstationProfile,
    command: &str,
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
                "nslookup <name>       Run a configured DNS exchange".into(),
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
        "ipconfig" => Ok(ipconfig_report(profile)),
        "nslookup" => {
            let Some(name) = arguments.first() else {
                return Ok(usage_report(profile, "Usage: nslookup <name>"));
            };
            execute_dns(appliances, connections, scenarios, profile, name)
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
            )
        }
        "ssh" => {
            let Some(target) = arguments.first() else {
                return Ok(usage_report(profile, "Usage: ssh <host>"));
            };
            execute_ssh(appliances, connections, scenarios, profile, target)
        }
        "clear" => Ok(WorkstationActionReport {
            schema_version: WORKSTATION_SCHEMA_VERSION,
            workstation_id: profile.id.clone(),
            action: WorkstationActionKind::Terminal,
            status: WorkstationActionStatus::Completed,
            title: "Terminal cleared".into(),
            output: Vec::new(),
            clear_output: true,
            browser: None,
            simulations: Vec::new(),
        }),
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

fn execute_dns(
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    scenarios: &ScenarioRepository,
    profile: &WorkstationProfile,
    name: &str,
) -> Result<WorkstationActionReport, ConfigError> {
    let (simulation, answer) = run_dns_query(appliances, connections, scenarios, profile, name)?;
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

fn execute_ssh(
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    scenarios: &ScenarioRepository,
    profile: &WorkstationProfile,
    target: &str,
) -> Result<WorkstationActionReport, ConfigError> {
    let host = target
        .rsplit('@')
        .next()
        .unwrap_or(target)
        .to_ascii_lowercase();
    let mut simulations = Vec::new();
    let destination = if host.parse::<Ipv4Addr>().is_ok() {
        host.clone()
    } else {
        let (dns, answer) = run_dns_query(appliances, connections, scenarios, profile, &host)?;
        if !dns.expectation_met {
            let output = vec![format!("ssh: Could not resolve hostname {host}")];
            simulations.push(dns);
            return Ok(network_report(
                profile,
                WorkstationActionKind::Terminal,
                WorkstationActionStatus::Failed,
                format!("SSH: {host}"),
                output,
                None,
                simulations,
            ));
        }
        simulations.push(dns);
        answer.ok_or_else(|| ConfigError::new(format!("DNS returned no address for {host}")))?
    };
    let scenario = find_service_scenario(scenarios, &profile.id, "ssh").ok_or_else(|| {
        ConfigError::new(format!(
            "no configured SSH scenario originates from {}",
            profile.id
        ))
    })?;
    let mut packet = scenario.config.packet.clone();
    packet.destination_ip = destination;
    let simulation = run_scenario(appliances, connections, &scenario.config, Some(packet))?;
    let status = network_status(&simulation);
    let output = vec![
        format!("Connecting to {host} on TCP/22"),
        final_failure(&simulation),
        simulation_summary(&simulation),
    ];
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
    let report = run_scenario(appliances, connections, &scenario.config, Some(packet))?;
    let answer = report
        .expectation_met
        .then(|| dns_answer(appliances, &scenario.config.participants, name))
        .flatten();
    Ok((report, answer))
}
