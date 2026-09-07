use hearthline_model::Text;

use crate::{
    CELL_SNAPSHOT_SCHEMA_VERSION, CellSnapshot, ComponentSnapshot, ConfiguredNetwork,
    ProjectSnapshot, SchedulerMetrics,
};

#[derive(Clone, Debug, Default)]
pub struct ScenarioRuntimeSnapshots {
    pub initial: ScenarioStateSnapshot,
    pub final_state: ScenarioStateSnapshot,
}

#[derive(Clone, Debug, Default)]
pub struct ScenarioStateSnapshot {
    pub captured_at_us: u64,
    pub components: Vec<ComponentSnapshot>,
}

impl ScenarioStateSnapshot {
    pub(crate) fn capture(network: &ConfiguredNetwork, captured_at_us: u64) -> Self {
        Self {
            captured_at_us,
            components: network.component_snapshots(captured_at_us),
        }
    }

    pub(crate) fn add_component(&mut self, component: ComponentSnapshot) {
        self.components.push(component);
        self.components
            .sort_by(|left, right| left.component.cmp(&right.component));
    }

    pub fn project_snapshot(
        &self,
        model_digest: impl Into<String>,
        scenario: &str,
    ) -> Result<ProjectSnapshot, String> {
        let site = Text::try_new("recorded-scenario").map_err(|error| error.to_string())?;
        let cell = Text::try_new(scenario).map_err(|error| error.to_string())?;
        ProjectSnapshot {
            schema_version: CELL_SNAPSHOT_SCHEMA_VERSION.into(),
            model_digest: model_digest.into(),
            captured_at_us: self.captured_at_us,
            cells: vec![CellSnapshot {
                schema_version: CELL_SNAPSHOT_SCHEMA_VERSION.into(),
                site,
                cell,
                captured_at_us: self.captured_at_us,
                components: self.components.clone(),
            }],
            conduits: Vec::new(),
            scheduler: SchedulerMetrics::default(),
        }
        .normalize()
    }
}
