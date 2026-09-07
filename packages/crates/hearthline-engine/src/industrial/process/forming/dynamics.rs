use hearthline_model::{FixedValue, fixed};

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
        self.measurements.slip_feed_flow_l_min = fixed!(0.0);
        self.measurements.water_flow_l_min = fixed!(0.0);
        self.measurements.excess_slip_drain_flow_l_min = fixed!(0.0);
        match self.phase {
            FormingPhase::Filling => {
                self.measurements.slip_tank_level_percent =
                    self.tank_level_at_cycle_start - fixed!(0.8) * progress;
                self.measurements.slip_feed_flow_l_min =
                    fixed!(85.0) * self.material_effects.filling_flow_factor;
                self.measurements.fill_head_position_mm = fixed!(800.0) * progress;
            }
            FormingPhase::Pressurizing => {
                self.measurements.fill_head_position_mm = fixed!(800.0) * (fixed!(1.0) - progress);
                self.measurements.mould_pressure_bar = self.setpoints.pressure_bar * progress;
            }
            FormingPhase::PressureDwell => {
                self.measurements.mould_pressure_bar = self.setpoints.pressure_bar;
                let pressure_moisture_reduction = (self.setpoints.pressure_bar / fixed!(40.0)
                    * fixed!(3.0))
                .clamp(fixed!(0.0), fixed!(3.0));
                self.measurements.piece_moisture_percent =
                    self.material_effects.predicted_green_moisture_percent
                        - pressure_moisture_reduction * progress;
            }
            FormingPhase::Depressurizing => {
                self.measurements.mould_pressure_bar =
                    self.setpoints.pressure_bar * (fixed!(1.0) - progress);
            }
            FormingPhase::Draining => {
                self.measurements.excess_slip_drain_flow_l_min =
                    fixed!(70.0) * (fixed!(1.0) - progress);
            }
            FormingPhase::ReleaseWater => {
                self.measurements.mould_pressure_bar = fixed!(0.0);
                self.measurements.water_flow_l_min = fixed!(10.0);
                self.measurements.mould_moisture_percent = fixed!(8.0) + fixed!(6.0) * progress;
            }
            FormingPhase::ReleaseAir => {
                self.measurements.mould_pressure_bar = fixed!(1.0) * (fixed!(1.0) - progress);
            }
            FormingPhase::OpeningMould => {
                self.measurements.mould_position_mm = fixed!(600.0) * progress;
            }
            FormingPhase::RobotPickup => {
                self.measurements.mould_position_mm = fixed!(600.0);
                self.measurements.robot_position_mm = fixed!(1_200.0) * progress;
                self.measurements.piece_gripped = progress >= fixed!(0.8);
            }
            FormingPhase::RobotDelivery => {
                self.measurements.robot_position_mm = fixed!(1_200.0) + fixed!(1_800.0) * progress;
                self.measurements.piece_gripped = progress < fixed!(0.95);
            }
            FormingPhase::MouldWash => {
                self.measurements.water_flow_l_min = fixed!(18.0);
                self.measurements.mould_moisture_percent = fixed!(14.0) + fixed!(16.0) * progress;
                self.measurements.robot_position_mm = fixed!(3_000.0) * (fixed!(1.0) - progress);
            }
            FormingPhase::AirPurge => {
                self.measurements.mould_moisture_percent = fixed!(30.0) - fixed!(15.0) * progress;
                self.measurements.robot_position_mm = fixed!(0.0);
            }
            FormingPhase::VacuumDry => {
                self.measurements.vacuum_pressure_kpa = fixed!(-80.0) * progress;
                self.measurements.mould_moisture_percent = fixed!(15.0) - fixed!(12.0) * progress;
            }
            FormingPhase::ClosingMould => {
                self.measurements.vacuum_pressure_kpa = fixed!(-80.0) * (fixed!(1.0) - progress);
                self.measurements.mould_position_mm = fixed!(600.0) * (fixed!(1.0) - progress);
            }
            FormingPhase::Idle | FormingPhase::Faulted => {}
        }
    }

    pub(super) fn evaluate_fault(&mut self) -> Option<FormingTrip> {
        match self.fault {
            Some(FormingFault::SlipSupplyLoss) if self.phase == FormingPhase::Filling => {
                self.measurements.slip_feed_pressure_bar = fixed!(0.0);
                self.measurements.slip_feed_flow_l_min = fixed!(0.0);
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
                self.measurements.compressed_air_pressure_bar = fixed!(1.0);
                self.measurements.mould_pressure_bar = fixed!(0.5);
                Some(FormingTrip::CompressedAirLow)
            }
            Some(FormingFault::MouldOverpressure)
                if matches!(
                    self.phase,
                    FormingPhase::Pressurizing | FormingPhase::PressureDwell
                ) =>
            {
                self.measurements.mould_pressure_bar = fixed!(9.5);
                Some(FormingTrip::MouldOverpressure)
            }
            Some(FormingFault::VacuumLoss) if self.phase == FormingPhase::VacuumDry => {
                self.measurements.vacuum_pressure_kpa = fixed!(-10.0);
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

    fn progress(&self) -> FixedValue {
        let duration = self.setpoints.phase_duration_ms(self.phase);
        if duration == 0 {
            fixed!(0.0)
        } else {
            FixedValue::from_u64_ratio(self.phase_elapsed_ms, duration)
                .clamp(fixed!(0.0), fixed!(1.0))
        }
    }
}
