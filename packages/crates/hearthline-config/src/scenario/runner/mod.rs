use hearthline_engine::{FirewallHaControl, SimulationEvent};
use hearthline_model::ComponentId;

use crate::{
    ConfigError, ConfigRepository, ConfiguredNetwork, ConnectionRepository, FirewallHaRole,
    FirstHopRole,
};

use super::report::{ScenarioContinuityEvidence, ScenarioExecutionEvidence};
use super::{
    ScenarioConfig, ScenarioConnectionOverride, ScenarioContinuityFault,
    ScenarioFirewallHaOverride, ScenarioFirstHopOverride, ScenarioPacketConfig, ScenarioReport,
};

mod isolation;
use isolation::run_ha_isolation_scenario;
mod autonomy;
use autonomy::run_local_autonomy_scenario;
mod interactive;
pub use interactive::InteractiveScenarioSession;
pub(crate) use interactive::is_interactive_scenario;
mod support;
use support::{ContinuityTopology, append_phase};

pub fn run_scenario(
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    scenario: &ScenarioConfig,
    packet_override: Option<ScenarioPacketConfig>,
) -> Result<ScenarioReport, ConfigError> {
    run_scenario_with_overrides(appliances, connections, scenario, packet_override, None)
}

pub fn run_scenario_with_overrides(
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    scenario: &ScenarioConfig,
    packet_override: Option<ScenarioPacketConfig>,
    connection_overrides: Option<Vec<ScenarioConnectionOverride>>,
) -> Result<ScenarioReport, ConfigError> {
    run_scenario_with_state_overrides(
        appliances,
        connections,
        scenario,
        packet_override,
        connection_overrides,
        None,
        None,
    )
}

pub fn run_scenario_with_state_overrides(
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    scenario: &ScenarioConfig,
    packet_override: Option<ScenarioPacketConfig>,
    connection_overrides: Option<Vec<ScenarioConnectionOverride>>,
    first_hop_overrides: Option<Vec<ScenarioFirstHopOverride>>,
    firewall_ha_overrides: Option<Vec<ScenarioFirewallHaOverride>>,
) -> Result<ScenarioReport, ConfigError> {
    scenario.validate()?;
    if (scenario.continuity.is_some()
        || scenario.ha_isolation.is_some()
        || scenario.local_autonomy.is_some())
        && (packet_override.is_some()
            || connection_overrides.is_some()
            || first_hop_overrides.is_some()
            || firewall_ha_overrides.is_some())
    {
        return Err(ConfigError::new(format!(
            "scenario {} controlled contract does not accept runtime overrides",
            scenario.id
        )));
    }
    let packet = packet_override.unwrap_or_else(|| scenario.packet.clone());
    packet.validate()?;
    let connection_states = super::connection::scenario_connection_states(
        scenario,
        connections,
        connection_overrides.as_deref(),
    )?;
    let first_hop_states = super::first_hop::scenario_first_hop_states(
        scenario,
        appliances,
        first_hop_overrides.as_deref(),
    )?;
    let firewall_ha_states = super::firewall_ha::scenario_firewall_ha_states(
        scenario,
        appliances,
        connections,
        &connection_states,
        firewall_ha_overrides.as_deref(),
    )?;
    let link_aggregation_states = super::link_aggregation::scenario_link_aggregation_states(
        scenario,
        appliances,
        connections,
        &connection_states,
    )?;
    let spanning_tree_states = super::spanning_tree::scenario_spanning_tree_states(
        scenario,
        appliances,
        connections,
        &connection_states,
        &link_aggregation_states,
    )?;
    let continuity_topology = if let Some(continuity) = &scenario.continuity {
        let connection_states = super::connection::scenario_connection_states(
            scenario,
            connections,
            Some(&continuity.connection_overrides),
        )?;
        let link_aggregation_states = super::link_aggregation::scenario_link_aggregation_states(
            scenario,
            appliances,
            connections,
            &connection_states,
        )?;
        let spanning_tree_states = super::spanning_tree::scenario_spanning_tree_states(
            scenario,
            appliances,
            connections,
            &connection_states,
            &link_aggregation_states,
        )?;
        Some(ContinuityTopology {
            connection_states,
            link_aggregation_states,
            spanning_tree_states,
        })
    } else {
        None
    };
    let isolation_topology = if let Some(isolation) = &scenario.ha_isolation {
        let connection_states = super::connection::scenario_connection_states(
            scenario,
            connections,
            Some(&isolation.connection_overrides),
        )?;
        let link_aggregation_states = super::link_aggregation::scenario_link_aggregation_states(
            scenario,
            appliances,
            connections,
            &connection_states,
        )?;
        let spanning_tree_states = super::spanning_tree::scenario_spanning_tree_states(
            scenario,
            appliances,
            connections,
            &connection_states,
            &link_aggregation_states,
        )?;
        Some(ContinuityTopology {
            connection_states,
            link_aggregation_states,
            spanning_tree_states,
        })
    } else {
        None
    };
    let mut network =
        ConfiguredNetwork::from_selection(appliances, connections, &scenario.participants)?;
    for connection in &connection_states {
        network.set_connection_operational(&connection.id, connection.operational)?;
    }
    for state in &first_hop_states {
        network.set_first_hop_active(
            &state.appliance,
            &state.interface,
            state
                .virtual_ip
                .parse()
                .expect("validated first-hop address"),
            state.role.is_active(),
        )?;
    }
    for state in &firewall_ha_states {
        network.set_firewall_ha_active(&state.appliance, state.role.is_active())?;
    }
    for state in &link_aggregation_states {
        network.set_link_aggregation_forwarding(
            &state.appliance,
            &state.interface,
            state.distributing,
        )?;
        if state.multi_chassis_domain.is_some() {
            network.set_multi_chassis_peer_forwarding(
                &state.appliance,
                &state.logical_id,
                state.peer_forwarding,
            )?;
        }
    }
    for state in &spanning_tree_states {
        network.set_spanning_tree_forwarding(
            &state.appliance,
            &state.interface,
            state.vlan,
            state.state.is_forwarding(),
        )?;
    }
    let appliance_count = network.appliance_count();
    let link_count = network.link_count();
    if scenario.continuity.is_some() {
        return run_continuity_scenario(
            scenario,
            network,
            appliance_count,
            link_count,
            first_hop_states,
            firewall_ha_states,
            continuity_topology.expect("continuity scenario has a converged topology"),
        );
    }
    if scenario.ha_isolation.is_some() {
        return run_ha_isolation_scenario(
            scenario,
            network,
            appliance_count,
            link_count,
            first_hop_states,
            firewall_ha_states,
            isolation_topology.expect("HA isolation scenario has isolated topology"),
        );
    }
    if scenario.local_autonomy.is_some() {
        return run_local_autonomy_scenario(
            scenario,
            appliances,
            connections,
            network,
            appliance_count,
            link_count,
            connection_states,
            first_hop_states,
            firewall_ha_states,
            link_aggregation_states,
            spanning_tree_states,
        );
    }
    let (expectation_mode, expectation) =
        scenario.active_expectation(&connection_states, &first_hop_states, &firewall_ha_states);
    let source =
        ComponentId::new(&scenario.source).map_err(|error| ConfigError::new(error.to_string()))?;
    let runtime_packet = packet.ipv4_packet()?;
    let trace = network
        .run_ipv4_with_wire_length(
            &source,
            runtime_packet,
            packet.wire_length_bytes,
            scenario.event_limit,
        )
        .map_err(|error| {
            ConfigError::new(format!(
                "scenario {} simulation failed: {error}",
                scenario.id
            ))
        })?;
    Ok(ScenarioReport::from_trace(
        scenario,
        ScenarioExecutionEvidence {
            active_expectation: (expectation_mode, expectation),
            packet,
            appliance_count,
            link_count,
            connection_states,
            first_hop_states,
            firewall_ha_states,
            link_aggregation_states,
            spanning_tree_states,
            continuity: None,
            ha_isolation: None,
            local_autonomy: None,
            trace: &trace,
        },
    ))
}

#[allow(clippy::too_many_arguments)]
fn run_continuity_scenario(
    scenario: &ScenarioConfig,
    mut network: ConfiguredNetwork,
    appliance_count: usize,
    link_count: usize,
    mut first_hop_states: Vec<super::ScenarioFirstHopState>,
    mut firewall_ha_states: Vec<super::ScenarioFirewallHaState>,
    converged: ContinuityTopology,
) -> Result<ScenarioReport, ConfigError> {
    let continuity = scenario
        .continuity
        .as_ref()
        .expect("continuity runner requires continuity config");
    let failed = ComponentId::new(&continuity.failed_appliance)
        .map_err(|error| ConfigError::new(error.to_string()))?;
    let failed_ha_state = firewall_ha_states
        .iter()
        .find(|state| state.appliance == continuity.failed_appliance)
        .expect("repository validation selected the failed firewall HA state");
    let peer_id = failed_ha_state.peer.clone();
    let sync_connection = failed_ha_state.sync_connection.clone();
    let peer = ComponentId::new(&peer_id).map_err(|error| ConfigError::new(error.to_string()))?;
    let opening_source =
        ComponentId::new(&scenario.source).map_err(|error| ConfigError::new(error.to_string()))?;
    let continuation_source = ComponentId::new(&continuity.source)
        .map_err(|error| ConfigError::new(error.to_string()))?;
    let mut trace = Vec::new();

    append_phase(
        scenario,
        &mut trace,
        network.run_ipv4_at(
            &opening_source,
            scenario.packet.ipv4_packet()?,
            scenario.packet.wire_length_bytes,
            0,
            scenario.event_limit,
        ),
    )?;
    let active_status = network.firewall_ha_status(&continuity.failed_appliance)?;
    let mut heartbeat_at = active_status.heartbeat_interval_us;
    let mut fault_index = 0;
    while heartbeat_at < continuity.failure_at_us || fault_index < continuity.faults.len() {
        let next_fault_at = continuity
            .faults
            .get(fault_index)
            .map_or(u64::MAX, |fault| fault.at_us());
        if next_fault_at <= heartbeat_at {
            match continuity.faults[fault_index] {
                ScenarioContinuityFault::SyncLinkLoss { .. } => {
                    network.set_connection_operational(&sync_connection, false)?;
                }
                ScenarioContinuityFault::StandbySessionLoss { at_us } => {
                    append_phase(
                        scenario,
                        &mut trace,
                        network.run_event_at(
                            &peer,
                            SimulationEvent::FirewallHa(
                                FirewallHaControl::ClearReplicatedSessions { at_us },
                            ),
                            at_us,
                            scenario.event_limit,
                        ),
                    )?;
                }
            }
            fault_index += 1;
        } else if heartbeat_at < continuity.failure_at_us {
            append_phase(
                scenario,
                &mut trace,
                network.run_event_at(
                    &failed,
                    SimulationEvent::FirewallHa(FirewallHaControl::HeartbeatTick {
                        at_us: heartbeat_at,
                    }),
                    heartbeat_at,
                    scenario.event_limit,
                ),
            )?;
            heartbeat_at = heartbeat_at.saturating_add(active_status.heartbeat_interval_us);
        }
    }

    for state in &converged.connection_states {
        network.set_connection_operational(&state.id, state.operational)?;
    }
    for state in &converged.link_aggregation_states {
        network.set_link_aggregation_forwarding(
            &state.appliance,
            &state.interface,
            state.distributing,
        )?;
        if state.multi_chassis_domain.is_some() {
            network.set_multi_chassis_peer_forwarding(
                &state.appliance,
                &state.logical_id,
                state.peer_forwarding,
            )?;
        }
    }
    for state in &converged.spanning_tree_states {
        network.set_spanning_tree_forwarding(
            &state.appliance,
            &state.interface,
            state.vlan,
            state.state.is_forwarding(),
        )?;
    }

    append_phase(
        scenario,
        &mut trace,
        network.run_event_at(
            &failed,
            SimulationEvent::SetOperational(false),
            continuity.failure_at_us,
            scenario.event_limit,
        ),
    )?;
    let standby_before = network.firewall_ha_status(&peer_id)?;
    let last_heartbeat_us = standby_before.last_heartbeat_us.ok_or_else(|| {
        ConfigError::new(format!(
            "scenario {} standby did not receive a firewall HA heartbeat",
            scenario.id
        ))
    })?;
    let promotion_at_us = last_heartbeat_us.saturating_add(standby_before.failure_hold_us);
    append_phase(
        scenario,
        &mut trace,
        network.run_event_at(
            &peer,
            SimulationEvent::FirewallHa(FirewallHaControl::EvaluatePeer {
                at_us: promotion_at_us,
                peer_failure_confirmed: true,
            }),
            promotion_at_us,
            scenario.event_limit,
        ),
    )?;
    let promoted = network.firewall_ha_status(&peer_id)?;
    if !promoted.active || promoted.promoted_at_us != Some(promotion_at_us) {
        return Err(ConfigError::new(format!(
            "scenario {} standby firewall did not promote after its heartbeat hold timer",
            scenario.id
        )));
    }
    append_phase(
        scenario,
        &mut trace,
        network.run_ipv4_at(
            &continuation_source,
            continuity.packet.ipv4_packet()?,
            continuity.packet.wire_length_bytes,
            continuity.continuation_at_us,
            scenario.event_limit,
        ),
    )?;
    let post_continuation = network.firewall_ha_status(&peer_id)?;

    for state in &mut firewall_ha_states {
        state.sync_operational = converged
            .connection_states
            .iter()
            .find(|connection| connection.id == state.sync_connection)
            .is_some_and(|connection| connection.operational);
        if state.appliance == continuity.failed_appliance {
            state.role = FirewallHaRole::Standby;
        } else if state.appliance == peer_id {
            state.role = FirewallHaRole::Active;
        }
    }
    for state in &mut first_hop_states {
        if state.appliance == continuity.failed_appliance {
            state.role = FirstHopRole::Standby;
        } else if state.appliance == peer_id {
            state.role = FirstHopRole::Active;
        }
    }
    let sync_operational_at_failure = firewall_ha_states
        .iter()
        .find(|state| state.appliance == peer_id)
        .is_some_and(|state| state.sync_operational);

    Ok(ScenarioReport::from_trace(
        scenario,
        ScenarioExecutionEvidence {
            active_expectation: (
                super::ScenarioExpectationMode::Continuity,
                &continuity.expectation,
            ),
            packet: scenario.packet.clone(),
            appliance_count,
            link_count,
            connection_states: converged.connection_states,
            first_hop_states,
            firewall_ha_states,
            link_aggregation_states: converged.link_aggregation_states,
            spanning_tree_states: converged.spanning_tree_states,
            continuity: Some(ScenarioContinuityEvidence {
                failed_appliance: continuity.failed_appliance.clone(),
                promoted_appliance: peer_id,
                failure_at_us: continuity.failure_at_us,
                last_heartbeat_us,
                promotion_at_us,
                synchronized_sessions: promoted.session_count,
                sessions_after_continuation: post_continuation.session_count,
                replicated_updates: promoted.replicated_updates,
                sync_operational_at_failure,
                faults: continuity.faults.clone(),
            }),
            ha_isolation: None,
            local_autonomy: None,
            trace: &trace,
        },
    ))
}
