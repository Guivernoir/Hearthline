use hearthline_config::{
    ConfigError, ConfigRepository, HmiAction, HmiActionReport, HmiControlProgramDocument,
    HmiSnapshot,
};
use hearthline_sim::PlantRuntimeStore;

/// Stateless operator boundary over authoritative simulation state.
///
/// The gateway owns neither the project catalog nor plant state. It keeps host
/// adapters from binding directly to simulation command and projection APIs.
pub struct PlantOperatorGateway<'a> {
    appliances: &'a ConfigRepository,
    plant: &'a mut PlantRuntimeStore,
}

impl<'a> PlantOperatorGateway<'a> {
    pub const fn new(appliances: &'a ConfigRepository, plant: &'a mut PlantRuntimeStore) -> Self {
        Self { appliances, plant }
    }

    pub fn projection(&mut self, operator_id: &str) -> Result<HmiSnapshot, ConfigError> {
        self.plant.profile(self.appliances, operator_id)
    }

    pub fn submit(
        &mut self,
        operator_id: &str,
        command: HmiAction,
    ) -> Result<HmiActionReport, ConfigError> {
        self.plant.execute(self.appliances, operator_id, command)
    }

    pub fn control_program(
        &mut self,
        operator_id: &str,
    ) -> Result<Option<HmiControlProgramDocument>, ConfigError> {
        self.plant.control_program(self.appliances, operator_id)
    }

    pub fn record_telemetry_publication(
        &mut self,
        operator_id: &str,
        delivered: bool,
    ) -> Result<(), ConfigError> {
        self.plant
            .record_telemetry_publication(self.appliances, operator_id, delivered)
    }
}
