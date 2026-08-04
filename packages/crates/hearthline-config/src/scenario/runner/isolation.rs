use hearthline_engine::{FirewallHaControl, SimulationEvent, TraceEntry};
use hearthline_model::ComponentId;

use crate::{ConfigError, ConfiguredNetwork};

use super::super::report::{ScenarioExecutionEvidence, ScenarioHaIsolationEvidence};
use super::super::{ScenarioConfig, ScenarioExpectationMode, ScenarioReport};
use super::{ContinuityTopology, append_phase};

#[allow(clippy::too_many_arguments)]
pub(super) fn run_ha_isolation_scenario(
    scenario: &ScenarioConfig,
    mut network: ConfiguredNetwork,
    appliance_count: usize,
    link_count: usize,
    first_hop_states: Vec<super::super::ScenarioFirstHopState>,
    mut firewall_ha_states: Vec<super::super::ScenarioFirewallHaState>,
    isolated: ContinuityTopology,
) -> Result<ScenarioReport, ConfigError> {
    let isolation = scenario
        .ha_isolation
        .as_ref()
        .expect("HA isolation runner requires isolation config");
    let standby_id = isolation.standby_appliance.clone();
    let standby =
        ComponentId::new(&standby_id).map_err(|error| ConfigError::new(error.to_string()))?;
    let standby_state = firewall_ha_states
        .iter()
        .find(|state| state.appliance == standby_id)
        .expect("repository validation selected the isolated standby");
    let active_id = standby_state.peer.clone();
    let active =
        ComponentId::new(&active_id).map_err(|error| ConfigError::new(error.to_string()))?;
    let sync_connection = standby_state.sync_connection.clone();
    let opening_source =
        ComponentId::new(&scenario.source).map_err(|error| ConfigError::new(error.to_string()))?;
    let continuation_source =
        ComponentId::new(&isolation.source).map_err(|error| ConfigError::new(error.to_string()))?;
    let mut trace = Vec::<TraceEntry>::new();

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
    let active_status = network.firewall_ha_status(&active_id)?;
    let mut heartbeat_at = active_status.heartbeat_interval_us;
    while heartbeat_at < isolation.isolation_at_us {
        append_phase(
            scenario,
            &mut trace,
            network.run_event_at(
                &active,
                SimulationEvent::FirewallHa(FirewallHaControl::HeartbeatTick {
                    at_us: heartbeat_at,
                }),
                heartbeat_at,
                scenario.event_limit,
            ),
        )?;
        heartbeat_at = heartbeat_at.saturating_add(active_status.heartbeat_interval_us);
    }
    network.set_connection_operational(&sync_connection, false)?;
    append_phase(
        scenario,
        &mut trace,
        network.run_event_at(
            &active,
            SimulationEvent::FirewallHa(FirewallHaControl::HeartbeatTick {
                at_us: heartbeat_at,
            }),
            heartbeat_at,
            scenario.event_limit,
        ),
    )?;

    let standby_before = network.firewall_ha_status(&standby_id)?;
    let last_heartbeat_us = standby_before.last_heartbeat_us.ok_or_else(|| {
        ConfigError::new(format!(
            "scenario {} isolated standby did not receive an HA heartbeat",
            scenario.id
        ))
    })?;
    let evaluation_at_us = last_heartbeat_us.saturating_add(standby_before.failure_hold_us);
    append_phase(
        scenario,
        &mut trace,
        network.run_event_at(
            &standby,
            SimulationEvent::FirewallHa(FirewallHaControl::EvaluatePeer {
                at_us: evaluation_at_us,
                peer_failure_confirmed: false,
            }),
            evaluation_at_us,
            scenario.event_limit,
        ),
    )?;
    let inhibited = network.firewall_ha_status(&standby_id)?;
    if inhibited.active || inhibited.promotion_inhibited_at_us != Some(evaluation_at_us) {
        return Err(ConfigError::new(format!(
            "scenario {} isolated standby was not fenced after heartbeat timeout",
            scenario.id
        )));
    }

    for state in &isolated.connection_states {
        network.set_connection_operational(&state.id, state.operational)?;
    }
    append_phase(
        scenario,
        &mut trace,
        network.run_ipv4_at(
            &continuation_source,
            isolation.packet.ipv4_packet()?,
            isolation.packet.wire_length_bytes,
            isolation.continuation_at_us,
            scenario.event_limit,
        ),
    )?;
    let active_after = network.firewall_ha_status(&active_id)?;
    let standby_after = network.firewall_ha_status(&standby_id)?;
    let active_members = usize::from(active_after.active && active_after.operational)
        + usize::from(standby_after.active && standby_after.operational);
    if active_members != 1 {
        return Err(ConfigError::new(format!(
            "scenario {} HA isolation did not preserve single-active ownership",
            scenario.id
        )));
    }
    let sync_operational = isolated
        .connection_states
        .iter()
        .find(|connection| connection.id == sync_connection)
        .is_some_and(|connection| connection.operational);
    for state in &mut firewall_ha_states {
        state.sync_operational = isolated
            .connection_states
            .iter()
            .find(|connection| connection.id == state.sync_connection)
            .is_some_and(|connection| connection.operational);
    }

    Ok(ScenarioReport::from_trace(
        scenario,
        ScenarioExecutionEvidence {
            active_expectation: (ScenarioExpectationMode::Isolation, &isolation.expectation),
            packet: scenario.packet.clone(),
            appliance_count,
            link_count,
            connection_states: isolated.connection_states,
            first_hop_states,
            firewall_ha_states,
            link_aggregation_states: isolated.link_aggregation_states,
            spanning_tree_states: isolated.spanning_tree_states,
            continuity: None,
            ha_isolation: Some(ScenarioHaIsolationEvidence {
                active_appliance: active_id,
                standby_appliance: standby_id,
                isolation_at_us: isolation.isolation_at_us,
                last_heartbeat_us,
                evaluation_at_us,
                promotion_inhibited_at_us: evaluation_at_us,
                active_members,
                standby_sessions: standby_after.session_count,
                sync_operational,
                peer_failure_confirmed: false,
            }),
            local_autonomy: None,
            trace: &trace,
        },
    ))
}
