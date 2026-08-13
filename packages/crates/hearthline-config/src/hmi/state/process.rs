use hearthline_engine::{FormingOutputs, FormingPhase, FormingProcess, FormingTrip};

use super::HmiSession;
use crate::hmi::{HmiAlarmSeverity, HmiControlMode, HmiSignal};

impl HmiSession {
    pub(in crate::hmi) fn tick(&mut self, elapsed_ms: u64) {
        if self.moulds.is_empty() {
            if let Some(robot) = &mut self.robot {
                let _ = robot.tick(elapsed_ms);
            }
            return;
        }
        let robot_auto = self.robot_in_auto();
        if robot_auto {
            let requests = self
                .moulds
                .values()
                .filter(|mould| {
                    mould.phase() == FormingPhase::RobotPickup
                        && self
                            .guarded_cell
                            .as_ref()
                            .is_none_or(|cell| cell.ready_for_robot(mould.target()))
                })
                .map(|mould| mould.target().to_string())
                .collect::<Vec<_>>();
            let robot_fault = if let Some(robot) = &mut self.robot {
                for target in requests {
                    robot.request_automatic_handoff(&target);
                }
                robot.tick_automatic(elapsed_ms)
            } else {
                None
            };
            if let Some(fault) = robot_fault {
                self.raise_alarm(
                    fault.code,
                    &fault.mould,
                    &fault.message,
                    HmiAlarmSeverity::Trip,
                );
            }
        } else if let Some(robot) = &mut self.robot {
            let _ = robot.tick(elapsed_ms);
        }
        let completed_handoffs = self
            .moulds
            .keys()
            .filter(|target| {
                self.robot
                    .as_ref()
                    .is_some_and(|robot| robot.delivery_ready(target))
            })
            .cloned()
            .collect::<Vec<_>>();
        for target in completed_handoffs {
            let transfer_started = self
                .guarded_cell
                .as_mut()
                .is_some_and(|cell| cell.begin_delivery(&target));
            if transfer_started && let Some(robot) = &mut self.robot {
                robot.clear_delivery(&target);
            }
        }
        if let Some(cell) = &mut self.guarded_cell {
            cell.tick(elapsed_ms);
        }
        let targets = self.moulds.keys().cloned().collect::<Vec<_>>();
        let mut trips = Vec::new();
        for target in targets {
            let safety_ready = self.mould_safety_ready(&target);
            let phase_before = self
                .moulds
                .get(&target)
                .expect("configured mould exists")
                .phase();
            let robot_pickup_permitted = robot_auto
                && self
                    .robot
                    .as_ref()
                    .is_some_and(|robot| robot.pickup_ready(&target));
            let robot_delivery_permitted = robot_auto
                && self.guarded_cell.as_ref().map_or_else(
                    || {
                        self.robot
                            .as_ref()
                            .is_some_and(|robot| robot.delivery_ready(&target))
                    },
                    |cell| cell.delivery_ready(&target),
                );
            let tick = self
                .moulds
                .get_mut(&target)
                .expect("configured mould exists")
                .tick(
                    elapsed_ms,
                    safety_ready,
                    robot_pickup_permitted,
                    robot_delivery_permitted,
                );
            let phase_after = self
                .moulds
                .get(&target)
                .expect("configured mould exists")
                .phase();
            if phase_before == FormingPhase::RobotDelivery
                && phase_after != FormingPhase::RobotDelivery
            {
                if let Some(cell) = &mut self.guarded_cell {
                    cell.begin_return(&target);
                }
                if let Some(robot) = &mut self.robot {
                    robot.clear_delivery(&target);
                }
            }
            self.sequence = self.sequence.saturating_add(tick.phase_changes);
            if let Some(trip) = tick.trip {
                trips.push((target, trip));
            }
        }
        for (target, trip) in trips {
            self.apply_trip(&target, trip);
        }
        let filling = self
            .moulds
            .values()
            .filter(|mould| mould.running() && mould.phase() == FormingPhase::Filling)
            .count() as f64;
        self.shared_tank_level_percent =
            (self.shared_tank_level_percent - filling * 0.8 * elapsed_ms as f64 / 1_500.0).max(0.0);
        if let Some(program) = self
            .moulds
            .values()
            .find(|mould| mould.running())
            .or_else(|| self.moulds.values().find(|mould| mould.paused()))
            .map(|mould| mould.program().clone())
        {
            self.controller.program = Some(program);
        }
        self.sync_process_snapshot();
        self.sync_robot_signals();
        self.sync_guarded_cell_io();
        if let Some(supervisory) = &mut self.supervisory {
            supervisory.tick(elapsed_ms, &self.signals);
        }
    }

    pub(in crate::hmi) fn mould_safety_ready(&self, target: &str) -> bool {
        let safety_ids = [
            mould_safety_id(target),
            Some("area-02-robot-safe-01"),
            Some("area-02-cell-guard-safe-01"),
        ];
        safety_ids.into_iter().flatten().all(|id| {
            self.safety
                .iter()
                .find(|safety| safety.component_id == id)
                .is_some_and(|safety| {
                    !safety.trip_latched
                        && safety
                            .permissives
                            .iter()
                            .all(|permissive| permissive.satisfied)
                })
        })
    }

    pub(in crate::hmi) fn mould_running(&self, target: &str) -> bool {
        self.moulds.get(target).is_some_and(|mould| mould.running())
    }

    pub(in crate::hmi) fn any_mould_running(&self) -> bool {
        self.moulds.values().any(|mould| mould.running())
    }

    pub(in crate::hmi) fn reset_faulted_moulds(&mut self) -> usize {
        let targets = self.moulds.keys().cloned().collect::<Vec<_>>();
        targets
            .into_iter()
            .filter(|target| {
                let ready = self.mould_safety_ready(target);
                self.moulds
                    .get_mut(target)
                    .is_some_and(|mould| mould.reset_after_trip(ready))
            })
            .count()
    }

    fn sync_process_snapshot(&mut self) {
        self.sync_shared_signals();
        let stations = MOULD_STATIONS.map(|station| {
            let runtime = self
                .moulds
                .get(station.target)
                .expect("configured mould runtime exists");
            (station, *runtime.measurements(), runtime.outputs())
        });
        for (station, measurements, outputs) in stations {
            let timestamp_ms = self
                .moulds
                .get(station.target)
                .expect("configured mould runtime exists")
                .process()
                .scan_count()
                .saturating_mul(FormingProcess::SCAN_INTERVAL_MS);
            let inclination = measurements.mould_position_mm / 600.0 * 90.0;
            for (tag, value) in [
                (station.pressure, measurements.mould_pressure_bar),
                (station.temperature, measurements.mould_temperature_c),
                (station.fill_head, measurements.fill_head_position_mm),
                (station.position, measurements.mould_position_mm),
                (station.moisture, measurements.mould_moisture_percent),
                (station.inclination, inclination),
            ] {
                self.set_signal(tag, value, timestamp_ms);
            }
            if self.station_in_auto(station.target) {
                self.set_actuator(station.movement, outputs.mould);
                self.set_actuator(station.manifold, manifold_state(outputs));
            }
        }
        self.sync_shared_outputs();
    }

    fn sync_shared_signals(&mut self) {
        let timestamp_ms = self
            .moulds
            .values()
            .map(|mould| mould.process().scan_count())
            .max()
            .unwrap_or_default()
            .saturating_mul(FormingProcess::SCAN_INTERVAL_MS);
        let sum = |value: fn(&hearthline_engine::FormingMeasurements) -> f64| {
            self.moulds
                .values()
                .map(|mould| value(mould.measurements()))
                .sum::<f64>()
        };
        let max = |value: fn(&hearthline_engine::FormingMeasurements) -> f64| {
            self.moulds
                .values()
                .map(|mould| value(mould.measurements()))
                .fold(0.0, f64::max)
        };
        let min = |value: fn(&hearthline_engine::FormingMeasurements) -> f64| {
            self.moulds
                .values()
                .map(|mould| value(mould.measurements()))
                .fold(0.0, f64::min)
        };
        let first = self
            .moulds
            .values()
            .next()
            .expect("configured mould runtime")
            .measurements();
        let values = [
            ("area-02-lt-01", self.shared_tank_level_percent),
            ("area-02-dt-01", first.slip_density_g_cm3),
            ("area-02-vis-01", first.slip_viscosity_mpa_s),
            ("area-02-tt-01", first.slip_temperature_c),
            ("area-02-ft-01", sum(|m| m.slip_feed_flow_l_min)),
            ("area-02-pt-01", max(|m| m.slip_feed_pressure_bar)),
            ("area-02-ft-02", sum(|m| m.water_flow_l_min)),
            ("area-02-ft-03", sum(|m| m.excess_slip_drain_flow_l_min)),
            ("area-02-pt-04", max(|m| m.compressed_air_pressure_bar)),
            ("area-02-vt-01", min(|m| m.vacuum_pressure_kpa)),
            ("area-02-pos-03", max(|m| m.robot_position_mm)),
            (
                "area-02-pe-01",
                f64::from(
                    self.moulds
                        .values()
                        .any(|mould| mould.measurements().piece_gripped),
                ),
            ),
        ];
        for (tag, value) in values {
            self.set_signal(tag, value, timestamp_ms);
        }
    }

    fn sync_shared_outputs(&mut self) {
        let outputs = self
            .moulds
            .values()
            .map(|mould| mould.outputs())
            .collect::<Vec<_>>();
        self.set_actuator(
            "area-02-slip-01-command",
            active_state(&outputs, |output| output.slip, "recirculating"),
        );
        self.set_actuator(
            "area-02-water-01-command",
            active_state(&outputs, |output| output.water, "isolated"),
        );
        self.set_actuator(
            "area-02-air-01-command",
            active_state(&outputs, |output| output.air, "isolated"),
        );
        self.set_actuator(
            "area-02-vac-01-command",
            active_state(&outputs, |output| output.vacuum, "stopped"),
        );
        if self.robot_in_auto() {
            let command = self
                .robot
                .as_ref()
                .map(|robot| robot.automatic_command().to_string())
                .unwrap_or_else(|| "home".into());
            self.set_actuator("area-02-robot-01-command", &command);
        }
    }

    fn station_in_auto(&self, target: &str) -> bool {
        self.controller.stations.values().any(|station| {
            station.station_type == "mould-panel"
                && station.target == target
                && station.selected_mode == HmiControlMode::Auto
        })
    }

    fn robot_in_auto(&self) -> bool {
        self.controller.stations.values().any(|station| {
            station.station_type == "robot-joystick"
                && station.target == "robot-01"
                && station.selected_mode == HmiControlMode::Auto
        })
    }

    fn sync_robot_signals(&mut self) {
        let Some(robot) = &self.robot else {
            return;
        };
        let position = robot.normalized_position_mm();
        let snapshot = robot.snapshot();
        let piece_gripped = f64::from(snapshot.gripper_closed);
        let timestamp_ms = self
            .moulds
            .values()
            .map(|mould| mould.process().scan_count())
            .max()
            .unwrap_or_default()
            .saturating_mul(FormingProcess::SCAN_INTERVAL_MS);
        self.set_signal("area-02-pos-03", position, timestamp_ms);
        self.set_signal("area-02-pe-01", piece_gripped, timestamp_ms);
    }

    fn apply_trip(&mut self, target: &str, trip: FormingTrip) {
        if trip == FormingTrip::MouldOverpressure
            && let Some(safety_id) = mould_safety_id(target)
            && let Some(safety) = self
                .safety
                .iter_mut()
                .find(|safety| safety.component_id == safety_id)
        {
            safety.trip_latched = true;
        }
        self.raise_alarm(trip.code(), target, trip.message(), HmiAlarmSeverity::Trip);
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

#[derive(Clone, Copy)]
struct MouldStationTags {
    target: &'static str,
    pressure: &'static str,
    temperature: &'static str,
    fill_head: &'static str,
    position: &'static str,
    moisture: &'static str,
    inclination: &'static str,
    movement: &'static str,
    manifold: &'static str,
}

const MOULD_STATIONS: [MouldStationTags; 4] = [
    MouldStationTags {
        target: "mould-01",
        pressure: "area-02-pt-02",
        temperature: "area-02-tt-02",
        fill_head: "area-02-pos-01",
        position: "area-02-pos-02",
        moisture: "area-02-mt-02",
        inclination: "area-02-m01-inc-01",
        movement: "area-02-mould-01-command",
        manifold: "area-02-m01-manifold-01-command",
    },
    MouldStationTags {
        target: "mould-02",
        pressure: "area-02-m02-pt-01",
        temperature: "area-02-m02-tt-01",
        fill_head: "area-02-m02-pos-01",
        position: "area-02-m02-pos-02",
        moisture: "area-02-m02-mt-01",
        inclination: "area-02-m02-inc-01",
        movement: "area-02-m02-mould-01-command",
        manifold: "area-02-m02-manifold-01-command",
    },
    MouldStationTags {
        target: "mould-03",
        pressure: "area-02-m03-pt-01",
        temperature: "area-02-m03-tt-01",
        fill_head: "area-02-m03-pos-01",
        position: "area-02-m03-pos-02",
        moisture: "area-02-m03-mt-01",
        inclination: "area-02-m03-inc-01",
        movement: "area-02-m03-mould-01-command",
        manifold: "area-02-m03-manifold-01-command",
    },
    MouldStationTags {
        target: "mould-04",
        pressure: "area-02-m04-pt-01",
        temperature: "area-02-m04-tt-01",
        fill_head: "area-02-m04-pos-01",
        position: "area-02-m04-pos-02",
        moisture: "area-02-m04-mt-01",
        inclination: "area-02-m04-inc-01",
        movement: "area-02-m04-mould-01-command",
        manifold: "area-02-m04-manifold-01-command",
    },
];

fn mould_safety_id(target: &str) -> Option<&'static str> {
    match target {
        "mould-01" => Some("area-02-safe-01"),
        "mould-02" => Some("area-02-m02-safe-01"),
        "mould-03" => Some("area-02-m03-safe-01"),
        "mould-04" => Some("area-02-m04-safe-01"),
        _ => None,
    }
}

fn manifold_state(outputs: FormingOutputs) -> &'static str {
    match (outputs.slip, outputs.water, outputs.air, outputs.vacuum) {
        ("filling", _, _, _) => "slip-fill",
        ("draining", _, _, _) => "drain-under-pressure",
        (_, _, "pressurizing", _) => "casting-pressure",
        (_, "release-wet", _, _) => "release-water-both",
        (_, _, "release-assist", _) => "release-air-both",
        (_, "mould-wash", _, _) => "wash-water-both",
        (_, _, "cleaning-purge", _) => "cleaning-air-both",
        (_, _, _, "vacuum-drying") => "vacuum-dry",
        _ => "isolated",
    }
}

fn active_state<'a>(
    outputs: &'a [FormingOutputs],
    value: impl Fn(&'a FormingOutputs) -> &'static str,
    idle: &'static str,
) -> &'static str {
    outputs
        .iter()
        .map(value)
        .find(|state| *state != idle && *state != "stopped")
        .unwrap_or(idle)
}

fn update_signal(signal: &mut HmiSignal, value: f64, timestamp_ms: u64) {
    signal.value = value.clamp(signal.minimum, signal.maximum);
    signal.quality_good = true;
    signal.timestamp_ms = timestamp_ms;
}
