use std::collections::BTreeMap;
use std::ops::Deref;

use hearthline_engine::{BodyPreparationProcess, CeramicSlipBatch, IoDirection};
use hearthline_model::{ComponentKind, FixedValue};

use super::actions::process::ConfiguredControlProgram;
use super::{
    HMI_SCHEMA_VERSION, HmiActionReport, HmiActionStatus, HmiActuator, HmiAlarm, HmiAlarmSeverity,
    HmiAuditEntry, HmiControlMode, HmiControlProgramDocument, HmiControlProgramState,
    HmiControlStation, HmiMouldProcessState, HmiParameter, HmiProcessState, HmiRecipe, HmiSafety,
    HmiSignal, HmiSnapshot, HmiStationStatus, HmiTraceEntry,
};

const MAX_AUDIT_ENTRIES: usize = 32;

mod mould;
mod process;
mod robot;
mod store;
mod supervisory;

pub(in crate::hmi) use mould::MouldProcessRuntime;
pub(in crate::hmi) use process::{setpoints_from_parameter_groups, setpoints_from_parameters};
pub(in crate::hmi) use robot::{
    GuardedCellRuntime, HandoffStationRuntime, RobotRuntime, angle as robot_angle,
    percentage as robot_percentage, position as robot_position, robot_pose,
};
pub use store::{PlantRuntimeOwnership, PlantRuntimeStore};
pub(in crate::hmi) use supervisory::SupervisoryRuntime;

#[derive(Debug)]
pub struct PlantControlRuntime {
    pub(super) context: HmiOperatorContext,
    pub(super) controller: ControllerRuntime,
    pub(super) signals: Vec<RuntimeSignal>,
    pub(super) actuators: Vec<HmiActuator>,
    pub(super) safety: Vec<HmiSafety>,
    pub(super) alarms: Vec<HmiAlarm>,
    pub(super) audit: Vec<HmiAuditEntry>,
    pub(super) sequence: u64,
    pub(super) body_preparation: Option<Box<BodyPreparationProcess>>,
    pub(super) moulds: BTreeMap<String, MouldProcessRuntime>,
    pub(super) shared_tank_level_percent: FixedValue,
    pub(super) robot: Option<Box<RobotRuntime>>,
    pub(super) guarded_cell: Option<GuardedCellRuntime>,
    pub(super) supervisory: Option<SupervisoryRuntime>,
}

#[doc(hidden)]
#[derive(Debug)]
pub struct HmiOperatorContext {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) environment: String,
    pub(super) zone: String,
    pub(super) role: String,
    pub(super) kind: ComponentKind,
    pub(super) controller_id: String,
    pub(super) remote_io: Vec<RemoteIoRuntime>,
    pub(super) permissions: Vec<String>,
    pub(super) ports: Vec<String>,
    pub(super) command_tags: Vec<String>,
    pub(super) signal_scope: Vec<String>,
    pub(super) safety_scope: Vec<String>,
    pub(super) supervisory_visible: bool,
}

#[derive(Debug)]
pub(super) struct ControllerRuntime {
    pub(super) id: String,
    pub(super) ports: Vec<String>,
    pub(super) scan_interval_ms: u64,
    pub(super) program: Option<Box<ConfiguredControlProgram>>,
    pub(super) stations: BTreeMap<String, ControlStationRuntime>,
    pub(super) parameters: Vec<RuntimeParameter>,
    pub(super) recipes: Vec<HmiRecipe>,
    pub(super) active_recipe: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct ControlStationRuntime {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) station_type: String,
    pub(super) target: String,
    pub(super) positions: Vec<HmiControlMode>,
    pub(super) selected_mode: HmiControlMode,
    pub(super) setup_password_sha256: Option<String>,
    pub(super) setup_authenticated: bool,
    pub(super) bypassed_permissives: Vec<String>,
    pub(super) retained_protections: Vec<String>,
}

impl ControlStationRuntime {
    pub(super) fn snapshot(&self) -> HmiControlStation {
        HmiControlStation {
            station_type: self.station_type.clone(),
            target: self.target.clone(),
            positions: self.positions.clone(),
            selected_mode: self.selected_mode,
            setup_authenticated: self.setup_authenticated,
            sensor_bypass_active: self.selected_mode == HmiControlMode::Setup
                && self.setup_authenticated,
            bypassed_permissives: self.bypassed_permissives.clone(),
            retained_protections: self.retained_protections.clone(),
        }
    }

    pub(super) fn status(&self) -> HmiStationStatus {
        let snapshot = self.snapshot();
        HmiStationStatus {
            station_id: self.id.clone(),
            label: self.label.clone(),
            station_type: snapshot.station_type,
            target: snapshot.target,
            selected_mode: snapshot.selected_mode,
            setup_authenticated: snapshot.setup_authenticated,
            sensor_bypass_active: snapshot.sensor_bypass_active,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct RemoteIoRuntime {
    pub(super) id: String,
    pub(super) ports: Vec<String>,
    pub(super) channels: Vec<(String, IoDirection)>,
}

#[derive(Clone, Debug)]
pub(in crate::hmi) struct RuntimeSignal {
    pub(super) component_id: String,
    pub(super) label: String,
    pub(super) tag: String,
    pub(super) unit: String,
    pub(super) minimum: FixedValue,
    pub(super) maximum: FixedValue,
    pub(super) value: FixedValue,
    pub(super) quality_good: bool,
    pub(super) timestamp_ms: u64,
}

impl RuntimeSignal {
    pub(super) fn snapshot(&self) -> HmiSignal {
        HmiSignal {
            component_id: self.component_id.clone(),
            label: self.label.clone(),
            tag: self.tag.clone(),
            unit: self.unit.clone(),
            minimum: super::quantities::project(self.minimum),
            maximum: super::quantities::project(self.maximum),
            value: super::quantities::project(self.value),
            quality_good: self.quality_good,
            timestamp_ms: self.timestamp_ms,
        }
    }
}

#[derive(Clone, Debug)]
pub(in crate::hmi) struct RuntimeParameter {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) target: String,
    pub(super) unit: String,
    pub(super) minimum: FixedValue,
    pub(super) maximum: FixedValue,
    pub(super) step: FixedValue,
    pub(super) value: FixedValue,
}

impl RuntimeParameter {
    fn snapshot(&self) -> HmiParameter {
        HmiParameter {
            id: self.id.clone(),
            label: self.label.clone(),
            target: self.target.clone(),
            unit: self.unit.clone(),
            minimum: super::quantities::project(self.minimum),
            maximum: super::quantities::project(self.maximum),
            step: super::quantities::project(self.step),
            value: super::quantities::project(self.value),
        }
    }
}

impl HmiOperatorContext {
    pub(super) fn vacant() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            environment: String::new(),
            zone: String::new(),
            role: String::new(),
            kind: ComponentKind::Hmi,
            controller_id: String::new(),
            remote_io: Vec::new(),
            permissions: Vec::new(),
            ports: Vec::new(),
            command_tags: Vec::new(),
            signal_scope: Vec::new(),
            safety_scope: Vec::new(),
            supervisory_visible: false,
        }
    }

    pub(super) fn cell_id(&self) -> &str {
        if self.environment == "Body Preparation" {
            "body-preparation-plant"
        } else {
            &self.controller_id
        }
    }
}

impl ControllerRuntime {
    pub(super) fn vacant() -> Self {
        Self {
            id: String::new(),
            ports: Vec::new(),
            scan_interval_ms: 0,
            program: None,
            stations: BTreeMap::new(),
            parameters: Vec::new(),
            recipes: Vec::new(),
            active_recipe: None,
        }
    }
}

impl Deref for PlantControlRuntime {
    type Target = HmiOperatorContext;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

impl PlantControlRuntime {
    pub(in crate::hmi) fn released_slip(&self) -> Option<CeramicSlipBatch> {
        self.body_preparation
            .as_ref()
            .and_then(|process| process.released_slip())
    }

    pub(in crate::hmi) fn apply_released_slip(&mut self, batch: CeramicSlipBatch) {
        for mould in self.moulds.values_mut() {
            mould.apply_slip_batch(batch);
        }
    }

    pub fn control_program(&self) -> Option<HmiControlProgramDocument> {
        if !self.has_permission("view-control-source") {
            return None;
        }
        self.controller
            .program
            .as_ref()
            .map(|program| HmiControlProgramDocument {
                schema_version: HMI_SCHEMA_VERSION,
                controller: self.controller.id.clone(),
                language: "structured-text",
                program: program.program_name().into(),
                task: program.task_name().into(),
                source_path: program.source_path().into(),
                binding_path: program.binding_path().into(),
                revision: program.revision().into(),
                source: program.source().into(),
                binding_yaml: program.binding_yaml().into(),
            })
    }

    pub fn snapshot(&self) -> HmiSnapshot {
        let moulds = self
            .moulds
            .values()
            .map(MouldProcessRuntime::snapshot)
            .collect::<Vec<_>>();
        HmiSnapshot {
            schema_version: HMI_SCHEMA_VERSION,
            id: self.id.clone(),
            label: self.label.clone(),
            environment: self.environment.clone(),
            zone: self.zone.clone(),
            role: self.role.clone(),
            interface_kind: self.kind.to_string(),
            controller: self.controller.id.clone(),
            remote_io: self
                .remote_io
                .first()
                .map(|remote_io| remote_io.id.clone())
                .unwrap_or_default(),
            remote_io_stations: self
                .remote_io
                .iter()
                .map(|remote_io| remote_io.id.clone())
                .collect(),
            permissions: self.permissions.clone(),
            sequence: self.sequence,
            control_program: self
                .has_permission("view-control-source")
                .then_some(self.controller.program.as_ref())
                .flatten()
                .map(|program| HmiControlProgramState {
                    language: "structured-text",
                    program: program.program_name().into(),
                    task: program.task_name().into(),
                    source_path: program.source_path().into(),
                    binding_path: program.binding_path().into(),
                    revision: program.revision().into(),
                    current_step: self
                        .control_program_current_step(program.runtime().current_step()),
                    scan_interval_ms: program.runtime().program().scan_interval_ms,
                    watchdog_ms: program.watchdog_ms(),
                }),
            control_station: self
                .controller
                .stations
                .get(&self.id)
                .map(ControlStationRuntime::snapshot),
            station_status: if self.has_permission("configure-parameters") {
                self.controller
                    .stations
                    .values()
                    .map(ControlStationRuntime::status)
                    .collect()
            } else {
                self.controller
                    .stations
                    .get(&self.id)
                    .map(ControlStationRuntime::status)
                    .into_iter()
                    .collect()
            },
            parameters: if self.has_permission("configure-parameters") {
                self.controller
                    .parameters
                    .iter()
                    .map(RuntimeParameter::snapshot)
                    .collect()
            } else {
                Vec::new()
            },
            recipes: if self.has_permission("select-recipe") {
                self.controller.recipes.clone()
            } else {
                Vec::new()
            },
            active_recipe: self
                .has_permission("select-recipe")
                .then(|| self.controller.active_recipe.clone())
                .flatten(),
            process: self.process_snapshot(&moulds),
            body_preparation: self.body_preparation_snapshot(),
            moulds,
            robot: self.robot.as_ref().and_then(|robot| {
                self.controller
                    .stations
                    .get(&self.id)
                    .filter(|station| {
                        matches!(
                            station.station_type.as_str(),
                            "robot-joystick" | "machine-pc"
                        )
                    })
                    .map(|_| robot.snapshot())
            }),
            guarded_cell: self
                .guarded_cell
                .as_ref()
                .map(|cell| cell.snapshot(&self.safety)),
            supervisory: self.supervisory.as_ref().and_then(|runtime| {
                self.supervisory_visible
                    .then(|| runtime.snapshot(&self.signals, &self.alarms, &self.audit))
            }),
            signals: self
                .signals
                .iter()
                .filter(|signal| self.signal_scope.iter().any(|tag| tag == &signal.tag))
                .map(RuntimeSignal::snapshot)
                .collect(),
            actuators: self
                .actuators
                .iter()
                .filter(|actuator| {
                    self.command_tags
                        .iter()
                        .any(|tag| tag == &actuator.command_tag)
                })
                .cloned()
                .collect(),
            safety: self
                .safety
                .iter()
                .filter(|safety| self.safety_in_scope(&safety.component_id))
                .cloned()
                .collect(),
            alarms: self
                .alarms
                .iter()
                .filter(|alarm| self.alarm_in_scope(&alarm.source, &alarm.code))
                .cloned()
                .collect(),
            audit: self.audit.clone(),
        }
    }

    pub(super) fn has_permission(&self, permission: &str) -> bool {
        self.permissions
            .iter()
            .any(|candidate| candidate == permission)
    }

    pub(super) fn safety_in_scope(&self, safety_id: &str) -> bool {
        self.safety_scope
            .iter()
            .any(|candidate| candidate == safety_id)
    }

    fn alarm_in_scope(&self, source: &str, code: &str) -> bool {
        self.environment != "Body Preparation" || self.body_alarm_in_scope(source, code)
    }

    fn process_snapshot(&self, moulds: &[HmiMouldProcessState]) -> Option<HmiProcessState> {
        if let Some(snapshot) = self.body_process_snapshot() {
            return Some(snapshot);
        }
        if moulds.is_empty() {
            return None;
        }
        let local_target = self
            .controller
            .stations
            .get(&self.id)
            .filter(|station| station.station_type == "mould-panel")
            .map(|station| station.target.as_str());
        let selected =
            local_target.and_then(|target| moulds.iter().find(|mould| mould.target == target));
        let phase = selected
            .map(|mould| mould.phase)
            .unwrap_or_else(|| mould::aggregate_phase(moulds));
        Some(HmiProcessState {
            model: "ceramic-slip-pressure-casting-cell",
            phase,
            running: selected
                .map(|mould| mould.running)
                .unwrap_or_else(|| moulds.iter().any(|mould| mould.running)),
            phase_elapsed_ms: selected
                .map(|mould| mould.phase_elapsed_ms)
                .unwrap_or_else(|| {
                    moulds
                        .iter()
                        .map(|mould| mould.phase_elapsed_ms)
                        .max()
                        .unwrap_or_default()
                }),
            scan_count: selected
                .map(|mould| mould.scan_count)
                .unwrap_or_else(|| moulds.iter().map(|mould| mould.scan_count).sum()),
            cycle_count: selected
                .map(|mould| mould.cycle_count)
                .unwrap_or_else(|| moulds.iter().map(|mould| mould.cycle_count).sum()),
            fault: selected
                .and_then(|mould| mould.fault)
                .or_else(|| moulds.iter().find_map(|mould| mould.fault)),
            phases: mould::phases(),
        })
    }

    pub(super) fn raise_alarm(
        &mut self,
        code: &str,
        source: &str,
        message: &str,
        severity: HmiAlarmSeverity,
    ) {
        if self
            .alarms
            .iter()
            .any(|alarm| alarm.active && alarm.code == code && alarm.source == source)
        {
            return;
        }
        self.alarms.push(HmiAlarm {
            id: format!("alarm-{}", self.sequence),
            code: code.into(),
            source: source.into(),
            message: message.into(),
            severity,
            active: true,
            acknowledged: false,
            sequence: self.sequence,
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn finish(
        &mut self,
        status: HmiActionStatus,
        message: String,
        action: &str,
        target: &str,
        result: &str,
        mut trace: Vec<HmiTraceEntry>,
    ) -> HmiActionReport {
        for (index, entry) in trace.iter_mut().enumerate() {
            entry.sequence = index;
        }
        self.record_audit(action, target, result);
        HmiActionReport {
            schema_version: HMI_SCHEMA_VERSION,
            status,
            message,
            trace,
            snapshot: self.snapshot(),
        }
    }

    fn record_audit(&mut self, action: &str, target: &str, result: &str) {
        self.audit.push(HmiAuditEntry {
            sequence: self.sequence,
            action: action.into(),
            target: target.into(),
            result: result.into(),
        });
        if self.audit.len() > MAX_AUDIT_ENTRIES {
            self.audit.remove(0);
        }
    }
}
