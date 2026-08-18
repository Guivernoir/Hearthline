mod glaze;
mod quality;
mod return_water;
mod rheology;
mod slip;
mod water;

pub use glaze::{GlazeMeasurements, GlazePhase, GlazeSetpoints};
pub use quality::{
    BodyPreparationFault, BodyPreparationPipelineMeasurements, BodyPreparationStartError,
    BodyPreparationTick, BodyPreparationTrip, CeramicSlipBatch, DownstreamMaterialEffects,
    GlazeBatch, HandoffPipelineMeasurements, PreparationTrain, WaterQuality,
};
pub use slip::{SlipMeasurements, SlipPhase, SlipSetpoints};
pub use water::{
    PUMP_HEARTBEAT_INTERVAL_MS, PUMP_HEARTBEAT_TIMEOUT_MS, PumpMaintenanceState,
    ReturnWaterMeasurements, ReturnWaterPhase, WATER_NETWORK_PUMP_COUNT, WATER_NETWORK_ROUTE_COUNT,
    WaterMeasurements, WaterNetworkMeasurements, WaterPhase, WaterPumpMeasurements,
    WaterRouteMeasurements, WaterSetpoints,
};

pub const SIMULATED_MS_PER_PROCESS_MINUTE: u64 = 50;

pub type BodyPreparationPhase = SlipPhase;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BodyPreparationSetpoints {
    pub slip: SlipSetpoints,
    pub water: WaterSetpoints,
    pub glaze: GlazeSetpoints,
}

impl BodyPreparationSetpoints {
    pub fn dry_mass_kg(self) -> f64 {
        self.slip.dry_mass_kg()
    }

    pub fn total_batch_mass_kg(self) -> f64 {
        self.slip.total_batch_mass_kg()
    }

    pub fn target_solids_percent(self) -> f64 {
        self.slip.target_solids_percent()
    }

    pub fn phase_duration_ms(self, phase: SlipPhase) -> u64 {
        self.slip.phase_duration_ms(phase)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyPreparationMeasurements {
    pub slip: SlipMeasurements,
    pub water: WaterMeasurements,
    pub return_water: ReturnWaterMeasurements,
    pub glaze: GlazeMeasurements,
    pub pipelines: BodyPreparationPipelineMeasurements,
    pub water_networks: WaterNetworkMeasurements,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BodyPreparationOutputs {
    pub slip_water_valve: &'static str,
    pub slip_blunger: &'static str,
    pub slip_screen: &'static str,
    pub slip_transfer_pump: &'static str,
    pub raw_water_pump: &'static str,
    pub media_filter: &'static str,
    pub softener: &'static str,
    pub reverse_osmosis: &'static str,
    pub return_equalization: &'static str,
    pub flocculant_pump: &'static str,
    pub clarifier: &'static str,
    pub filter_press: &'static str,
    pub reuse_diverter: &'static str,
    pub glaze_mill: &'static str,
    pub glaze_screen: &'static str,
    pub glaze_agitator: &'static str,
    pub glaze_transfer_pump: &'static str,
}

impl BodyPreparationOutputs {
    const fn safe() -> Self {
        Self {
            slip_water_valve: "closed",
            slip_blunger: "stopped",
            slip_screen: "stopped",
            slip_transfer_pump: "stopped",
            raw_water_pump: "stopped",
            media_filter: "isolated",
            softener: "service",
            reverse_osmosis: "stopped",
            return_equalization: "stopped",
            flocculant_pump: "stopped",
            clarifier: "stopped",
            filter_press: "stopped",
            reuse_diverter: "hold",
            glaze_mill: "stopped",
            glaze_screen: "stopped",
            glaze_agitator: "stopped",
            glaze_transfer_pump: "stopped",
        }
    }
}

#[derive(Clone, Debug)]
pub struct BodyPreparationProcess {
    slip: slip::SlipRuntime,
    water: water::treatment::WaterRuntime,
    return_water: return_water::ReturnWaterRuntime,
    glaze: glaze::GlazeRuntime,
    scan_elapsed_ms: u64,
    scan_count: u64,
    fault: Option<BodyPreparationFault>,
    setpoints: BodyPreparationSetpoints,
    outputs: BodyPreparationOutputs,
    pipelines: BodyPreparationPipelineMeasurements,
    water_networks: water::distribution::WaterNetworkRuntime,
    released_slip: Option<CeramicSlipBatch>,
    released_glaze: Option<GlazeBatch>,
}

impl Default for BodyPreparationProcess {
    fn default() -> Self {
        Self::new(BodyPreparationSetpoints::default())
    }
}

impl BodyPreparationProcess {
    pub const SCAN_INTERVAL_MS: u64 = 100;

    pub fn new(setpoints: BodyPreparationSetpoints) -> Self {
        Self {
            slip: slip::SlipRuntime::new(),
            water: water::treatment::WaterRuntime::new(),
            return_water: return_water::ReturnWaterRuntime::new(),
            glaze: glaze::GlazeRuntime::new(),
            scan_elapsed_ms: 0,
            scan_count: 0,
            fault: None,
            setpoints,
            outputs: BodyPreparationOutputs::safe(),
            pipelines: BodyPreparationPipelineMeasurements::idle(),
            water_networks: water::distribution::WaterNetworkRuntime::new(),
            released_slip: None,
            released_glaze: None,
        }
    }

    pub const fn phase(&self) -> SlipPhase {
        self.slip.phase
    }
    pub const fn phase_elapsed_ms(&self) -> u64 {
        self.slip.phase_elapsed_ms
    }
    pub const fn scan_count(&self) -> u64 {
        self.scan_count
    }
    pub const fn batch_count(&self) -> u64 {
        self.slip.batch_count
    }
    pub const fn running(&self) -> bool {
        self.slip.running
    }
    pub const fn held(&self) -> bool {
        self.slip.held
    }
    pub const fn fault(&self) -> Option<BodyPreparationFault> {
        self.fault
    }
    pub const fn setpoints(&self) -> BodyPreparationSetpoints {
        self.setpoints
    }
    pub const fn outputs(&self) -> BodyPreparationOutputs {
        self.outputs
    }
    pub const fn released_slip(&self) -> Option<CeramicSlipBatch> {
        self.released_slip
    }
    pub const fn released_glaze(&self) -> Option<GlazeBatch> {
        self.released_glaze
    }

    pub const fn measurements(&self) -> BodyPreparationMeasurements {
        BodyPreparationMeasurements {
            slip: self.slip.measurements,
            water: self.water.measurements,
            return_water: self.return_water.measurements,
            glaze: self.glaze.measurements,
            pipelines: self.pipelines,
            water_networks: self.water_networks.measurements,
        }
    }

    pub fn phase_progress_percent(&self) -> f64 {
        self.slip.progress(&self.setpoints.slip) * 100.0
    }
    pub fn phase_target_process_minutes(&self) -> f64 {
        self.setpoints.slip.phase_duration_ms(self.slip.phase) as f64
            / SIMULATED_MS_PER_PROCESS_MINUTE as f64
    }

    pub fn start(&mut self, safety_ready: bool) -> Result<(), BodyPreparationStartError> {
        self.start_train(PreparationTrain::Slip, safety_ready)
    }

    pub fn start_train(
        &mut self,
        train: PreparationTrain,
        safety_ready: bool,
    ) -> Result<(), BodyPreparationStartError> {
        if !safety_ready {
            return Err(BodyPreparationStartError::SafetyNotReady);
        }
        if self.fault.is_some_and(BodyPreparationFault::prevents_start) {
            return Err(BodyPreparationStartError::FaultActive);
        }
        match train {
            PreparationTrain::Slip => {
                if self.slip.running {
                    return Err(BodyPreparationStartError::AlreadyRunning);
                }
                if self.slip.held {
                    self.slip.start(self.slip.measurements.water)?;
                    self.refresh_outputs();
                    return Ok(());
                }
                let quality = self
                    .water
                    .reserve_slip_water(
                        self.setpoints.slip.water_kg,
                        self.setpoints.water.maximum_body_reuse_percent,
                        &mut self.return_water,
                    )
                    .ok_or(BodyPreparationStartError::WaterUnavailable)?;
                self.slip.start(quality)?;
            }
            PreparationTrain::Water => self.water.start(&self.setpoints.water)?,
            PreparationTrain::ReturnWater => self.return_water.start(&self.setpoints.water)?,
            PreparationTrain::Glaze => {
                if self.glaze.running {
                    return Err(BodyPreparationStartError::AlreadyRunning);
                }
                if self.glaze.held {
                    self.glaze.start(self.glaze.measurements.water)?;
                    self.refresh_outputs();
                    return Ok(());
                }
                let quality = self
                    .water
                    .reserve_glaze_water(
                        self.setpoints.glaze.water_kg,
                        self.setpoints.water.maximum_glaze_reuse_percent,
                        &mut self.return_water,
                    )
                    .ok_or(BodyPreparationStartError::WaterUnavailable)?;
                self.glaze.start(quality)?;
            }
        }
        self.refresh_outputs();
        Ok(())
    }

    pub fn hold(&mut self) -> bool {
        self.hold_train(PreparationTrain::Slip)
    }

    pub fn hold_train(&mut self, train: PreparationTrain) -> bool {
        let held = match train {
            PreparationTrain::Slip => self.slip.hold(),
            PreparationTrain::Water => self.water.hold(),
            PreparationTrain::ReturnWater => self.return_water.hold(),
            PreparationTrain::Glaze => self.glaze.hold(),
        };
        self.refresh_outputs();
        held
    }

    pub fn set_fault(&mut self, fault: Option<BodyPreparationFault>) {
        self.fault = fault;
    }

    pub fn set_setpoints(&mut self, setpoints: BodyPreparationSetpoints) -> bool {
        if self.any_running_or_held() {
            return false;
        }
        self.setpoints = setpoints;
        true
    }

    pub fn reset_after_trip(&mut self, safety_ready: bool) -> bool {
        if !safety_ready || self.fault.is_some() || !self.any_faulted() {
            return false;
        }
        self.slip.reset_fault();
        self.water.reset_fault();
        self.return_water.reset_fault();
        self.glaze.reset_fault();
        self.refresh_outputs();
        true
    }

    pub fn tick(&mut self, elapsed_ms: u64) -> BodyPreparationTick {
        self.scan_elapsed_ms = self.scan_elapsed_ms.saturating_add(elapsed_ms);
        self.scan_count = self
            .scan_count
            .saturating_add(self.scan_elapsed_ms / Self::SCAN_INTERVAL_MS);
        self.scan_elapsed_ms %= Self::SCAN_INTERVAL_MS;

        let mut changed = false;
        let mut trip = None;
        let mut trip_train = None;
        for train in [
            PreparationTrain::Water,
            PreparationTrain::ReturnWater,
            PreparationTrain::Slip,
            PreparationTrain::Glaze,
        ] {
            let result = match train {
                PreparationTrain::Slip => {
                    self.slip.tick(elapsed_ms, &self.setpoints.slip, self.fault)
                }
                PreparationTrain::Water => {
                    self.water
                        .tick(elapsed_ms, &self.setpoints.water, self.fault)
                }
                PreparationTrain::ReturnWater => {
                    self.return_water
                        .tick(elapsed_ms, &self.setpoints.water, self.fault)
                }
                PreparationTrain::Glaze => {
                    self.glaze
                        .tick(elapsed_ms, &self.setpoints.glaze, self.fault)
                }
            };
            changed |= result.0;
            if trip.is_none() && result.1.is_some() {
                trip = result.1;
                trip_train = Some(train);
            }
        }
        self.update_handoff_pipelines();
        self.water_networks.tick(
            elapsed_ms,
            water::distribution::WaterNetworkContext {
                industrial_quality: self.water.measurements.product,
                body_return_quality: self.return_water.measurements.body_reuse_quality,
                glaze_return_quality: self.return_water.measurements.glaze_reuse_quality,
                slip_phase: self.slip.phase,
                glaze_phase: self.glaze.phase,
                return_phase: self.return_water.phase,
                fault: self.fault,
            },
        );
        if self.slip.take_release_pending() {
            let batch = self.slip.release_batch(&self.setpoints.slip);
            self.released_slip = Some(self.apply_slip_handoff(batch));
        }
        if self.glaze.take_release_pending() {
            self.released_glaze = Some(self.glaze.release_batch(&self.setpoints.glaze));
        }
        self.refresh_outputs();
        BodyPreparationTick {
            phase_changed: changed,
            trip,
            train: trip_train,
        }
    }

    fn any_running_or_held(&self) -> bool {
        self.slip.running
            || self.slip.held
            || self.water.running
            || self.water.held
            || self.return_water.running
            || self.return_water.held
            || self.glaze.running
            || self.glaze.held
    }

    fn any_faulted(&self) -> bool {
        self.slip.phase == SlipPhase::Faulted
            || self.water.phase == WaterPhase::Faulted
            || self.return_water.phase == ReturnWaterPhase::Faulted
            || self.glaze.phase == GlazePhase::Faulted
    }

    fn refresh_outputs(&mut self) {
        self.outputs = BodyPreparationOutputs::safe();
        self.slip.apply_outputs(&mut self.outputs);
        self.water.apply_outputs(&mut self.outputs);
        self.return_water.apply_outputs(&mut self.outputs);
        self.glaze.apply_outputs(&mut self.outputs);
    }

    pub fn set_water_pump_failed(&mut self, id: &str, failed: bool) -> bool {
        self.water_networks.set_pump_failed(id, failed)
    }

    pub fn dispatch_water_pump_maintenance(&mut self, id: &str) -> bool {
        self.water_networks.dispatch_maintenance(id)
    }
}
