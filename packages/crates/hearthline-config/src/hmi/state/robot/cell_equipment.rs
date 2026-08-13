use crate::hmi::schema::{HmiCellGuardState, HmiGuardedCellState, HmiHandoffStationState};
use crate::hmi::{HmiActuator, HmiSafety, HmiSignal};

use super::super::HmiSession;

const TRANSFER_TRAVEL_MS: f64 = 1_600.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransferState {
    InCell,
    MovingToOperator,
    OperatorSide,
    MovingToCell,
    Stopped,
}

impl TransferState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::InCell => "in-cell",
            Self::MovingToOperator => "moving-to-operator",
            Self::OperatorSide => "operator-side",
            Self::MovingToCell => "moving-to-cell",
            Self::Stopped => "stopped",
        }
    }
}

#[derive(Clone, Debug)]
pub(in crate::hmi) struct HandoffStationRuntime {
    mould: String,
    actuator: String,
    in_cell_sensor: String,
    operator_side_sensor: String,
    state: TransferState,
    progress_percent: f64,
    piece_present: bool,
}

impl HandoffStationRuntime {
    pub(in crate::hmi) fn new(
        mould: String,
        actuator: String,
        in_cell_sensor: String,
        operator_side_sensor: String,
    ) -> Self {
        Self {
            mould,
            actuator,
            in_cell_sensor,
            operator_side_sensor,
            state: TransferState::InCell,
            progress_percent: 0.0,
            piece_present: false,
        }
    }

    fn tick(&mut self, elapsed_ms: u64) {
        let delta = elapsed_ms as f64 / TRANSFER_TRAVEL_MS * 100.0;
        match self.state {
            TransferState::MovingToOperator => {
                self.progress_percent = (self.progress_percent + delta).min(100.0);
                if self.progress_percent >= 100.0 {
                    self.state = TransferState::OperatorSide;
                }
            }
            TransferState::MovingToCell => {
                self.progress_percent = (self.progress_percent - delta).max(0.0);
                if self.progress_percent <= 0.0 {
                    self.state = TransferState::InCell;
                }
            }
            _ => {}
        }
    }

    fn snapshot(&self) -> HmiHandoffStationState {
        HmiHandoffStationState {
            mould: self.mould.clone(),
            actuator: self.actuator.clone(),
            state: self.state.as_str(),
            progress_percent: self.progress_percent,
            in_cell_sensor: self.in_cell_sensor.clone(),
            operator_side_sensor: self.operator_side_sensor.clone(),
            in_cell_confirmed: self.state == TransferState::InCell,
            operator_side_confirmed: self.state == TransferState::OperatorSide,
            piece_present: self.piece_present,
        }
    }
}

#[derive(Clone, Debug)]
pub(in crate::hmi) struct GuardedCellRuntime {
    guard_safety: String,
    gate_position_sensor: String,
    gate_open: bool,
    handoffs: Vec<HandoffStationRuntime>,
}

impl GuardedCellRuntime {
    pub(in crate::hmi) fn new(
        guard_safety: String,
        gate_position_sensor: String,
        handoffs: Vec<HandoffStationRuntime>,
    ) -> Self {
        Self {
            guard_safety,
            gate_position_sensor,
            gate_open: false,
            handoffs,
        }
    }

    pub(in crate::hmi) const fn gate_open(&self) -> bool {
        self.gate_open
    }

    pub(in crate::hmi) fn set_gate_open(&mut self, open: bool) {
        self.gate_open = open;
    }

    pub(in crate::hmi) fn motion_active(&self) -> bool {
        self.handoffs.iter().any(|station| {
            matches!(
                station.state,
                TransferState::MovingToOperator | TransferState::MovingToCell
            )
        })
    }

    pub(in crate::hmi) fn ready_for_robot(&self, mould: &str) -> bool {
        self.handoffs
            .iter()
            .find(|station| station.mould == mould)
            .is_some_and(|station| station.state == TransferState::InCell)
    }

    pub(in crate::hmi) fn begin_delivery(&mut self, mould: &str) -> bool {
        let Some(station) = self
            .handoffs
            .iter_mut()
            .find(|station| station.mould == mould)
        else {
            return false;
        };
        if station.state != TransferState::InCell {
            return false;
        }
        station.piece_present = true;
        station.state = TransferState::MovingToOperator;
        true
    }

    pub(in crate::hmi) fn delivery_ready(&self, mould: &str) -> bool {
        self.handoffs
            .iter()
            .find(|station| station.mould == mould)
            .is_some_and(|station| station.state == TransferState::OperatorSide)
    }

    pub(in crate::hmi) fn begin_return(&mut self, mould: &str) {
        if let Some(station) = self
            .handoffs
            .iter_mut()
            .find(|station| station.mould == mould)
            && station.state == TransferState::OperatorSide
        {
            station.piece_present = false;
            station.state = TransferState::MovingToCell;
        }
    }

    pub(in crate::hmi) fn tick(&mut self, elapsed_ms: u64) {
        for station in &mut self.handoffs {
            station.tick(elapsed_ms);
        }
    }

    pub(in crate::hmi) fn trip_motion(&mut self) {
        for station in &mut self.handoffs {
            if matches!(
                station.state,
                TransferState::MovingToOperator | TransferState::MovingToCell
            ) {
                station.state = TransferState::Stopped;
            }
        }
    }

    pub(in crate::hmi) fn clear_trip(&mut self) {
        for station in &mut self.handoffs {
            if station.state == TransferState::Stopped {
                station.piece_present = false;
                station.state = TransferState::MovingToCell;
            }
        }
    }

    pub(in crate::hmi) fn snapshot(&self, safety: &[HmiSafety]) -> HmiGuardedCellState {
        let reset_required = safety
            .iter()
            .find(|state| state.component_id == self.guard_safety)
            .is_some_and(|state| state.trip_latched);
        HmiGuardedCellState {
            guard: HmiCellGuardState {
                safety_component: self.guard_safety.clone(),
                position_sensor: self.gate_position_sensor.clone(),
                position: if self.gate_open { "open" } else { "closed" },
                closed_permissive: !self.gate_open,
                reset_required,
            },
            handoff_stations: self
                .handoffs
                .iter()
                .map(HandoffStationRuntime::snapshot)
                .collect(),
        }
    }

    pub(in crate::hmi) fn sync_io(&self, signals: &mut [HmiSignal], actuators: &mut [HmiActuator]) {
        set_signal(
            signals,
            &self.gate_position_sensor,
            f64::from(!self.gate_open),
        );
        for station in &self.handoffs {
            set_signal(
                signals,
                &station.in_cell_sensor,
                f64::from(station.state == TransferState::InCell),
            );
            set_signal(
                signals,
                &station.operator_side_sensor,
                f64::from(station.state == TransferState::OperatorSide),
            );
            if let Some(actuator) = actuators
                .iter_mut()
                .find(|actuator| actuator.component_id == station.actuator)
            {
                actuator.current_state = station.state.as_str().into();
            }
        }
    }
}

fn set_signal(signals: &mut [HmiSignal], tag: &str, value: f64) {
    if let Some(signal) = signals.iter_mut().find(|signal| signal.tag == tag) {
        signal.value = value;
        signal.quality_good = true;
    }
}

impl HmiSession {
    pub(in crate::hmi) fn sync_guarded_cell_io(&mut self) {
        if let Some(cell) = &self.guarded_cell {
            cell.sync_io(&mut self.signals, &mut self.actuators);
        }
    }
}
