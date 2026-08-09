use std::collections::BTreeSet;

use hearthline_engine::NeighborEntry;
use hearthline_model::ComponentId;

use crate::scenario::report::ScenarioExecutionEvidence;
use crate::scenario::{
    ScenarioConfig, ScenarioPacketConfig, ScenarioReport, ScenarioRepository, connection,
    firewall_ha, first_hop, link_aggregation, spanning_tree,
};
use crate::{
    ConfigError, ConfigRepository, ConfiguredNetwork, ConnectionRepository, RuntimeDeviceSnapshot,
};

#[derive(Clone, Debug)]
pub struct InteractiveScenarioSession {
    source: String,
    network: ConfiguredNetwork,
    now_us: u64,
}

impl InteractiveScenarioSession {
    pub fn from_source(
        appliances: &ConfigRepository,
        connections: &ConnectionRepository,
        scenarios: &ScenarioRepository,
        source: &str,
    ) -> Result<Self, ConfigError> {
        let participants = scenarios
            .scenarios()
            .filter(|scenario| is_interactive_scenario(&scenario.config, source))
            .flat_map(|scenario| scenario.config.participants.iter().cloned())
            .collect::<BTreeSet<_>>();
        if participants.is_empty() {
            return Err(ConfigError::new(format!(
                "no compatible interactive scenarios originate from {source}"
            )));
        }
        Ok(Self {
            source: source.into(),
            network: ConfiguredNetwork::from_selection(appliances, connections, participants)?,
            now_us: 0,
        })
    }

    pub fn tick(&mut self, elapsed_ms: u64) {
        self.now_us = self.now_us.saturating_add(elapsed_ms.saturating_mul(1_000));
    }

    pub fn now_us(&self) -> u64 {
        self.now_us
    }

    pub fn endpoint_neighbors(&self) -> Result<Vec<NeighborEntry>, ConfigError> {
        self.network.endpoint_neighbors(&self.source, self.now_us)
    }

    pub fn active_pat_translation_count(&self) -> usize {
        self.network.active_pat_translation_count(self.now_us)
    }

    pub fn runtime_devices(&self) -> Vec<RuntimeDeviceSnapshot> {
        self.network.runtime_snapshot(self.now_us)
    }

    pub fn run(
        &mut self,
        appliances: &ConfigRepository,
        connections: &ConnectionRepository,
        scenario: &ScenarioConfig,
        packet_override: Option<ScenarioPacketConfig>,
    ) -> Result<ScenarioReport, ConfigError> {
        if !is_interactive_scenario(scenario, &self.source) {
            return Err(ConfigError::new(format!(
                "scenario {} is not compatible with the {} interactive session",
                scenario.id, self.source
            )));
        }
        scenario.validate()?;
        let packet = packet_override.unwrap_or_else(|| scenario.packet.clone());
        packet.validate()?;
        let connection_states =
            connection::scenario_connection_states(scenario, connections, None)?;
        let first_hop_states = first_hop::scenario_first_hop_states(scenario, appliances, None)?;
        let firewall_ha_states = firewall_ha::scenario_firewall_ha_states(
            scenario,
            appliances,
            connections,
            &connection_states,
            None,
        )?;
        let link_aggregation_states = link_aggregation::scenario_link_aggregation_states(
            scenario,
            appliances,
            connections,
            &connection_states,
        )?;
        let spanning_tree_states = spanning_tree::scenario_spanning_tree_states(
            scenario,
            appliances,
            connections,
            &connection_states,
            &link_aggregation_states,
        )?;
        self.apply_state(
            &connection_states,
            &first_hop_states,
            &firewall_ha_states,
            &link_aggregation_states,
            &spanning_tree_states,
        )?;

        let source = ComponentId::new(&scenario.source)
            .map_err(|error| ConfigError::new(error.to_string()))?;
        let started_at_us = self.now_us;
        let mut trace = self
            .network
            .run_ipv4_at(
                &source,
                packet.ipv4_packet()?,
                packet.wire_length_bytes,
                started_at_us,
                scenario.event_limit,
            )
            .map_err(|error| {
                ConfigError::new(format!(
                    "interactive scenario {} simulation failed: {error}",
                    scenario.id
                ))
            })?;
        self.now_us = trace
            .last()
            .map_or(started_at_us, |entry| entry.time_us)
            .saturating_add(1);
        for entry in &mut trace {
            entry.time_us = entry.time_us.saturating_sub(started_at_us);
        }
        let appliance_count = self.network.appliance_count();
        let link_count = self.network.link_count();
        let active_expectation =
            scenario.active_expectation(&connection_states, &first_hop_states, &firewall_ha_states);
        Ok(ScenarioReport::from_trace(
            scenario,
            ScenarioExecutionEvidence {
                active_expectation,
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

    fn apply_state(
        &mut self,
        connections: &[crate::ScenarioConnectionState],
        first_hops: &[crate::ScenarioFirstHopState],
        firewall_ha: &[crate::ScenarioFirewallHaState],
        aggregates: &[crate::ScenarioLinkAggregationState],
        spanning_tree: &[crate::ScenarioSpanningTreeState],
    ) -> Result<(), ConfigError> {
        for state in connections {
            self.network
                .set_connection_operational(&state.id, state.operational)?;
        }
        for state in first_hops {
            self.network.set_first_hop_active(
                &state.appliance,
                &state.interface,
                state
                    .virtual_ip
                    .parse()
                    .expect("validated first-hop address"),
                state.role.is_active(),
            )?;
        }
        for state in firewall_ha {
            self.network
                .set_firewall_ha_active(&state.appliance, state.role.is_active())?;
        }
        for state in aggregates {
            self.network.set_link_aggregation_forwarding(
                &state.appliance,
                &state.interface,
                state.distributing,
            )?;
            if state.multi_chassis_domain.is_some() {
                self.network.set_multi_chassis_peer_forwarding(
                    &state.appliance,
                    &state.logical_id,
                    state.peer_forwarding,
                )?;
            }
        }
        for state in spanning_tree {
            self.network.set_spanning_tree_forwarding(
                &state.appliance,
                &state.interface,
                state.vlan,
                state.state.is_forwarding(),
            )?;
        }
        Ok(())
    }
}

pub(crate) fn is_interactive_scenario(scenario: &ScenarioConfig, source: &str) -> bool {
    scenario.source == source
        && scenario.connection_overrides.is_empty()
        && scenario.first_hop_overrides.is_empty()
        && scenario.firewall_ha_overrides.is_empty()
        && scenario.recovery.is_none()
        && scenario.continuity.is_none()
        && scenario.ha_isolation.is_none()
        && scenario.local_autonomy.is_none()
}
