use hearthline_engine::{FormingProcess, FormingTrip, SequenceInputs};

use super::HmiSession;
use crate::hmi::{HmiAlarmSeverity, HmiSignal};

impl HmiSession {
    pub(in crate::hmi) fn tick(&mut self, elapsed_ms: u64) {
        if self.process.is_none() {
            return;
        }
        if self.controller.program.is_some() {
            self.tick_configured_process(elapsed_ms);
        } else {
            self.tick_builtin_process(elapsed_ms);
        }
        self.sync_process_snapshot();
    }

    fn tick_builtin_process(&mut self, elapsed_ms: u64) {
        let tick = self
            .process
            .as_mut()
            .expect("process exists")
            .tick(elapsed_ms);
        if tick.phase_changed {
            self.sequence = self.sequence.saturating_add(1);
        }
        if let Some(trip) = tick.trip {
            self.apply_trip(trip);
        }
    }

    fn tick_configured_process(&mut self, elapsed_ms: u64) {
        let mut remaining = elapsed_ms;
        while remaining > 0 {
            let slice = remaining.min(
                self.controller
                    .program
                    .as_ref()
                    .expect("configured program exists")
                    .runtime()
                    .time_to_next_scan_ms(),
            );
            let tick = self
                .process
                .as_mut()
                .expect("process exists")
                .tick_controlled(slice);
            remaining -= slice;
            if let Some(trip) = tick.trip {
                self.controller
                    .program
                    .as_mut()
                    .expect("configured program exists")
                    .force_fault();
                self.synchronize_program_state();
                self.apply_trip(trip);
                break;
            }
            let safety_ready = self.safety_ready();
            let running = self
                .controller
                .program
                .as_ref()
                .expect("configured program exists")
                .runtime()
                .running();
            let scan = self
                .controller
                .program
                .as_mut()
                .expect("configured program exists")
                .elapse(
                    slice,
                    SequenceInputs {
                        safety_ready,
                        trip_active: running && !safety_ready,
                        ..SequenceInputs::default()
                    },
                );
            if scan.is_some_and(|scan| scan.step_changed) {
                self.sequence = self.sequence.saturating_add(1);
            }
            self.synchronize_program_state();
        }
        if elapsed_ms == 0 {
            self.synchronize_program_state();
        }
    }

    pub(super) fn synchronize_program_state(&mut self) {
        let Some(program) = self.controller.program.as_ref() else {
            return;
        };
        let phase = program.phase();
        let runtime = program.runtime();
        self.process
            .as_mut()
            .expect("configured program requires process")
            .synchronize_control_state(
                phase,
                runtime.running(),
                runtime.scan_count(),
                runtime.cycle_count(),
            );
    }

    fn sync_process_snapshot(&mut self) {
        let (measurements, timestamp_ms, builtin_outputs) = {
            let process = self.process.as_ref().expect("process exists");
            (
                *process.measurements(),
                process
                    .scan_count()
                    .saturating_mul(FormingProcess::SCAN_INTERVAL_MS),
                process.outputs(),
            )
        };
        let values = [
            ("area-02-lt-01", measurements.slip_tank_level_percent),
            ("area-02-dt-01", measurements.slip_density_g_cm3),
            ("area-02-vis-01", measurements.slip_viscosity_mpa_s),
            ("area-02-tt-01", measurements.slip_temperature_c),
            ("area-02-ft-01", measurements.slip_feed_flow_l_min),
            ("area-02-pt-01", measurements.slip_feed_pressure_bar),
            ("area-02-pt-02", measurements.mould_pressure_bar),
            ("area-02-tt-02", measurements.mould_temperature_c),
            ("area-02-pos-01", measurements.fill_head_position_mm),
            ("area-02-pos-02", measurements.mould_position_mm),
            ("area-02-ft-02", measurements.water_flow_l_min),
            ("area-02-ft-03", measurements.excess_slip_drain_flow_l_min),
            ("area-02-mt-02", measurements.mould_moisture_percent),
            ("area-02-pt-04", measurements.compressed_air_pressure_bar),
            ("area-02-vt-01", measurements.vacuum_pressure_kpa),
            ("area-02-pos-03", measurements.robot_position_mm),
            ("area-02-pe-01", f64::from(measurements.piece_gripped)),
        ];
        for (tag, value) in values {
            self.set_signal(tag, value, timestamp_ms);
        }
        if let Some(program) = &self.controller.program {
            for (tag, state) in program.output_states() {
                self.set_actuator(&tag, &state);
            }
        } else {
            for (tag, state) in [
                ("area-02-slip-01-command", builtin_outputs.slip),
                ("area-02-mould-01-command", builtin_outputs.mould),
                ("area-02-water-01-command", builtin_outputs.water),
                ("area-02-air-01-command", builtin_outputs.air),
                ("area-02-vac-01-command", builtin_outputs.vacuum),
                ("area-02-robot-01-command", builtin_outputs.robot),
            ] {
                self.set_actuator(tag, state);
            }
        }
    }

    fn safety_ready(&self) -> bool {
        self.safety.iter().all(|safety| {
            !safety.trip_latched
                && safety
                    .permissives
                    .iter()
                    .all(|permissive| permissive.satisfied)
        })
    }

    fn apply_trip(&mut self, trip: FormingTrip) {
        if trip == FormingTrip::MouldOverpressure {
            for safety in &mut self.safety {
                safety.trip_latched = true;
            }
        }
        let source = self.controller.id.clone();
        self.raise_alarm(trip.code(), &source, trip.message(), HmiAlarmSeverity::Trip);
    }

    fn set_signal(&mut self, tag: &str, value: f64, timestamp_ms: u64) {
        if let Some(signal) = self.signals.iter_mut().find(|signal| signal.tag == tag) {
            update_signal(signal, value, timestamp_ms);
        }
    }

    fn set_actuator(&mut self, tag: &str, state: &str) {
        if let Some(actuator) = self
            .actuators
            .iter_mut()
            .find(|actuator| actuator.command_tag == tag)
        {
            actuator.current_state = state.into();
        }
    }
}

fn update_signal(signal: &mut HmiSignal, value: f64, timestamp_ms: u64) {
    signal.value = value.clamp(signal.minimum, signal.maximum);
    signal.quality_good = true;
    signal.timestamp_ms = timestamp_ms;
}
