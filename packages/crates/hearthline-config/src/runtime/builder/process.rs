use hearthline_engine::{
    Actuator, FieldSensor, IoDirection, OperatorInterface, RemoteIo, SafetyInterface,
    SimulatedComponent, SimulationEvent, VirtualPlc,
};
use hearthline_model::{ProcessEvent, ProcessSignal, SignalValue, Text};

use crate::appliance::{ApplianceConfig, BehaviorConfig, ConfigError, ConfigRepository};

use super::{component_id, parse_ipv4, port_id, routed_interfaces};
use crate::runtime::ConfiguredAppliance;

pub(super) fn build_process_appliance(
    config: &ApplianceConfig,
    appliances: &ConfigRepository,
) -> Result<ConfiguredAppliance, ConfigError> {
    let id = component_id(&config.id)?;
    let ports = config
        .interfaces
        .iter()
        .map(|interface| port_id(&interface.id))
        .collect::<Result<Vec<_>, _>>()?;
    match &config.behavior {
        BehaviorConfig::VirtualController {
            scan_interval_ms, ..
        } => {
            let interfaces = routed_interfaces(config)?;
            let controller = if interfaces.is_empty() {
                VirtualPlc::new(id, ports, *scan_interval_ms, [])
            } else {
                VirtualPlc::with_network(
                    id,
                    ports,
                    *scan_interval_ms,
                    [],
                    interfaces,
                    config
                        .default_gateway
                        .as_deref()
                        .map(|gateway| parse_ipv4(gateway, "default gateway"))
                        .transpose()?,
                )
            };
            Ok(ConfiguredAppliance::VirtualPlc(Box::new(controller)))
        }
        BehaviorConfig::OperatorInterface { command_tags, .. } => {
            let tags = command_tags
                .iter()
                .map(|tag| Text::try_new(tag).map_err(|error| ConfigError::new(error.to_string())))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ConfiguredAppliance::Hmi(Box::new(
                OperatorInterface::with_kind(id, config.kind, ports, tags),
            )))
        }
        BehaviorConfig::RemoteIo { channels, .. } => {
            let channels = channels
                .iter()
                .map(|channel| channel_mapping(appliances, channel))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            Ok(ConfiguredAppliance::RemoteIo(Box::new(RemoteIo::new(
                id, ports, channels,
            ))))
        }
        BehaviorConfig::FieldSensor {
            signal_tag,
            initial_value,
            ..
        } => {
            let mut sensor = FieldSensor::with_ports(
                id,
                ports,
                Text::try_new(signal_tag).map_err(|error| ConfigError::new(error.to_string()))?,
                100,
                1.0,
                0.0,
            );
            sensor.set_raw_value(initial_value.unwrap_or(0.0));
            Ok(ConfiguredAppliance::FieldSensor(Box::new(sensor)))
        }
        BehaviorConfig::FieldActuator {
            command_tag,
            safe_state,
            ..
        } => Ok(ConfiguredAppliance::FieldActuator(Box::new(
            Actuator::with_ports(
                id,
                ports,
                Text::try_new(command_tag).map_err(|error| ConfigError::new(error.to_string()))?,
                SignalValue::Text(
                    Text::try_new(safe_state)
                        .map_err(|error| ConfigError::new(error.to_string()))?,
                ),
                SignalValue::Text(
                    Text::try_new(safe_state)
                        .map_err(|error| ConfigError::new(error.to_string()))?,
                ),
            ),
        ))),
        BehaviorConfig::Safety {
            permissives,
            latched_trip,
            initially_permissive,
        } => {
            let tags = permissives
                .iter()
                .map(|tag| Text::try_new(tag).map_err(|error| ConfigError::new(error.to_string())))
                .collect::<Result<Vec<_>, _>>()?;
            let mut safety = SafetyInterface::with_ports(id, ports, tags);
            for tag in permissives {
                safety.handle(SimulationEvent::Process(ProcessEvent::Signal(
                    ProcessSignal {
                        tag: Text::try_new(tag)
                            .map_err(|error| ConfigError::new(error.to_string()))?,
                        value: SignalValue::Bool(initially_permissive.contains(tag)),
                        quality_good: true,
                        timestamp_ms: 0,
                    },
                )));
            }
            if !latched_trip {
                safety.handle(SimulationEvent::Process(ProcessEvent::Reset {
                    authorized: true,
                }));
            }
            Ok(ConfiguredAppliance::Safety(Box::new(safety)))
        }
        behavior => Err(ConfigError::new(format!(
            "appliance {} behavior {} is not an industrial process appliance",
            config.id,
            behavior.family()
        ))),
    }
}

fn channel_mapping(
    appliances: &ConfigRepository,
    component: &str,
) -> Result<Option<(Text<64>, IoDirection)>, ConfigError> {
    let loaded = appliances.get(component).ok_or_else(|| {
        ConfigError::new(format!(
            "remote I/O references unknown component {component}"
        ))
    })?;
    match &loaded.config.behavior {
        BehaviorConfig::FieldSensor { signal_tag, .. } => Ok(Some((
            Text::try_new(signal_tag).map_err(|error| ConfigError::new(error.to_string()))?,
            IoDirection::Input,
        ))),
        BehaviorConfig::FieldActuator { command_tag, .. } => Ok(Some((
            Text::try_new(command_tag).map_err(|error| ConfigError::new(error.to_string()))?,
            IoDirection::Output,
        ))),
        BehaviorConfig::Safety { .. } => Ok(None),
        _ => Err(ConfigError::new(format!(
            "remote I/O channel {component} is not a field component"
        ))),
    }
}
