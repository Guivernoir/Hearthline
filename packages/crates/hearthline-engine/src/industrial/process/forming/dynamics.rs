use super::{FormingFault, FormingOutputs, FormingPhase, FormingProcess, FormingTrip};

impl FormingProcess {
    pub(super) fn apply_phase_outputs(&mut self) {
        self.outputs = match self.phase {
            FormingPhase::Idle => FormingOutputs::idle(),
            FormingPhase::Filling => FormingOutputs {
                slip: "filling",
                mould: "closed",
                ..FormingOutputs::safe()
            },
            FormingPhase::Pressurizing | FormingPhase::PressureDwell => FormingOutputs {
                mould: "closed",
                air: "pressurizing",
                ..FormingOutputs::safe()
            },
            FormingPhase::Depressurizing => FormingOutputs {
                mould: "closed",
                ..FormingOutputs::safe()
            },
            FormingPhase::Draining => FormingOutputs {
                slip: "draining",
                mould: "closed",
                air: "pressurizing",
                ..FormingOutputs::safe()
            },
            FormingPhase::ReleaseWater => FormingOutputs {
                mould: "closed",
                water: "release-wet",
                ..FormingOutputs::safe()
            },
            FormingPhase::ReleaseAir => FormingOutputs {
                mould: "closed",
                air: "release-assist",
                ..FormingOutputs::safe()
            },
            FormingPhase::OpeningMould => FormingOutputs {
                mould: "opening",
                ..FormingOutputs::safe()
            },
            FormingPhase::RobotPickup => FormingOutputs {
                mould: "open",
                robot: "gripping",
                ..FormingOutputs::safe()
            },
            FormingPhase::RobotDelivery => FormingOutputs {
                mould: "open",
                robot: "delivering",
                ..FormingOutputs::safe()
            },
            FormingPhase::MouldWash => FormingOutputs {
                mould: "open",
                water: "mould-wash",
                robot: "returning",
                ..FormingOutputs::safe()
            },
            FormingPhase::AirPurge => FormingOutputs {
                mould: "open",
                air: "cleaning-purge",
                robot: "home",
                ..FormingOutputs::safe()
            },
            FormingPhase::VacuumDry => FormingOutputs {
                mould: "open",
                vacuum: "vacuum-drying",
                robot: "home",
                ..FormingOutputs::safe()
            },
            FormingPhase::ClosingMould => FormingOutputs {
                mould: "closing",
                robot: "home",
                ..FormingOutputs::safe()
            },
            FormingPhase::Faulted => FormingOutputs::safe(),
        };
    }

    pub(super) fn apply_measurements(&mut self) {
        let progress = self.progress();
        self.measurements.slip_feed_flow_l_min = 0.0;
        self.measurements.water_flow_l_min = 0.0;
        self.measurements.excess_slip_drain_flow_l_min = 0.0;
        match self.phase {
            FormingPhase::Filling => {
                self.measurements.slip_tank_level_percent =
                    self.tank_level_at_cycle_start - 0.8 * progress;
                self.measurements.slip_feed_flow_l_min = 85.0;
                self.measurements.fill_head_position_mm = 800.0 * progress;
            }
            FormingPhase::Pressurizing => {
                self.measurements.fill_head_position_mm = 800.0 * (1.0 - progress);
                self.measurements.mould_pressure_bar = 6.0 * progress;
            }
            FormingPhase::PressureDwell => self.measurements.mould_pressure_bar = 6.0,
            FormingPhase::Draining => {
                self.measurements.mould_pressure_bar = 6.0;
                self.measurements.excess_slip_drain_flow_l_min = 70.0 * (1.0 - progress);
            }
            FormingPhase::Depressurizing => {
                self.measurements.mould_pressure_bar = 6.0 * (1.0 - progress);
            }
            FormingPhase::ReleaseWater => {
                self.measurements.mould_pressure_bar = 0.0;
                self.measurements.water_flow_l_min = 10.0;
                self.measurements.mould_moisture_percent = 8.0 + 6.0 * progress;
            }
            FormingPhase::ReleaseAir => {
                self.measurements.mould_pressure_bar = 1.0 * (1.0 - progress);
            }
            FormingPhase::OpeningMould => {
                self.measurements.mould_position_mm = 600.0 * progress;
            }
            FormingPhase::RobotPickup => {
                self.measurements.mould_position_mm = 600.0;
                self.measurements.robot_position_mm = 1_200.0 * progress;
                self.measurements.piece_gripped = progress >= 0.8;
            }
            FormingPhase::RobotDelivery => {
                self.measurements.robot_position_mm = 1_200.0 + 1_800.0 * progress;
                self.measurements.piece_gripped = progress < 0.95;
            }
            FormingPhase::MouldWash => {
                self.measurements.water_flow_l_min = 18.0;
                self.measurements.mould_moisture_percent = 14.0 + 16.0 * progress;
                self.measurements.robot_position_mm = 3_000.0 * (1.0 - progress);
            }
            FormingPhase::AirPurge => {
                self.measurements.mould_moisture_percent = 30.0 - 15.0 * progress;
                self.measurements.robot_position_mm = 0.0;
            }
            FormingPhase::VacuumDry => {
                self.measurements.vacuum_pressure_kpa = -80.0 * progress;
                self.measurements.mould_moisture_percent = 15.0 - 12.0 * progress;
            }
            FormingPhase::ClosingMould => {
                self.measurements.vacuum_pressure_kpa = -80.0 * (1.0 - progress);
                self.measurements.mould_position_mm = 600.0 * (1.0 - progress);
            }
            FormingPhase::Idle | FormingPhase::Faulted => {}
        }
    }

    pub(super) fn evaluate_fault(&mut self) -> Option<FormingTrip> {
        match self.fault {
            Some(FormingFault::SlipSupplyLoss) if self.phase == FormingPhase::Filling => {
                self.measurements.slip_feed_pressure_bar = 0.0;
                self.measurements.slip_feed_flow_l_min = 0.0;
                (self.phase_elapsed_ms >= 500).then_some(FormingTrip::SlipFlowNotEstablished)
            }
            Some(FormingFault::CompressedAirLoss)
                if matches!(
                    self.phase,
                    FormingPhase::Pressurizing
                        | FormingPhase::PressureDwell
                        | FormingPhase::ReleaseAir
                        | FormingPhase::AirPurge
                ) =>
            {
                self.measurements.compressed_air_pressure_bar = 1.0;
                self.measurements.mould_pressure_bar = 0.5;
                Some(FormingTrip::CompressedAirLow)
            }
            Some(FormingFault::MouldOverpressure)
                if matches!(
                    self.phase,
                    FormingPhase::Pressurizing | FormingPhase::PressureDwell
                ) =>
            {
                self.measurements.mould_pressure_bar = 9.5;
                Some(FormingTrip::MouldOverpressure)
            }
            Some(FormingFault::VacuumLoss) if self.phase == FormingPhase::VacuumDry => {
                self.measurements.vacuum_pressure_kpa = -10.0;
                (self.phase_elapsed_ms >= 800).then_some(FormingTrip::VacuumNotEstablished)
            }
            Some(FormingFault::RobotPickupFailure) if self.phase == FormingPhase::RobotPickup => {
                self.measurements.piece_gripped = false;
                (self.phase_elapsed_ms >= 900).then_some(FormingTrip::RobotPickupFailed)
            }
            _ => None,
        }
    }

    pub(super) fn trip(&mut self) {
        self.running = false;
        self.phase = FormingPhase::Faulted;
        self.phase_elapsed_ms = 0;
        self.outputs = FormingOutputs::safe();
    }

    fn progress(&self) -> f64 {
        let duration = self.phase.duration_ms();
        if duration == 0 {
            0.0
        } else {
            (self.phase_elapsed_ms as f64 / duration as f64).clamp(0.0, 1.0)
        }
    }
}
