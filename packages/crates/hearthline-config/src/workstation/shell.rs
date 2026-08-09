use crate::RuntimeDeviceSnapshot;

use super::schema::{
    WorkstationActionKind, WorkstationActionReport, WorkstationActionStatus, WorkstationProfile,
    WorkstationSession,
};
use super::support::network_report;

const MAX_ARGUMENTS: usize = 32;

pub(super) fn split_command_line(command: &str) -> Result<Vec<String>, String> {
    let mut arguments = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut started = false;

    for character in command.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            started = true;
            continue;
        }
        match (quote, character) {
            (Some('\''), '\'') | (Some('"'), '"') => quote = None,
            (Some('\''), value) => {
                current.push(value);
                started = true;
            }
            (Some('"'), '\\') | (None, '\\') => {
                escaped = true;
                started = true;
            }
            (Some('"'), value) => {
                current.push(value);
                started = true;
            }
            (None, '\'' | '"') => {
                quote = Some(character);
                started = true;
            }
            (None, value) if value.is_whitespace() => {
                if started {
                    push_argument(&mut arguments, core::mem::take(&mut current))?;
                    started = false;
                }
            }
            (None, value) => {
                current.push(value);
                started = true;
            }
            _ => unreachable!("supported quote states are exhaustive"),
        }
    }

    if escaped {
        return Err("terminal: trailing escape is incomplete".into());
    }
    if quote.is_some() {
        return Err("terminal: quoted argument is not terminated".into());
    }
    if started {
        push_argument(&mut arguments, current)?;
    }
    Ok(arguments)
}

fn push_argument(arguments: &mut Vec<String>, argument: String) -> Result<(), String> {
    if arguments.len() == MAX_ARGUMENTS {
        return Err(format!(
            "terminal: command exceeds {MAX_ARGUMENTS} arguments"
        ));
    }
    arguments.push(argument);
    Ok(())
}

pub(super) fn runtime_inspection_report(
    profile: &WorkstationProfile,
    session: &WorkstationSession,
    appliance_id: &str,
    command: &str,
) -> Result<WorkstationActionReport, crate::ConfigError> {
    let state = session.network_state()?;
    if !state.active {
        return Ok(runtime_report(
            profile,
            WorkstationActionStatus::Completed,
            "Runtime inactive",
            vec!["No network state has been learned in this workstation session.".into()],
        ));
    }
    let Some(device) = state
        .devices
        .iter()
        .find(|device| device.id == appliance_id)
    else {
        return Ok(runtime_report(
            profile,
            WorkstationActionStatus::Unsupported,
            "Unknown runtime appliance",
            vec![format!(
                "Appliance {appliance_id} is not part of this workstation runtime."
            )],
        ));
    };
    if command.len() > 128 {
        return Ok(runtime_report(
            profile,
            WorkstationActionStatus::Unsupported,
            "Command rejected",
            vec!["Runtime command exceeds 128 bytes.".into()],
        ));
    }
    let command = command
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let (title, output) = match command.as_str() {
        "show status" => (
            format!("{} runtime status", device.id),
            vec![
                format!("Appliance . . . . . : {}", device.id),
                format!("Kind  . . . . . . . : {}", device.kind),
                format!("CAM entries . . . . : {}", device.mac_table.len()),
                format!("Neighbor entries  . : {}", device.neighbors.len()),
                format!("PAT translations  . : {}", device.pat_translations.len()),
                format!("Firewall sessions . : {}", device.firewall_sessions.len()),
                format!("Simulation time  . . : {} ms", state.simulated_time_ms),
            ],
        ),
        "show mac address-table" if device.supports_mac_table => (
            format!("{} MAC address table", device.id),
            mac_table_output(device),
        ),
        "show arp" if device.supports_neighbors => (
            format!("{} neighbor table", device.id),
            neighbor_output(device),
        ),
        "show ip nat translations" if device.supports_pat => (
            format!("{} PAT translations", device.id),
            pat_output(device),
        ),
        "show sessions" if device.supports_firewall_sessions => (
            format!("{} stateful sessions", device.id),
            firewall_session_output(device),
        ),
        _ => {
            return Ok(runtime_report(
                profile,
                WorkstationActionStatus::Unsupported,
                "Unsupported runtime command",
                vec![format!(
                    "'{command}' is not available for {} ({})",
                    device.id, device.kind
                )],
            ));
        }
    };
    Ok(runtime_report(
        profile,
        WorkstationActionStatus::Completed,
        &title,
        output,
    ))
}

fn mac_table_output(device: &RuntimeDeviceSnapshot) -> Vec<String> {
    let mut output = vec!["VLAN   MAC address         Interface       Remaining".into()];
    for entry in &device.mac_table {
        output.push(format!(
            "{:<6} {:<19} {:<15} {} ms",
            entry.vlan, entry.mac_address, entry.interface, entry.remaining_ttl_ms
        ));
    }
    append_empty(&mut output, device.mac_table.is_empty());
    output
}

fn neighbor_output(device: &RuntimeDeviceSnapshot) -> Vec<String> {
    let mut output = vec!["Address          MAC address         Interface       State".into()];
    for entry in &device.neighbors {
        output.push(format!(
            "{:<16} {:<19} {:<15} {}",
            entry.address, entry.mac_address, entry.interface, entry.state
        ));
    }
    append_empty(&mut output, device.neighbors.is_empty());
    output
}

fn pat_output(device: &RuntimeDeviceSnapshot) -> Vec<String> {
    let mut output = vec!["Proto  Inside local         Inside global        Remote".into()];
    for entry in &device.pat_translations {
        output.push(format!(
            "{:<6} {}:{:<5}  {}:{:<5}  {}:{}",
            entry.protocol,
            entry.internal_address,
            entry.internal_token,
            entry.external_address,
            entry.external_token,
            entry.remote_address,
            entry
                .remote_port
                .map_or_else(|| "-".into(), |port| port.to_string())
        ));
    }
    append_empty(&mut output, device.pat_translations.is_empty());
    output
}

fn firewall_session_output(device: &RuntimeDeviceSnapshot) -> Vec<String> {
    let mut output = vec!["Proto  Source                    Destination".into()];
    for entry in &device.firewall_sessions {
        output.push(format!(
            "{:<6} {}:{:<5}  {}:{}",
            entry.protocol,
            entry.source_address,
            display_port(entry.source_port),
            entry.destination_address,
            display_port(entry.destination_port)
        ));
    }
    append_empty(&mut output, device.firewall_sessions.is_empty());
    output
}

fn display_port(port: Option<u16>) -> String {
    port.map_or_else(|| "-".into(), |value| value.to_string())
}

fn append_empty(output: &mut Vec<String>, empty: bool) {
    if empty {
        output.push("No active entries.".into());
    }
}

fn runtime_report(
    profile: &WorkstationProfile,
    status: WorkstationActionStatus,
    title: &str,
    output: Vec<String>,
) -> WorkstationActionReport {
    network_report(
        profile,
        WorkstationActionKind::Inspect,
        status,
        title.into(),
        output,
        None,
        Vec::new(),
    )
}
