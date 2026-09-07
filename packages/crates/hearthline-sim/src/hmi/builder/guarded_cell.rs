use crate::{BehaviorConfig, ConfigError, ConfigRepository};

use super::super::state::{GuardedCellRuntime, HandoffStationRuntime};

const GUARD_SAFETY: &str = "area-02-cell-guard-safe-01";
const GATE_SENSOR: &str = "area-02-cell-gate-pos-01";

pub(super) fn guarded_cell_runtime(
    appliances: &ConfigRepository,
    environment: &str,
    zone: &str,
) -> Result<Option<GuardedCellRuntime>, ConfigError> {
    let Some(gate) = appliances.get(GATE_SENSOR) else {
        return Ok(None);
    };
    if gate.config.environment != environment || gate.config.zone != zone {
        return Ok(None);
    }
    require_component(appliances, environment, zone, GATE_SENSOR, "field sensor")?;
    require_component(
        appliances,
        environment,
        zone,
        GUARD_SAFETY,
        "safety interface",
    )?;

    let mut handoffs = Vec::new();
    for number in 1..=4 {
        let mould = format!("mould-{number:02}");
        let actuator = format!("area-02-m{number:02}-handoff-01");
        let in_cell_sensor = format!("area-02-m{number:02}-handoff-in-01");
        let operator_side_sensor = format!("area-02-m{number:02}-handoff-out-01");
        require_component(appliances, environment, zone, &actuator, "field actuator")?;
        require_component(
            appliances,
            environment,
            zone,
            &in_cell_sensor,
            "field sensor",
        )?;
        require_component(
            appliances,
            environment,
            zone,
            &operator_side_sensor,
            "field sensor",
        )?;
        handoffs.push(HandoffStationRuntime::new(
            mould,
            actuator,
            in_cell_sensor,
            operator_side_sensor,
        ));
    }
    Ok(Some(GuardedCellRuntime::new(
        GUARD_SAFETY.into(),
        GATE_SENSOR.into(),
        handoffs,
    )))
}

fn require_component(
    appliances: &ConfigRepository,
    environment: &str,
    zone: &str,
    id: &str,
    expected: &str,
) -> Result<(), ConfigError> {
    let loaded = appliances
        .get(id)
        .ok_or_else(|| ConfigError::new(format!("guarded forming cell requires {id}")))?;
    if loaded.config.environment != environment || loaded.config.zone != zone {
        return Err(ConfigError::new(format!(
            "guarded-cell component {id} is outside {environment}/{zone}"
        )));
    }
    let valid = matches!(
        (&loaded.config.behavior, expected),
        (BehaviorConfig::FieldSensor { .. }, "field sensor")
            | (BehaviorConfig::FieldActuator { .. }, "field actuator")
            | (BehaviorConfig::Safety { .. }, "safety interface")
    );
    if !valid {
        return Err(ConfigError::new(format!(
            "guarded-cell component {id} must be a {expected}"
        )));
    }
    Ok(())
}
