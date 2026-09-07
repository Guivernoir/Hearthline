use hearthline_engine::TraceEntry;

use crate::ConfigError;

use super::super::ScenarioConfig;

pub(super) struct ContinuityTopology {
    pub connection_states: Vec<super::super::ScenarioConnectionState>,
    pub link_aggregation_states: Vec<super::super::ScenarioLinkAggregationState>,
    pub spanning_tree_states: Vec<super::super::ScenarioSpanningTreeState>,
}

pub(super) fn append_phase(
    scenario: &ScenarioConfig,
    trace: &mut Vec<TraceEntry>,
    phase: Result<Vec<TraceEntry>, hearthline_engine::SimulationError>,
) -> Result<(), ConfigError> {
    let phase = phase.map_err(|error| {
        ConfigError::new(format!(
            "scenario {} simulation failed: {error}",
            scenario.id
        ))
    })?;
    if trace.len().saturating_add(phase.len()) > scenario.event_limit {
        return Err(ConfigError::new(format!(
            "scenario {} combined trace exceeds the {} event limit",
            scenario.id, scenario.event_limit
        )));
    }
    trace.extend(phase);
    Ok(())
}
