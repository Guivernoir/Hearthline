use std::ops::{Deref, DerefMut};

use crate::{BehaviorConfig, ConfigError, ConfigRepository};

use super::{
    ControllerRuntime, HmiControlProgramDocument, HmiOperatorContext, HmiSnapshot,
    PlantControlRuntime, setpoints_from_parameter_groups,
};
use crate::hmi::{HmiAction, HmiActionReport};

#[derive(Debug, Default)]
pub struct PlantRuntimeStore {
    cells: Vec<HmiCellRuntime>,
    operators: Vec<HmiOperatorContext>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct PlantRuntimeOwnership {
    pub plant_runtimes: usize,
    pub operator_contexts: usize,
    pub controller_runtimes: usize,
}

#[derive(Debug)]
struct HmiCellRuntime {
    id: String,
    plant: PlantControlRuntime,
    controllers: Vec<ControllerRuntime>,
    physics_controller: usize,
}

struct BoundPlantControlRuntime<'a> {
    plant: &'a mut PlantControlRuntime,
    operator: &'a mut HmiOperatorContext,
    controller: &'a mut ControllerRuntime,
}

struct BoundController<'a> {
    plant: &'a mut PlantControlRuntime,
    controller: &'a mut ControllerRuntime,
}

impl PlantRuntimeStore {
    pub fn control_program(
        &mut self,
        appliances: &ConfigRepository,
        id: &str,
    ) -> Result<Option<HmiControlProgramDocument>, ConfigError> {
        Ok(self.bind(appliances, id)?.control_program())
    }

    pub fn profile(
        &mut self,
        appliances: &ConfigRepository,
        id: &str,
    ) -> Result<HmiSnapshot, ConfigError> {
        Ok(self.bind(appliances, id)?.snapshot())
    }

    pub fn execute(
        &mut self,
        appliances: &ConfigRepository,
        id: &str,
        action: HmiAction,
    ) -> Result<HmiActionReport, ConfigError> {
        Ok(self.bind(appliances, id)?.execute(action))
    }

    pub fn tick(&mut self, elapsed_ms: u64) {
        for cell in &mut self.cells {
            cell.tick(elapsed_ms);
        }
        if let Some(batch) = self
            .cells
            .iter()
            .find_map(|cell| cell.plant.released_slip())
        {
            for cell in &mut self.cells {
                cell.plant.apply_released_slip(batch);
            }
        }
    }

    pub fn record_telemetry_publication(
        &mut self,
        appliances: &ConfigRepository,
        id: &str,
        delivered: bool,
    ) -> Result<(), ConfigError> {
        let mut session = self.bind(appliances, id)?;
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
        self.operators.clear();
    }

    pub fn ownership(&self) -> PlantRuntimeOwnership {
        PlantRuntimeOwnership {
            plant_runtimes: self.cells.len(),
            operator_contexts: self.operators.len(),
            controller_runtimes: self.cells.iter().map(|cell| cell.controllers.len()).sum(),
        }
    }

    fn bind(
        &mut self,
        appliances: &ConfigRepository,
        id: &str,
    ) -> Result<BoundPlantControlRuntime<'_>, ConfigError> {
        self.ensure(appliances, id)?;
        let operator_index = self
            .operators
            .iter()
            .position(|operator| operator.id == id)
            .expect("initialized HMI operator exists");
        let cell_index = {
            let cell_id = self.operators[operator_index].cell_id();
            self.cells
                .iter()
                .position(|cell| cell.id == cell_id)
                .expect("initialized HMI cell exists")
        };
        let operator = &mut self.operators[operator_index];
        self.cells[cell_index].bind(operator)
    }

    fn ensure(&mut self, appliances: &ConfigRepository, id: &str) -> Result<(), ConfigError> {
        if self.operators.iter().any(|operator| operator.id == id) {
            return Ok(());
        }
        self.initialize_cell(appliances, id)
    }

    fn initialize_cell(
        &mut self,
        appliances: &ConfigRepository,
        id: &str,
    ) -> Result<(), ConfigError> {
        let requested = appliances
            .get(id)
            .ok_or_else(|| ConfigError::new(format!("unknown HMI {id}")))?;
        let BehaviorConfig::OperatorInterface {
            controller: requested_controller,
            ..
        } = &requested.config.behavior
        else {
            return Err(ConfigError::new(format!(
                "appliance {id} is not an operator interface"
            )));
        };
        let environment = requested.config.environment.as_str();
        let cell_id = if environment == "Body Preparation" {
            "body-preparation-plant"
        } else {
            requested_controller
        };
        if self.cells.iter().any(|cell| cell.id == cell_id) {
            return Err(ConfigError::new(format!(
                "HMI {id} was not registered with its existing cell"
            )));
        }

        let mut candidates = Vec::new();
        for candidate in appliances.appliances() {
            let BehaviorConfig::OperatorInterface {
                controller: assigned,
                ..
            } = &candidate.config.behavior
            else {
                continue;
            };
            if (assigned == requested_controller
                || environment == "Body Preparation" && candidate.config.environment == environment)
                && candidate.config.tags.iter().any(|tag| tag == "interactive")
            {
                candidates.push(PlantControlRuntime::from_repository(
                    appliances,
                    &candidate.config.id,
                )?);
            }
        }
        let canonical_index = candidates
            .iter()
            .enumerate()
            .max_by_key(|(_, session)| session.signals.len() + session.actuators.len())
            .map(|(index, _)| index)
            .ok_or_else(|| ConfigError::new(format!("HMI {id} has no interactive cell")))?;
        let mut canonical = candidates.swap_remove(canonical_index);
        let mut controllers = Vec::new();
        let mut operators = Vec::new();
        register_authority(&mut canonical, &mut controllers, &mut operators);
        for mut candidate in candidates {
            register_authority(&mut candidate, &mut controllers, &mut operators);
            canonical.absorb_component_state(candidate);
        }
        if environment == "Body Preparation" {
            let setpoints = setpoints_from_parameter_groups(
                controllers
                    .iter()
                    .map(|controller| controller.parameters.as_slice()),
            );
            canonical.body_preparation = Some(Box::new(
                hearthline_engine::BodyPreparationProcess::new(setpoints),
            ));
        }
        let physics_controller_id = if environment == "Body Preparation" {
            "area-01-vplc-01"
        } else {
            requested_controller.as_str()
        };
        let physics_controller = controllers
            .iter()
            .position(|controller| controller.id == physics_controller_id)
            .ok_or_else(|| {
                ConfigError::new(format!(
                    "cell {cell_id} has no physics controller {physics_controller_id}"
                ))
            })?;
        let mut cell = HmiCellRuntime {
            id: cell_id.into(),
            plant: canonical,
            controllers,
            physics_controller,
        };
        cell.tick(0);
        self.operators.extend(operators);
        self.cells.push(cell);
        Ok(())
    }
}

impl HmiCellRuntime {
    fn bind<'a>(
        &'a mut self,
        operator: &'a mut HmiOperatorContext,
    ) -> Result<BoundPlantControlRuntime<'a>, ConfigError> {
        let controller = self
            .controllers
            .iter_mut()
            .find(|controller| controller.id == operator.controller_id)
            .ok_or_else(|| {
                ConfigError::new(format!(
                    "HMI {} references unavailable controller {}",
                    operator.id, operator.controller_id
                ))
            })?;
        Ok(BoundPlantControlRuntime::new(
            &mut self.plant,
            operator,
            controller,
        ))
    }

    fn tick(&mut self, elapsed_ms: u64) {
        if let Some(controller) = self.controllers.get_mut(self.physics_controller) {
            BoundController::new(&mut self.plant, controller).tick(elapsed_ms);
        }
    }
}

impl<'a> BoundPlantControlRuntime<'a> {
    fn new(
        plant: &'a mut PlantControlRuntime,
        operator: &'a mut HmiOperatorContext,
        controller: &'a mut ControllerRuntime,
    ) -> Self {
        std::mem::swap(&mut plant.context, operator);
        std::mem::swap(&mut plant.controller, controller);
        Self {
            plant,
            operator,
            controller,
        }
    }
}

impl Deref for BoundPlantControlRuntime<'_> {
    type Target = PlantControlRuntime;

    fn deref(&self) -> &Self::Target {
        self.plant
    }
}

impl DerefMut for BoundPlantControlRuntime<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.plant
    }
}

impl Drop for BoundPlantControlRuntime<'_> {
    fn drop(&mut self) {
        std::mem::swap(&mut self.plant.controller, self.controller);
        std::mem::swap(&mut self.plant.context, self.operator);
    }
}

impl<'a> BoundController<'a> {
    fn new(plant: &'a mut PlantControlRuntime, controller: &'a mut ControllerRuntime) -> Self {
        std::mem::swap(&mut plant.controller, controller);
        Self { plant, controller }
    }
}

impl Deref for BoundController<'_> {
    type Target = PlantControlRuntime;

    fn deref(&self) -> &Self::Target {
        self.plant
    }
}

impl DerefMut for BoundController<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.plant
    }
}

impl Drop for BoundController<'_> {
    fn drop(&mut self) {
        std::mem::swap(&mut self.plant.controller, self.controller);
    }
}

fn register_authority(
    session: &mut PlantControlRuntime,
    controllers: &mut Vec<ControllerRuntime>,
    operators: &mut Vec<HmiOperatorContext>,
) {
    let operator = std::mem::replace(&mut session.context, HmiOperatorContext::vacant());
    let controller = std::mem::replace(&mut session.controller, ControllerRuntime::vacant());
    if !controllers
        .iter()
        .any(|candidate| candidate.id == controller.id)
    {
        controllers.push(controller);
    }
    operators.push(operator);
}

impl PlantControlRuntime {
    fn absorb_component_state(&mut self, mut source: Self) {
        if self.body_preparation.is_none() {
            self.body_preparation = source.body_preparation.take();
        }
        if self.robot.is_none() {
            self.robot = source.robot.take();
        }
        if self.guarded_cell.is_none() {
            self.guarded_cell = source.guarded_cell.take();
        }
        if self.supervisory.is_none() {
            self.supervisory = source.supervisory.take();
        }
        for signal in source.signals {
            if !self
                .signals
                .iter()
                .any(|candidate| candidate.tag == signal.tag)
            {
                self.signals.push(signal);
            }
        }
        for actuator in source.actuators {
            if !self
                .actuators
                .iter()
                .any(|candidate| candidate.command_tag == actuator.command_tag)
            {
                self.actuators.push(actuator);
            }
        }
        for safety in source.safety {
            if !self
                .safety
                .iter()
                .any(|candidate| candidate.component_id == safety.component_id)
            {
                self.safety.push(safety);
            }
        }
    }
}
