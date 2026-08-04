use hearthline_engine::{
    Effect, FieldSensor, IoDirection, ProcessEffect, SimulatedComponent, SimulationEvent,
};
use hearthline_model::{ProcessEvent, ProcessSignal, SignalValue, Text};

use crate::{BehaviorConfig, ConfigError, ConfigRepository};

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
                } => signals.push(sample_signal(
                    &candidate.config.id,
                    &candidate.config.label,
                    signal_tag,
                    unit,
                    *minimum,
                    *maximum,
                    *initial_value,
                )?),
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

        Ok(Self {
            id: loaded.config.id.clone(),
            label: loaded.config.label.clone(),
            environment: loaded.config.environment.clone(),
            zone: loaded.config.zone.clone(),
            role: loaded.config.role.clone(),
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
    } = controller.config.behavior
    else {
        return Err(ConfigError::new(format!(
            "HMI controller {id} is not a virtual controller"
        )));
    };
    Ok(ControllerRuntime {
        id: id.into(),
        ports: controller
            .config
            .interfaces
            .iter()
            .map(|interface| interface.id.clone())
            .collect(),
        scan_interval_ms,
    })
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
