use hearthline_engine::{
    Effect, FieldSensor, FormingMeasurements, FormingProcess, IoDirection, ProcessEffect,
    SimulatedComponent, SimulationEvent,
};
use hearthline_model::{ProcessEvent, ProcessSignal, SignalValue, Text};

use crate::{BehaviorConfig, ConfigError, ConfigRepository};

use super::actions::process::load_control_program;
use super::state::{ControllerRuntime, HmiSession, RemoteIoRuntime};
use super::support::component_id;
use super::{HmiActuator, HmiAlarm, HmiAlarmSeverity, HmiPermissive, HmiSafety, HmiSignal};

impl HmiSession {
    pub fn from_repository(appliances: &ConfigRepository, id: &str) -> Result<Self, ConfigError> {
        let loaded = appliances
            .get(id)
            .ok_or_else(|| ConfigError::new(format!("unknown HMI {id}")))?;
        let BehaviorConfig::OperatorInterface {
            controller,
            permissions,
            signal_tags,
            command_tags,
        } = &loaded.config.behavior
        else {
            return Err(ConfigError::new(format!(
                "appliance {id} is not an operator interface"
            )));
        };
        if !loaded.config.tags.iter().any(|tag| tag == "interactive") {
            return Err(ConfigError::new(format!(
                "HMI {id} is not configured for interaction"
            )));
        }

        let controller_runtime = controller_runtime(appliances, controller)?;
        let mut signals = Vec::new();
        let mut actuators = Vec::new();
        let mut safety = Vec::new();
        for candidate in appliances.appliances().filter(|candidate| {
            candidate.config.environment == loaded.config.environment
                && candidate.config.zone == loaded.config.zone
        }) {
            match &candidate.config.behavior {
                BehaviorConfig::FieldSensor {
                    signal_tag,
                    unit,
                    minimum,
                    maximum,
                    initial_value: Some(initial_value),
                } if signal_tags.is_empty() || signal_tags.contains(signal_tag) => {
                    signals.push(sample_signal(
                        &candidate.config.id,
                        &candidate.config.label,
                        signal_tag,
                        unit,
                        *minimum,
                        *maximum,
                        *initial_value,
                    )?)
                }
                BehaviorConfig::FieldActuator {
                    command_tag,
                    safe_state,
                    feedback_tag,
                    states,
                } if command_tags.contains(command_tag) => actuators.push(HmiActuator {
                    component_id: candidate.config.id.clone(),
                    label: candidate.config.label.clone(),
                    command_tag: command_tag.clone(),
                    feedback_tag: feedback_tag.clone(),
                    safe_state: safe_state.clone(),
                    states: states.clone(),
                    current_state: safe_state.clone(),
                }),
                BehaviorConfig::Safety {
                    permissives,
                    latched_trip,
                    initially_permissive,
                } => safety.push(HmiSafety {
                    component_id: candidate.config.id.clone(),
                    label: candidate.config.label.clone(),
                    permissives: permissives
                        .iter()
                        .map(|tag| HmiPermissive {
                            tag: tag.clone(),
                            satisfied: initially_permissive.contains(tag),
                        })
                        .collect(),
                    trip_latched: *latched_trip,
                }),
                _ => {}
            }
        }
        if signals.is_empty() {
            return Err(ConfigError::new(format!(
                "interactive HMI {id} has no configured field signals"
            )));
        }
        if let Some(signal_tag) = signal_tags
            .iter()
            .find(|signal_tag| !signals.iter().any(|signal| signal.tag == **signal_tag))
        {
            return Err(ConfigError::new(format!(
                "interactive operator interface {id} references unknown area signal {signal_tag}"
            )));
        }
        if command_tags.is_empty() {
            return Err(ConfigError::new(format!(
                "interactive HMI {id} has no authorized command tags"
            )));
        }
        if let Some(command_tag) = command_tags.iter().find(|command_tag| {
            !actuators
                .iter()
                .any(|actuator| actuator.command_tag == **command_tag)
        }) {
            return Err(ConfigError::new(format!(
                "interactive HMI {id} command tag {command_tag} has no field actuator"
            )));
        }
        if let Some(actuator) = actuators.iter().find(|actuator| actuator.states.is_empty()) {
            return Err(ConfigError::new(format!(
                "interactive HMI {id} actuator {} has no configured states",
                actuator.component_id
            )));
        }
        if safety.is_empty() {
            return Err(ConfigError::new(format!(
                "interactive HMI {id} has no safety interface"
            )));
        }
        let remote_io = remote_io_runtime(
            appliances,
            controller,
            &loaded.config.environment,
            &loaded.config.zone,
            &signals,
            &actuators,
            &safety,
        )?;
        let alarms = safety
            .iter()
            .filter(|state| state.trip_latched)
            .map(|state| HmiAlarm {
                id: format!("startup-{}", state.component_id),
                code: "SAFETY-RESET-REQUIRED".into(),
                source: state.component_id.clone(),
                message: "Safety permissives are healthy; operator reset is required.".into(),
                severity: HmiAlarmSeverity::Trip,
                active: true,
                acknowledged: false,
                sequence: 0,
            })
            .collect();

        let process = controller_runtime
            .program
            .is_some()
            .then(|| forming_process(appliances, &loaded.config.environment, &loaded.config.zone))
            .transpose()?;

        Ok(Self {
            id: loaded.config.id.clone(),
            label: loaded.config.label.clone(),
            environment: loaded.config.environment.clone(),
            zone: loaded.config.zone.clone(),
            role: loaded.config.role.clone(),
            kind: loaded.config.kind,
            controller: controller_runtime,
            remote_io,
            permissions: permissions.clone(),
            ports: loaded
                .config
                .interfaces
                .iter()
                .map(|interface| interface.id.clone())
                .collect(),
            command_tags: command_tags.clone(),
            signals,
            actuators,
            safety,
            alarms,
            audit: Vec::new(),
            sequence: 0,
            process,
        })
    }
}

fn controller_runtime(
    appliances: &ConfigRepository,
    id: &str,
) -> Result<ControllerRuntime, ConfigError> {
    let controller = appliances
        .get(id)
        .ok_or_else(|| ConfigError::new(format!("HMI references unknown controller {id}")))?;
    let BehaviorConfig::VirtualController {
        scan_interval_ms, ..
    } = &controller.config.behavior
    else {
        return Err(ConfigError::new(format!(
            "HMI controller {id} is not a virtual controller"
        )));
    };
    let program = load_control_program(appliances, id)?;
    Ok(ControllerRuntime {
        id: id.into(),
        ports: controller
            .config
            .interfaces
            .iter()
            .map(|interface| interface.id.clone())
            .collect(),
        scan_interval_ms: *scan_interval_ms,
        program,
    })
}

fn forming_process(
    appliances: &ConfigRepository,
    environment: &str,
    zone: &str,
) -> Result<FormingProcess, ConfigError> {
    let value = |tag| forming_initial_value(appliances, environment, zone, tag);
    Ok(FormingProcess::new(FormingMeasurements {
        slip_tank_level_percent: value("area-02-lt-01")?,
        slip_density_g_cm3: value("area-02-dt-01")?,
        slip_viscosity_mpa_s: value("area-02-vis-01")?,
        slip_temperature_c: value("area-02-tt-01")?,
        slip_feed_flow_l_min: value("area-02-ft-01")?,
        slip_feed_pressure_bar: value("area-02-pt-01")?,
        mould_pressure_bar: value("area-02-pt-02")?,
        mould_temperature_c: value("area-02-tt-02")?,
        fill_head_position_mm: value("area-02-pos-01")?,
        mould_position_mm: value("area-02-pos-02")?,
        water_flow_l_min: value("area-02-ft-02")?,
        excess_slip_drain_flow_l_min: value("area-02-ft-03")?,
        mould_moisture_percent: value("area-02-mt-02")?,
        compressed_air_pressure_bar: value("area-02-pt-04")?,
        vacuum_pressure_kpa: value("area-02-vt-01")?,
        robot_position_mm: value("area-02-pos-03")?,
        piece_gripped: value("area-02-pe-01")? >= 0.5,
    }))
}

fn forming_initial_value(
    appliances: &ConfigRepository,
    environment: &str,
    zone: &str,
    tag: &str,
) -> Result<f64, ConfigError> {
    appliances
        .appliances()
        .find_map(|candidate| {
            if candidate.config.environment != environment || candidate.config.zone != zone {
                return None;
            }
            let BehaviorConfig::FieldSensor {
                signal_tag,
                initial_value,
                ..
            } = &candidate.config.behavior
            else {
                return None;
            };
            (signal_tag == tag).then_some(*initial_value)
        })
        .flatten()
        .ok_or_else(|| ConfigError::new(format!("forming process requires initial signal {tag}")))
}

fn remote_io_runtime(
    appliances: &ConfigRepository,
    controller: &str,
    environment: &str,
    zone: &str,
    signals: &[HmiSignal],
    actuators: &[HmiActuator],
    safety: &[HmiSafety],
) -> Result<RemoteIoRuntime, ConfigError> {
    let remote_io = appliances
        .appliances()
        .find(|candidate| {
            candidate.config.environment == environment
                && candidate.config.zone == zone
                && matches!(
                    &candidate.config.behavior,
                    BehaviorConfig::RemoteIo {
                        controller: assigned,
                        ..
                    } if assigned == controller
                )
        })
        .ok_or_else(|| {
            ConfigError::new(format!(
                "interactive HMI for {zone} has no remote I/O assigned to {controller}"
            ))
        })?;
    let BehaviorConfig::RemoteIo { channels, .. } = &remote_io.config.behavior else {
        unreachable!("remote I/O selection checked its behavior");
    };
    let required_components = signals
        .iter()
        .map(|signal| signal.component_id.as_str())
        .chain(
            actuators
                .iter()
                .map(|actuator| actuator.component_id.as_str()),
        )
        .chain(safety.iter().map(|state| state.component_id.as_str()));
    if let Some(missing) = required_components
        .into_iter()
        .find(|component| !channels.iter().any(|channel| channel == component))
    {
        return Err(ConfigError::new(format!(
            "remote I/O {} does not map HMI component {missing}",
            remote_io.config.id
        )));
    }
    let mut mapped = Vec::new();
    for signal in signals {
        if channels.contains(&signal.component_id) {
            mapped.push((signal.tag.clone(), IoDirection::Input));
        }
    }
    for actuator in actuators {
        if channels.contains(&actuator.component_id) {
            mapped.push((actuator.command_tag.clone(), IoDirection::Output));
        }
    }
    Ok(RemoteIoRuntime {
        id: remote_io.config.id.clone(),
        ports: remote_io
            .config
            .interfaces
            .iter()
            .map(|interface| interface.id.clone())
            .collect(),
        channels: mapped,
    })
}

#[allow(clippy::too_many_arguments)]
fn sample_signal(
    component: &str,
    label: &str,
    tag: &str,
    unit: &str,
    minimum: f64,
    maximum: f64,
    initial_value: f64,
) -> Result<HmiSignal, ConfigError> {
    let mut sensor = FieldSensor::new(component_id(component), Text::from(tag), 1_000, 1.0, 0.0);
    sensor.set_raw_value(initial_value);
    let effects = sensor.handle(SimulationEvent::Process(ProcessEvent::Tick {
        elapsed_ms: 1_000,
    }));
    let signal = effects.iter().find_map(|effect| {
        let Effect::Process(ProcessEffect::Signal(signal)) = effect else {
            return None;
        };
        Some(signal)
    });
    let Some(ProcessSignal {
        value: SignalValue::Analog(value),
        quality_good,
        timestamp_ms,
        ..
    }) = signal
    else {
        return Err(ConfigError::new(format!(
            "sensor {component} did not produce an analog sample"
        )));
    };
    Ok(HmiSignal {
        component_id: component.into(),
        label: label.into(),
        tag: tag.into(),
        unit: unit.into(),
        minimum,
        maximum,
        value: *value,
        quality_good: *quality_good,
        timestamp_ms: *timestamp_ms,
    })
}
