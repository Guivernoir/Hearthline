use std::collections::BTreeMap;

use crate::{BehaviorConfig, ConfigError, ConfigRepository};

use super::{HmiControlProgramDocument, HmiSession, HmiSnapshot};
use crate::hmi::{HmiAction, HmiActionReport};

#[derive(Debug, Default)]
pub struct HmiSessionStore {
    cells: BTreeMap<String, HmiSession>,
    sessions: BTreeMap<String, HmiSession>,
}

impl HmiSessionStore {
    pub fn control_program(
        &mut self,
        appliances: &ConfigRepository,
        id: &str,
    ) -> Result<Option<HmiControlProgramDocument>, ConfigError> {
        let cell = self.ensure(appliances, id)?;
        let shared = self.cells.get(&cell).expect("HMI cell exists").clone();
        let session = self.sessions.get_mut(id).expect("HMI session exists");
        session.sync_shared_from(&shared);
        Ok(session.control_program())
    }

    pub fn profile(
        &mut self,
        appliances: &ConfigRepository,
        id: &str,
    ) -> Result<HmiSnapshot, ConfigError> {
        let cell = self.ensure(appliances, id)?;
        let shared = self.cells.get(&cell).expect("HMI cell exists").clone();
        let session = self.sessions.get_mut(id).expect("HMI session exists");
        session.sync_shared_from(&shared);
        Ok(session.snapshot())
    }

    pub fn execute(
        &mut self,
        appliances: &ConfigRepository,
        id: &str,
        action: HmiAction,
    ) -> Result<HmiActionReport, ConfigError> {
        let cell = self.ensure(appliances, id)?;
        let shared = self.cells.get(&cell).expect("HMI cell exists").clone();
        let session = self.sessions.get_mut(id).expect("HMI session exists");
        session.sync_shared_from(&shared);
        let report = session.execute(action);
        let updated = session.clone();
        self.cells
            .get_mut(&cell)
            .expect("HMI cell exists")
            .merge_shared_from(&updated);
        Ok(report)
    }

    pub fn tick(&mut self, elapsed_ms: u64) {
        for cell in self.cells.values_mut() {
            cell.tick(elapsed_ms);
        }
        if let Some(batch) = self.cells.values().find_map(HmiSession::released_slip) {
            for cell in self.cells.values_mut() {
                cell.apply_released_slip(batch);
            }
        }
    }

    pub fn record_telemetry_publication(
        &mut self,
        appliances: &ConfigRepository,
        id: &str,
        delivered: bool,
    ) -> Result<(), ConfigError> {
        let cell = self.ensure(appliances, id)?;
        let shared = self.cells.get(&cell).expect("HMI cell exists").clone();
        let session = self.sessions.get_mut(id).expect("HMI session exists");
        session.sync_shared_from(&shared);
        session.sequence = session.sequence.saturating_add(1);
        session.record_audit(
            "publish-telemetry",
            "operations-analytics-01",
            if delivered { "delivered" } else { "failed" },
        );
        Ok(())
    }

    pub fn clear(&mut self) {
        self.cells.clear();
        self.sessions.clear();
    }

    fn ensure(&mut self, appliances: &ConfigRepository, id: &str) -> Result<String, ConfigError> {
        if !self.sessions.contains_key(id) {
            self.sessions
                .insert(id.into(), HmiSession::from_repository(appliances, id)?);
        }
        let controller = self
            .sessions
            .get(id)
            .expect("HMI session exists")
            .controller
            .id
            .clone();
        let environment = self
            .sessions
            .get(id)
            .expect("HMI session exists")
            .environment
            .clone();
        let cell = if environment == "Body Preparation" {
            "body-preparation-plant".to_string()
        } else {
            controller.clone()
        };
        if !self.cells.contains_key(&cell) {
            let mut candidates = Vec::new();
            for candidate in appliances.appliances() {
                let BehaviorConfig::OperatorInterface {
                    controller: assigned,
                    ..
                } = &candidate.config.behavior
                else {
                    continue;
                };
                if (assigned == &controller
                    || environment == "Body Preparation"
                        && candidate.config.environment == environment)
                    && candidate.config.tags.iter().any(|tag| tag == "interactive")
                {
                    candidates.push(HmiSession::from_repository(
                        appliances,
                        &candidate.config.id,
                    )?);
                }
            }
            let mut canonical = candidates
                .iter()
                .max_by_key(|session| session.signals.len() + session.actuators.len())
                .cloned()
                .expect("interactive controller has a session");
            for candidate in &candidates {
                canonical.absorb_component_state(candidate);
            }
            if environment == "Body Preparation" {
                let parameters = candidates
                    .iter()
                    .flat_map(|candidate| candidate.controller.parameters.iter().cloned())
                    .collect::<Vec<_>>();
                canonical.body_preparation = Some(hearthline_engine::BodyPreparationProcess::new(
                    super::setpoints_from_parameters(&parameters)?,
                ));
            }
            canonical.tick(0);
            self.cells.insert(cell.clone(), canonical);
        }
        Ok(cell)
    }
}

impl HmiSession {
    fn absorb_component_state(&mut self, source: &Self) {
        if self.body_preparation.is_none() {
            self.body_preparation.clone_from(&source.body_preparation);
        }
        if self.robot.is_none() {
            self.robot.clone_from(&source.robot);
        }
        if self.guarded_cell.is_none() {
            self.guarded_cell.clone_from(&source.guarded_cell);
        }
        if self.supervisory.is_none() {
            self.supervisory.clone_from(&source.supervisory);
        }
        for signal in &source.signals {
            if !self
                .signals
                .iter()
                .any(|candidate| candidate.tag == signal.tag)
            {
                self.signals.push(signal.clone());
            }
        }
        for actuator in &source.actuators {
            if !self
                .actuators
                .iter()
                .any(|candidate| candidate.command_tag == actuator.command_tag)
            {
                self.actuators.push(actuator.clone());
            }
        }
        for safety in &source.safety {
            if !self
                .safety
                .iter()
                .any(|candidate| candidate.component_id == safety.component_id)
            {
                self.safety.push(safety.clone());
            }
        }
    }
}
