mod dynamics;
mod types;

pub use types::{
    FormingFault, FormingMeasurements, FormingOutputs, FormingPhase, FormingSetpoints,
    FormingStartError, FormingTick, FormingTrip,
};

use super::body_preparation::{CeramicSlipBatch, DownstreamMaterialEffects};

#[derive(Clone, Debug)]
pub struct FormingProcess {
    phase: FormingPhase,
    phase_elapsed_ms: u64,
    scan_elapsed_ms: u64,
    scan_count: u64,
    cycle_count: u64,
    running: bool,
    fault: Option<FormingFault>,
    measurements: FormingMeasurements,
    outputs: FormingOutputs,
    tank_level_at_cycle_start: f64,
    setpoints: FormingSetpoints,
    material_effects: DownstreamMaterialEffects,
}

impl FormingProcess {
    pub const SCAN_INTERVAL_MS: u64 = 20;

    pub fn new(measurements: FormingMeasurements) -> Self {
        Self {
            phase: FormingPhase::Idle,
            phase_elapsed_ms: 0,
            scan_elapsed_ms: 0,
            scan_count: 0,
            cycle_count: 0,
            running: false,
            fault: None,
            measurements,
            outputs: FormingOutputs::idle(),
            tank_level_at_cycle_start: measurements.slip_tank_level_percent,
            setpoints: FormingSetpoints::default(),
            material_effects: DownstreamMaterialEffects {
                filling_flow_factor: 1.0,
                casting_rate_g_cm2_min: 0.152,
                predicted_green_moisture_percent: measurements.piece_moisture_percent,
                predicted_drying_shrinkage_percent: measurements.predicted_drying_shrinkage_percent,
                drying_energy_factor: measurements.drying_energy_factor,
                green_strength_index: measurements.green_strength_index,
                fired_defect_risk_percent: measurements.fired_defect_risk_percent,
            },
        }
    }

    pub fn with_setpoints(mut self, setpoints: FormingSetpoints) -> Self {
        self.setpoints = setpoints;
        self
    }

    pub fn set_setpoints(&mut self, setpoints: FormingSetpoints) {
        self.setpoints = setpoints;
        self.apply_measurements();
    }

    pub const fn setpoints(&self) -> FormingSetpoints {
        self.setpoints
    }

    pub const fn phase(&self) -> FormingPhase {
        self.phase
    }

    pub const fn phase_elapsed_ms(&self) -> u64 {
        self.phase_elapsed_ms
    }

    pub const fn scan_count(&self) -> u64 {
        self.scan_count
    }

    pub const fn cycle_count(&self) -> u64 {
        self.cycle_count
    }

    pub const fn running(&self) -> bool {
        self.running
    }

    pub const fn fault(&self) -> Option<FormingFault> {
        self.fault
    }

    pub const fn measurements(&self) -> &FormingMeasurements {
        &self.measurements
    }

    pub const fn outputs(&self) -> FormingOutputs {
        self.outputs
    }

    pub const fn material_effects(&self) -> DownstreamMaterialEffects {
        self.material_effects
    }

    pub fn apply_slip_batch(&mut self, batch: CeramicSlipBatch) {
        self.measurements.slip_density_g_cm3 = batch.density_kg_l;
        self.measurements.slip_viscosity_mpa_s = batch.high_shear_viscosity_mpa_s;
        self.measurements.slip_temperature_c = batch.temperature_c;
        self.measurements.piece_moisture_percent = batch.effects.predicted_green_moisture_percent;
        self.measurements.predicted_drying_shrinkage_percent =
            batch.effects.predicted_drying_shrinkage_percent;
        self.measurements.drying_energy_factor = batch.effects.drying_energy_factor;
        self.measurements.green_strength_index = batch.effects.green_strength_index;
        self.measurements.fired_defect_risk_percent = batch.effects.fired_defect_risk_percent;
        self.material_effects = batch.effects;
    }

    pub fn start(&mut self, safety_ready: bool) -> Result<(), FormingStartError> {
        self.prepare_start(safety_ready, FormingPhase::Filling)?;
        self.apply_phase_outputs();
        Ok(())
    }

    pub fn start_controlled(
        &mut self,
        safety_ready: bool,
        phase: FormingPhase,
    ) -> Result<(), FormingStartError> {
        self.prepare_start(safety_ready, phase)?;
        self.apply_phase_outputs();
        Ok(())
    }

    fn prepare_start(
        &mut self,
        safety_ready: bool,
        phase: FormingPhase,
    ) -> Result<(), FormingStartError> {
        if self.running {
            return Err(FormingStartError::AlreadyRunning);
        }
        if !safety_ready || self.phase == FormingPhase::Faulted {
            return Err(FormingStartError::SafetyNotReady);
        }
        if self.fault.is_some() {
            return Err(FormingStartError::FaultActive);
        }
        self.running = true;
        self.phase = phase;
        self.phase_elapsed_ms = 0;
        self.tank_level_at_cycle_start = self.measurements.slip_tank_level_percent;
        self.measurements.piece_gripped = false;
        Ok(())
    }

    pub fn synchronize_control_state(
        &mut self,
        phase: FormingPhase,
        running: bool,
        scan_count: u64,
        cycle_count: u64,
    ) {
        let starting_cycle = !self.running
            && running
            && self.phase == FormingPhase::Idle
            && phase == FormingPhase::Filling;
        if self.phase != phase {
            self.phase = phase;
            self.phase_elapsed_ms = 0;
        }
        if starting_cycle {
            self.tank_level_at_cycle_start = self.measurements.slip_tank_level_percent;
            self.measurements.piece_gripped = false;
        }
        self.running = running;
        self.scan_count = scan_count;
        self.cycle_count = cycle_count;
        if phase == FormingPhase::Idle {
            self.measurements.piece_gripped = false;
        }
        self.apply_phase_outputs();
        self.apply_measurements();
    }

    pub fn pause_controlled(&mut self, phase: FormingPhase, scan_count: u64, cycle_count: u64) {
        self.phase = phase;
        self.phase_elapsed_ms = 0;
        self.scan_count = scan_count;
        self.cycle_count = cycle_count;
        self.running = false;
        self.outputs = FormingOutputs::safe();
        self.measurements.slip_feed_flow_l_min = 0.0;
        self.measurements.water_flow_l_min = 0.0;
        self.measurements.excess_slip_drain_flow_l_min = 0.0;
        self.measurements.vacuum_pressure_kpa = 0.0;
    }

    pub fn set_fault(&mut self, fault: Option<FormingFault>) {
        self.fault = fault;
        if fault.is_none() && !self.running {
            self.measurements.slip_feed_pressure_bar = 2.5;
            self.measurements.compressed_air_pressure_bar = 6.0;
            self.measurements.vacuum_pressure_kpa = 0.0;
        }
    }

    pub fn reset_after_trip(&mut self, safety_ready: bool) -> bool {
        if !safety_ready || self.fault.is_some() || self.phase != FormingPhase::Faulted {
            return false;
        }
        self.phase = FormingPhase::Idle;
        self.phase_elapsed_ms = 0;
        self.running = false;
        self.outputs = FormingOutputs::idle();
        self.measurements.mould_pressure_bar = 0.0;
        self.measurements.water_flow_l_min = 0.0;
        self.measurements.excess_slip_drain_flow_l_min = 0.0;
        self.measurements.vacuum_pressure_kpa = 0.0;
        true
    }

    pub fn tick(&mut self, elapsed_ms: u64) -> FormingTick {
        self.scan_elapsed_ms = self.scan_elapsed_ms.saturating_add(elapsed_ms);
        self.scan_count = self
            .scan_count
            .saturating_add(self.scan_elapsed_ms / Self::SCAN_INTERVAL_MS);
        self.scan_elapsed_ms %= Self::SCAN_INTERVAL_MS;
        if !self.running {
            return FormingTick {
                phase_changed: false,
                trip: None,
            };
        }

        self.phase_elapsed_ms = self.phase_elapsed_ms.saturating_add(elapsed_ms);
        self.apply_measurements();
        if let Some(trip) = self.evaluate_fault() {
            self.trip();
            return FormingTick {
                phase_changed: true,
                trip: Some(trip),
            };
        }

        let mut changed = false;
        while self.running && self.phase_elapsed_ms >= self.setpoints.phase_duration_ms(self.phase)
        {
            self.phase_elapsed_ms -= self.setpoints.phase_duration_ms(self.phase);
            self.phase = self.phase.next();
            changed = true;
            if self.phase == FormingPhase::Idle {
                self.running = false;
                self.cycle_count = self.cycle_count.saturating_add(1);
                self.measurements.piece_gripped = false;
            }
            self.apply_phase_outputs();
            self.apply_measurements();
        }
        FormingTick {
            phase_changed: changed,
            trip: None,
        }
    }

    pub fn tick_controlled(&mut self, elapsed_ms: u64) -> FormingTick {
        if !self.running {
            return FormingTick {
                phase_changed: false,
                trip: None,
            };
        }
        self.phase_elapsed_ms = self.phase_elapsed_ms.saturating_add(elapsed_ms);
        self.apply_measurements();
        if let Some(trip) = self.evaluate_fault() {
            self.trip();
            return FormingTick {
                phase_changed: true,
                trip: Some(trip),
            };
        }
        FormingTick {
            phase_changed: false,
            trip: None,
        }
    }
}
