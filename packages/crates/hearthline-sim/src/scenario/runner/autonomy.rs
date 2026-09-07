use std::collections::BTreeMap;

use hearthline_model::ComponentId;

use crate::{
    CELL_SNAPSHOT_SCHEMA_VERSION, ComponentSnapshot, ConfigError, ConfigRepository,
    ConfiguredNetwork, ConnectionRepository, HmiAction, HmiActionStatus, HmiSnapshot,
    PlantControlRuntime, SnapshotValue,
};

use super::super::report::{ScenarioExecutionEvidence, ScenarioLocalAutonomyEvidence};
use super::super::{
    ScenarioConfig, ScenarioConnectionState, ScenarioExpectationMode, ScenarioFirewallHaState,
    ScenarioFirstHopState, ScenarioLinkAggregationState, ScenarioReport, ScenarioRuntimeSnapshots,
    ScenarioSpanningTreeState, ScenarioStateSnapshot,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn run_local_autonomy_scenario(
    scenario: &ScenarioConfig,
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    mut network: ConfiguredNetwork,
    appliance_count: usize,
    link_count: usize,
    connection_states: Vec<ScenarioConnectionState>,
    first_hop_states: Vec<ScenarioFirstHopState>,
    firewall_ha_states: Vec<ScenarioFirewallHaState>,
    link_aggregation_states: Vec<ScenarioLinkAggregationState>,
    spanning_tree_states: Vec<ScenarioSpanningTreeState>,
    mut initial_runtime: ScenarioStateSnapshot,
) -> Result<ScenarioReport, ConfigError> {
    let autonomy = scenario
        .local_autonomy
        .as_ref()
        .expect("local-autonomy runner requires its contract");
    let source =
        ComponentId::new(&scenario.source).map_err(|error| ConfigError::new(error.to_string()))?;
    let trace = network
        .run_ipv4_with_wire_length(
            &source,
            scenario.packet.ipv4_packet()?,
            scenario.packet.wire_length_bytes,
            scenario.event_limit,
        )
        .map_err(|error| {
            ConfigError::new(format!(
                "scenario {} simulation failed: {error}",
                scenario.id
            ))
        })?;
    let topology = super::super::local_autonomy::local_control_topology(
        scenario,
        appliances,
        connections,
        &connection_states,
    )?;

    let mut session = PlantControlRuntime::from_repository(appliances, &autonomy.hmi)?;
    initial_runtime.add_component(plant_runtime_snapshot(&session.snapshot()));
    let reset = session.execute(HmiAction::ResetSafety {
        safety_id: autonomy.safety_interface.clone(),
    });
    let safety_reset_applied = matches!(reset.status, HmiActionStatus::Applied);
    let command = session.execute(HmiAction::Command {
        tag: autonomy.command_tag.clone(),
        value: autonomy.command_value.clone(),
    });
    let command_applied = matches!(command.status, HmiActionStatus::Applied);
    let actuator_state = command
        .snapshot
        .actuators
        .iter()
        .find(|state| state.component_id == autonomy.actuator)
        .expect("repository validation resolved the local-autonomy actuator")
        .current_state
        .clone();
    let mut control_trace = reset.trace;
    control_trace.extend(command.trace);
    for (sequence, entry) in control_trace.iter_mut().enumerate() {
        entry.sequence = sequence;
    }
    let autonomy_expectation_met = safety_reset_applied
        && command_applied
        && actuator_state == autonomy.expected_actuator_state;
    let mut final_runtime =
        ScenarioStateSnapshot::capture(&network, trace.last().map_or(0, |entry| entry.time_us));
    final_runtime.add_component(plant_runtime_snapshot(&command.snapshot));

    Ok(ScenarioReport::from_trace(
        scenario,
        ScenarioExecutionEvidence {
            active_expectation: (ScenarioExpectationMode::Autonomy, &scenario.expectation),
            packet: scenario.packet.clone(),
            appliance_count,
            link_count,
            connection_states,
            first_hop_states,
            firewall_ha_states,
            link_aggregation_states,
            spanning_tree_states,
            continuity: None,
            ha_isolation: None,
            local_autonomy: Some(ScenarioLocalAutonomyEvidence {
                hmi: autonomy.hmi.clone(),
                controller: topology.controller,
                remote_io: topology.remote_io,
                safety_interface: autonomy.safety_interface.clone(),
                actuator: autonomy.actuator.clone(),
                command_tag: autonomy.command_tag.clone(),
                command_value: autonomy.command_value.clone(),
                expected_actuator_state: autonomy.expected_actuator_state.clone(),
                actuator_state,
                outage_connections: scenario
                    .connection_overrides
                    .iter()
                    .filter(|state| !state.operational)
                    .map(|state| state.connection.clone())
                    .collect(),
                local_path_connections: topology.connections,
                local_path_operational: true,
                safety_reset_applied,
                command_applied,
                autonomy_expectation_met,
                control_trace,
            }),
            trace: &trace,
            runtime: ScenarioRuntimeSnapshots {
                initial: initial_runtime,
                final_state: final_runtime,
            },
        },
    ))
}

fn plant_runtime_snapshot(snapshot: &HmiSnapshot) -> ComponentSnapshot {
    ComponentSnapshot {
        schema_version: CELL_SNAPSHOT_SCHEMA_VERSION.into(),
        component: format!("plant-runtime-{}", snapshot.id),
        component_kind: "plant-control-runtime".into(),
        state: BTreeMap::from([
            (
                "runtime-state".into(),
                SnapshotValue::Text(
                    serde_json::to_string(snapshot)
                        .expect("plant-control projections are serializable"),
                ),
            ),
            (
                "sequence".into(),
                SnapshotValue::Integer(snapshot.sequence as i64),
            ),
        ]),
    }
}
